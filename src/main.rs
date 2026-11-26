mod app;
mod client;
mod config;
mod event;
mod event_modal;
mod event_mouse;
mod terminal;
mod ui;

use std::io;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{backend::CrosstermBackend, layout::Rect, Terminal};
use tracing_subscriber::fmt::writer::BoxMakeWriter;

use app::{App, AuthData, BgEvent, Pane};
use client::auth;
use client::error::PimError;
use client::graph_credential::GraphCredential;
use client::group::GroupPimClient;
use client::models::{PimRole, RoleType};
use client::pim::PimClient;
use event::EventAction;
use event_modal::ModalAction;

const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;
const ACTION_REFRESH_DELAY: Duration = Duration::from_secs(3);

fn rotate_log(path: &Path) {
    let too_big = std::fs::metadata(path).is_ok_and(|m| m.len() > MAX_LOG_BYTES);
    if too_big {
        let _ = std::fs::rename(path, path.with_extension("log.1"));
    }
}

fn init_logging() {
    let log_path = config::Config::log_path();
    let log_writer = log_path
        .parent()
        .and_then(|dir| std::fs::create_dir_all(dir).ok())
        .and_then(|_| {
            rotate_log(&log_path);
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
                .ok()
        });
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "azure_pim_tui=info".parse().unwrap()),
        )
        .with_ansi(false)
        .with_writer(match log_writer {
            Some(file) => BoxMakeWriter::new(std::sync::Mutex::new(file)),
            None => BoxMakeWriter::new(io::sink),
        })
        .init();
}

