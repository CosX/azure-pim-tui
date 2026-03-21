mod app;
mod client;
mod config;
mod event;
mod event_modal;
mod ui;

use std::io;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::{
    event::{Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::{ActiveModal, App, AuthData, BgEvent};
use client::auth;
use client::pim::PimClient;
use event::EventAction;
use event_modal::ModalAction;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging to stderr (so it doesn't interfere with TUI)
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
                let _ = tx.send(BgEvent::AuthReady(Ok(AuthData {
                    credential: info.credential,
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

    loop {
        terminal.draw(|f| ui::layout::render(f, app))?;

        // Process background events (non-blocking)
        while let Ok(bg_event) = app.bg_rx.try_recv() {
            let should_fetch = matches!(bg_event, BgEvent::AuthReady(Ok(_)));
            app.handle_bg_event(bg_event);
            if should_fetch {
                needs_fetch = true;
            }
        }

        // Trigger fetch if needed
        if needs_fetch {
            spawn_fetch(app);
            needs_fetch = false;
        }

        // Auto-refresh timer
        if last_refresh.elapsed() >= auto_refresh && app.auth.is_some() && !app.loading {
            spawn_fetch(app);
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
                            spawn_fetch(app);
                            last_refresh = Instant::now();
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

fn spawn_fetch(app: &App) {
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
        match client.fetch_roles().await {
            Ok(roles) => {
                let _ = tx.send(BgEvent::RolesLoaded(Ok(roles)));
            }
            Err(e) => {
                let _ = tx.send(BgEvent::RolesLoaded(Err(e.to_string())));
            }
        }
    });
}

fn handle_modal_action(app: &mut App, action: ModalAction) {
    let auth = match &app.auth {
        Some(a) => a.clone(),
        None => return,
    };

    match action {
        ModalAction::Activate {
            indices,
            justification,
            duration_hours,
        } => {
            for idx in indices {
                let tx = app.bg_tx.clone();
                let role = app.roles[idx].clone();
                let just = justification.clone();
                let auth = auth.clone();

                tokio::spawn(async move {
                    let client = PimClient::new(
                        auth.credential.clone(),
                        auth.principal_id.clone(),
                        auth.subscriptions.clone(),
                    );
                    match client.activate_role(&role, &just, duration_hours).await {
                        Ok(()) => {
                            let _ = tx.send(BgEvent::ActivationResult {
                                index: idx,
                                result: Ok(()),
                            });
                        }
                        Err(e) => {
                            let _ = tx.send(BgEvent::ActivationResult {
                                index: idx,
                                result: Err(e.to_string()),
                            });
                        }
                    }
                });
            }
        }
        ModalAction::Deactivate { index } => {
            let tx = app.bg_tx.clone();
            let role = app.roles[index].clone();

            tokio::spawn(async move {
                let client = PimClient::new(
                    auth.credential.clone(),
                    auth.principal_id.clone(),
                    auth.subscriptions.clone(),
                );
                match client.deactivate_role(&role).await {
                    Ok(()) => {
                        let _ = tx.send(BgEvent::DeactivationResult {
                            index,
                            result: Ok(()),
                        });
                    }
                    Err(e) => {
                        let _ = tx.send(BgEvent::DeactivationResult {
                            index,
                            result: Err(e.to_string()),
                        });
                    }
                }
            });
        }
    }
}
