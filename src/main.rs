mod app;
mod client;
mod config;
mod event;
mod event_modal;
mod ui;

use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::{
    event::{Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::{ActiveModal, App, AuthData, BgEvent, Pane};
use client::auth;
use client::graph_credential::GraphCredential;
use client::group::GroupPimClient;
use client::models::RoleType;
use client::pim::PimClient;
use event::EventAction;
use event_modal::ModalAction;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "azure_pim_tui=info".parse().unwrap()),
        )
        .with_writer(io::stderr)
        .init();

    let config = config::Config::load()?;
    let auto_refresh = Duration::from_secs(config.auto_refresh_secs);
    let mut app = App::new(config);

    // Spawn auth in background
    let tx = app.bg_tx.clone();
    tokio::spawn(async move {
        match auth::get_auth_info().await {
            Ok(info) => {
                // Create a channel for GraphCredential to send status messages
                // (device code prompts) back to the TUI
                let (status_tx, mut status_rx) = tokio::sync::mpsc::unbounded_channel();
                let bg_tx = tx.clone();
                tokio::spawn(async move {
                    while let Some(msg) = status_rx.recv().await {
                        let _ = bg_tx.send(BgEvent::GraphStatus(msg));
                    }
                });

                let graph_credential =
                    Arc::new(GraphCredential::new(info.tenant_id, Some(status_tx)));

                let _ = tx.send(BgEvent::AuthReady(Ok(AuthData {
                    credential: info.credential,
                    graph_credential,
                    principal_id: info.principal_id,
                    user_display: info.user_display,
                    subscriptions: info.subscriptions,
                })));
            }
            Err(e) => {
                let _ = tx.send(BgEvent::AuthReady(Err(e.to_string())));
            }
        }
    });

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal, &mut app, auto_refresh).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(e) = result {
        eprintln!("Error: {e:?}");
    }

    Ok(())
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    auto_refresh: Duration,
) -> Result<()> {
    let tick_rate = Duration::from_millis(250);
    let mut last_tick = Instant::now();
    let mut last_refresh = Instant::now();
    let mut needs_fetch = false;
    let mut needs_permissions_fetch = false;

    loop {
        terminal.draw(|f| ui::layout::render(f, app))?;

        // Process background events (non-blocking)
        while let Ok(bg_event) = app.bg_rx.try_recv() {
            let should_fetch = matches!(bg_event, BgEvent::AuthReady(Ok(_)));
            let roles_loaded = matches!(bg_event, BgEvent::RolesLoaded(Ok(_)));
            app.handle_bg_event(bg_event);
            if should_fetch {
                needs_fetch = true;
            }
            if roles_loaded {
                needs_permissions_fetch = true;
            }
        }

        // Trigger resource fetch if needed
        if needs_fetch {
            spawn_fetch_resources(app);
            needs_fetch = false;
        }

        // Trigger permissions fetch after roles load
        if needs_permissions_fetch {
            spawn_fetch_permissions(app);
            needs_permissions_fetch = false;
        }

        // Trigger group fetch on first visit to Groups pane
        if app.needs_group_fetch() && app.auth.is_some() {
            spawn_fetch_groups(app);
        }

        // Auto-refresh timer (only for resource roles in resource pane)
        if last_refresh.elapsed() >= auto_refresh && app.auth.is_some() && !app.loading {
            spawn_fetch_resources(app);
            last_refresh = Instant::now();
        }

        // Handle input
        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = crossterm::event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                if app.modal != ActiveModal::None {
                    if let Some(action) = event_modal::handle_modal_key(app, key) {
                        handle_modal_action(app, action);
                    }
                } else {
                    match event::handle_key(app, key) {
                        EventAction::Refresh => {
                            match app.active_pane {
                                Pane::Resources => {
                                    spawn_fetch_resources(app);
                                }
                                Pane::Groups => {
                                    app.groups_loaded = false;
                                    spawn_fetch_groups(app);
                                }
                            }
                            last_refresh = Instant::now();
                        }
                        EventAction::PaneSwitch => {
                            // Group fetch triggered above via needs_group_fetch()
                        }
                        EventAction::None => {}
                    }
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }

        if app.should_quit {
            return Ok(());
        }
    }
}