#[tokio::main]
async fn main() -> Result<()> {
    init_logging();

    let config = config::Config::load()?;
    let auto_refresh = Duration::from_secs(config.auto_refresh_secs);
    let mouse_enabled = config.mouse;
    let mut app = App::new(config);

    let tx = app.bg_tx.clone();
    tokio::spawn(async move {
        match auth::get_auth_info().await {
            Ok(info) => {
                let (status_tx, mut status_rx) = tokio::sync::mpsc::unbounded_channel();
                let bg_tx = tx.clone();
                tokio::spawn(async move {
                    while let Some(msg) = status_rx.recv().await {
                        let _ = bg_tx.send(BgEvent::GraphStatus(msg));
                    }
                });

                let graph_credential = Arc::new(GraphCredential::new(
                    info.http.clone(),
                    info.tenant_id,
                    Some(status_tx),
                ));

                let _ = tx.send(BgEvent::AuthReady(Ok(AuthData {
                    http: info.http,
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

    terminal::install_panic_hook(mouse_enabled);
    terminal::enter(mouse_enabled)?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal, &mut app, auto_refresh).await;

    terminal::leave(mouse_enabled)?;
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

        let mut refresh_after_action: Vec<Pane> = Vec::new();
        while let Ok(bg_event) = app.bg_rx.try_recv() {
            match &bg_event {
                BgEvent::AuthReady(Ok(_)) => needs_fetch = true,
                BgEvent::RolesLoaded {
                    generation,
                    result: Ok(_),
                } if *generation == app.resource_generation => needs_permissions_fetch = true,
                BgEvent::ActivationResult { pane, .. }
                | BgEvent::DeactivationResult { pane, .. } => {
                    if !refresh_after_action.contains(pane) {
                        refresh_after_action.push(*pane);
                    }
                }
                _ => {}
            }
            app.handle_bg_event(bg_event);
        }

        for pane in refresh_after_action {
            spawn_fetch(app, pane, ACTION_REFRESH_DELAY);
            if pane == Pane::Resources {
                last_refresh = Instant::now();
            }
        }

        if needs_fetch {
            spawn_fetch(app, Pane::Resources, Duration::ZERO);
            needs_fetch = false;
        }

        if needs_permissions_fetch {
            spawn_fetch_permissions(app);
            needs_permissions_fetch = false;
        }

        if app.needs_group_fetch() && app.auth.is_some() {
            spawn_fetch(app, Pane::Groups, Duration::ZERO);
        }

        let refresh_interval = auto_refresh * app.refresh_backoff;
        if last_refresh.elapsed() >= refresh_interval && app.auth.is_some() && !app.loading {
            spawn_fetch(app, Pane::Resources, Duration::ZERO);
            last_refresh = Instant::now();
        }

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if crossterm::event::poll(timeout)? {
            let key = match crossterm::event::read()? {
                Event::Key(key) => Some(key),
                Event::Mouse(mouse) => {
                    let size = terminal.size()?;
                    let area = Rect::new(0, 0, size.width, size.height);
                    event_mouse::handle_mouse(app, mouse, area);
                    None
                }
                Event::Resize(_, _) | Event::FocusGained => {
                    terminal.clear()?;
                    None
                }
                _ => None,
            };
            if let Some(key) = key {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('l') {
                    terminal.clear()?;
                } else if app.modal != app::ActiveModal::None {
                    if let Some(action) = event_modal::handle_modal_key(app, key) {
                        handle_modal_action(app, action);
                    }
                } else {
                    match event::handle_key(app, key) {
                        EventAction::Refresh => {
                            spawn_fetch(app, app.active_pane, Duration::ZERO);
                            if app.active_pane == Pane::Resources {
                                last_refresh = Instant::now();
                            }
                        }
                        EventAction::PaneSwitch | EventAction::None => {}
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

fn spawn_fetch(app: &mut App, pane: Pane, delay: Duration) {
    let Some(auth) = app.auth.clone() else {
        return;
    };

    let generation = app.begin_fetch(pane);
    if pane == Pane::Groups {
        app.group_status_message = "Authenticating with Graph API...".to_string();
    }

    let tx = app.bg_tx.clone();
    tokio::spawn(async move {
        if !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }
        match pane {
            Pane::Resources => {
                let result = resource_client(&auth).fetch_roles().await;
                let _ = tx.send(BgEvent::RolesLoaded {
                    generation,
                    result: result.map_err(|e| e.to_string()),
                });
            }
            Pane::Groups => {
                let result = group_client(&auth).fetch_group_roles().await;
                let _ = tx.send(BgEvent::GroupRolesLoaded {
                    generation,
                    result: result.map_err(|e| e.to_string()),
                });
            }
        }
    });
}

fn resource_client(auth: &AuthData) -> PimClient {
    PimClient::new(
        auth.http.clone(),
        auth.credential.clone(),
        auth.principal_id.clone(),
        auth.subscriptions.clone(),
    )
}

fn group_client(auth: &AuthData) -> GroupPimClient {
    GroupPimClient::new(
        auth.http.clone(),
        auth.graph_credential.clone(),
        auth.principal_id.clone(),
    )
}

fn spawn_fetch_permissions(app: &App) {
    let Some(auth) = app.auth.clone() else {
        return;
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
        let result = resource_client(&auth).fetch_role_permissions(ids).await;
        let _ = tx.send(BgEvent::RolePermissionsLoaded(
            result.map_err(|e| e.to_string()),
        ));
    });
}

fn already_active(err: &anyhow::Error) -> bool {
    matches!(
        err.downcast_ref::<PimError>(),
        Some(PimError::RoleAssignmentExists)
    )
}

async fn activate(
    auth: &AuthData,
    role: &PimRole,
    justification: &str,
    duration_hours: u32,
) -> Result<()> {
    let result = if role.role_type == RoleType::Resource {
        resource_client(auth)
            .activate_role(role, justification, duration_hours)
            .await
    } else {
        group_client(auth)
            .activate_group(role, justification, duration_hours)
            .await
    };
    match result {
        Err(e) if already_active(&e) => Ok(()),
        other => other,
    }
}

async fn deactivate(auth: &AuthData, role: &PimRole) -> Result<()> {
    if role.role_type == RoleType::Resource {
        resource_client(auth).deactivate_role(role).await
    } else {
        group_client(auth).deactivate_group(role).await
    }
}

fn handle_modal_action(app: &mut App, action: ModalAction) {
    let Some(auth) = app.auth.clone() else {
        return;
    };

    match action {
        ModalAction::Activate {
            pane,
            roles,
            justification,
            duration_hours,
        } => {
            for role in roles {
                let tx = app.bg_tx.clone();
                let justification = justification.clone();
                let auth = auth.clone();
                tokio::spawn(async move {
                    let result = activate(&auth, &role, &justification, duration_hours).await;
                    let _ = tx.send(BgEvent::ActivationResult {
                        pane,
                        key: role.key(),
                        result: result.map_err(|e| e.to_string()),
                    });
                });
            }
        }
        ModalAction::Deactivate { pane, role } => {
            let tx = app.bg_tx.clone();
            tokio::spawn(async move {
                let result = deactivate(&auth, &role).await;
                let _ = tx.send(BgEvent::DeactivationResult {
                    pane,
                    key: role.key(),
                    result: result.map_err(|e| e.to_string()),
                });
            });
        }
    }
}
