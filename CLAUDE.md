# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Run

```bash
cargo build          # compile
cargo run            # launch TUI (requires `az login` or `azd auth login` session)
```

No test suite exists yet. No linter or formatter configuration beyond default `rustfmt`/`clippy`.

## Architecture

Async event-driven TUI using ratatui + crossterm + tokio. The app runs a 250ms tick loop that polls crossterm keyboard events and a tokio mpsc channel for background task results.

### Layers

- **main.rs** — Terminal setup/teardown, event loop, spawns async tasks. Routes keyboard events to `event.rs` (normal mode) or `event_modal.rs` (modal mode). Owns `spawn_fetch()` and `handle_modal_action()`.
- **app.rs** — `App` struct holds all state: roles, selection, filters, modal, auth data (`Arc<AuthData>`). `BgEvent` enum carries results from background tasks back to the main loop. `handle_bg_event()` is the state transition handler.
- **event.rs / event_modal.rs** — Keyboard dispatch split by context. Normal mode handles navigation, activation triggers, filter/view cycling. Modal mode handles field editing, Tab navigation, Enter/Esc.
- **client/** — Azure API layer. `auth.rs` shells out to `az` CLI (3 parallel `spawn_blocking` calls) for token, principal ID, and subscription list. `pim.rs` makes REST calls to Azure Management API using reqwest with `.query()` for proper URL encoding. `models.rs` has serde structs matching the Azure PIM `2020-10-01` API response shapes.
- **ui/** — Stateless rendering functions. `layout.rs` composes the 4-panel layout (title, role table, detail, status bar) and overlays modals. Each panel is a separate module.

### Key Data Flow

1. **Auth** — Startup spawns `get_auth_info()` which uses `azure_identity::DeveloperToolsCredential` (chains Azure CLI + Azure Developer CLI) to get a token. Principal ID and display name are extracted from JWT claims (`oid`, `upn`). Subscriptions are fetched via REST (`/subscriptions`). The credential object is stored in `Arc<AuthData>` and shared across tasks — it handles token refresh automatically.
2. **Fetch** — `PimClient::fetch_roles()` queries `roleEligibilitySchedules` and `roleAssignmentScheduleInstances` endpoints **per subscription** in parallel using `futures::future::join_all`. Results merge into `Vec<PimRole>` with active status overlay. Each API call gets a fresh token from the credential.
3. **Activate/Deactivate** — Spawns per-role tokio tasks that PUT to `roleAssignmentScheduleRequests`. Uses the **user's** principal ID (from JWT `oid` claim), not the group principal ID from eligibility responses — this distinction matters for group-based PIM eligibility.

### Azure PIM API Details

- Scoped queries only: `{scope}/providers/Microsoft.Authorization/...` — tenant-level endpoints return InsufficientPermissions.
- Filter: `assignedTo('{principalId}') and atScope()` — handles group membership.
- Activation body requires `principalId` = the signed-in user, `linkedRoleEligibilityScheduleId` = the eligibility schedule ID from discovery.
- Known API errors handled: `RoleAssignmentExists` (already active), `ActiveDurationTooShort` (can't deactivate yet).

## Config

`~/.config/azure-pim-tui/config.toml` — auto-created with defaults on first run:

```toml
default_justification = "Local development"
default_duration_hours = 8
auto_refresh_secs = 60
```