fn spawn_fetch_resources(app: &App) {
    let auth = match &app.auth {
        Some(a) => a.clone(),
        None => return,
    };

    let tx = app.bg_tx.clone();
    tokio::spawn(async move {
        let client = PimClient::new(
            auth.credential.clone(),
            auth.principal_id.clone(),
            auth.subscriptions.clone(),
        );
        let result = client.fetch_roles().await;
        let _ = tx.send(BgEvent::RolesLoaded(result.map_err(|e| e.to_string())));
    });
}

fn spawn_fetch_permissions(app: &App) {
    let auth = match &app.auth {
        Some(a) => a.clone(),
        None => return,
    };

    let ids: Vec<String> = app
        .roles
        .iter()
        .filter(|r| r.role_type == RoleType::Resource && !r.role_definition_id.is_empty())
        .map(|r| r.role_definition_id.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .filter(|id| !app.role_permissions.contains_key(id))
        .collect();

    if ids.is_empty() {
        return;
    }

    let tx = app.bg_tx.clone();
    tokio::spawn(async move {
        let client = PimClient::new(
            auth.credential.clone(),
            auth.principal_id.clone(),
            auth.subscriptions.clone(),
        );
        let result = client.fetch_role_permissions(ids).await;
        let _ = tx.send(BgEvent::RolePermissionsLoaded(
            result.map_err(|e| e.to_string()),
        ));
    });
}

fn spawn_fetch_groups(app: &mut App) {
    let auth = match &app.auth {
        Some(a) => a.clone(),
        None => return,
    };

    app.groups_loading = true;
    app.group_status_message = "Authenticating with Graph API...".to_string();

    let tx = app.bg_tx.clone();
    tokio::spawn(async move {
        let client = GroupPimClient::new(auth.graph_credential.clone(), auth.principal_id.clone());
        let result = client.fetch_group_roles().await;
        let _ = tx.send(BgEvent::GroupRolesLoaded(result.map_err(|e| e.to_string())));
    });
}

fn handle_modal_action(app: &mut App, action: ModalAction) {
    let auth = match &app.auth {
        Some(a) => a.clone(),
        None => return,
    };

    let pane = app.active_pane;

    match action {
        ModalAction::Activate {
            indices,
            justification,
            duration_hours,
        } => {
            for idx in indices {
                let tx = app.bg_tx.clone();
                let role = app.active_roles()[idx].clone();
                let just = justification.clone();
                let auth = auth.clone();

                tokio::spawn(async move {
                    let result = if role.role_type == RoleType::Resource {
                        let client = PimClient::new(
                            auth.credential.clone(),
                            auth.principal_id.clone(),
                            auth.subscriptions.clone(),
                        );
                        client.activate_role(&role, &just, duration_hours).await
                    } else {
                        let client = GroupPimClient::new(
                            auth.graph_credential.clone(),
                            auth.principal_id.clone(),
                        );
                        client.activate_group(&role, &just, duration_hours).await
                    };

                    let _ = tx.send(BgEvent::ActivationResult {
                        index: idx,
                        result: result.map_err(|e| e.to_string()),
                    });
                });
            }
        }
        ModalAction::Deactivate { index } => {
            let tx = app.bg_tx.clone();
            let role = app.active_roles()[index].clone();

            tokio::spawn(async move {
                let result = if role.role_type == RoleType::Resource {
                    let client = PimClient::new(
                        auth.credential.clone(),
                        auth.principal_id.clone(),
                        auth.subscriptions.clone(),
                    );
                    client.deactivate_role(&role).await
                } else {
                    let client = GroupPimClient::new(
                        auth.graph_credential.clone(),
                        auth.principal_id.clone(),
                    );
                    client.deactivate_group(&role).await
                };

                let _ = tx.send(BgEvent::DeactivationResult {
                    index,
                    result: result.map_err(|e| e.to_string()),
                });
            });
        }
    }

    // Trigger refresh after activation/deactivation
    match pane {
        Pane::Resources => spawn_fetch_resources(app),
        Pane::Groups => {
            app.groups_loaded = false;
            spawn_fetch_groups(app);
        }
    }
}
