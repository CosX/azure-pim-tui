# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Run

```bash
cargo build          # compile
cargo run            # launch TUI (requires `az login` or `azd auth login` session)
```

No test suite exists yet.

## Lint & Format

Run both of these after every code change:

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings -A dead_code -A unused
```

Both must pass cleanly before committing. CI enforces the same checks.

## Available Skills

- `.claude/skills/azure-api.md` — Add a new Azure API call (ARM or Graph)
- `.claude/skills/add-operation.md` — Add a new async background operation using the sentinel dispatch pattern
- `.claude/skills/add-keybinding.md` — Add or modify a keyboard shortcut
- `.claude/skills/add-modal.md` — Add a new modal dialog overlay
- `.claude/skills/add-ui-panel.md` — Add or modify a UI panel or rendering component
- `.claude/skills/code-review.md` — Structured production-grade code review
- `.claude/skills/rust-check.md` — Check code against Rust standards for this project
- `.claude/skills/security-check.md` — Security-focused evaluation of code

## Architecture

Async event-driven TUI using ratatui + crossterm + tokio. The app runs a 250ms tick loop that polls crossterm keyboard events and a tokio mpsc channel for background task results.

### Layers

- **main.rs** — Terminal setup/teardown, event loop, spawns async tasks. Routes keyboard events to `event.rs` (normal mode) or `event_modal.rs` (modal mode). Owns `spawn_fetch()` and `handle_modal_action()`.
- **app.rs** — `App` struct holds all state: roles, selection, filters, modal, auth data (`Arc<AuthData>`). `BgEvent` enum carries results from background tasks back to the main loop. `handle_bg_event()` is the state transition handler.
- **event.rs / event_modal.rs** — Keyboard dispatch split by context. Normal mode handles navigation, activation triggers, filter/view cycling. Modal mode handles field editing, Tab navigation, Enter/Esc.
- **client/** — Azure API layer. `auth.rs` uses `azure_identity::DeveloperToolsCredential` for auth. `pim.rs` makes REST calls to Azure Management API for resource roles. `group.rs` makes REST calls to Microsoft Graph API for PIM for Groups. `models.rs` has serde structs for both ARM (`2020-10-01`) and Graph API response shapes.
- **ui/** — Stateless rendering functions. `layout.rs` composes the 4-panel layout (title, role table, detail, status bar) and overlays modals. Each panel is a separate module.

### Key Data Flow

1. **Auth** — Startup spawns `get_auth_info()` which uses `azure_identity::DeveloperToolsCredential` (chains Azure CLI + Azure Developer CLI) to get a token. Principal ID and display name are extracted from JWT claims (`oid`, `upn`). Subscriptions are fetched via REST (`/subscriptions`). The credential object is stored in `Arc<AuthData>` and shared across tasks — it handles token refresh automatically.
2. **Fetch** — `spawn_fetch()` runs resource and group fetches in parallel. `PimClient::fetch_roles()` queries ARM `roleEligibilitySchedules` + `roleAssignmentScheduleInstances` per subscription. `GroupPimClient::fetch_group_roles()` queries Graph `identityGovernance/privilegedAccess/group/eligibilityScheduleInstances` + `assignmentScheduleInstances`. Results merge into a single `Vec<PimRole>` with `RoleType` discriminator.
3. **Activate/Deactivate** — Routes by `RoleType`: Resource roles PUT to ARM `roleAssignmentScheduleRequests`, Group roles POST to Graph `privilegedAccess/group/assignmentScheduleRequests`. Both use the user's principal ID from JWT `oid` claim.

### Azure PIM API Details

**Resource Roles (ARM):**
- Scoped queries only: `{scope}/providers/Microsoft.Authorization/...` — tenant-level endpoints return InsufficientPermissions.
- Filter: `assignedTo('{principalId}') and atScope()` — handles group membership.
- Activation body requires `principalId` = the signed-in user, `linkedRoleEligibilityScheduleId` = the eligibility schedule ID from discovery.
- Known API errors handled: `RoleAssignmentExists` (already active), `ActiveDurationTooShort` (can't deactivate yet).

**PIM for Groups (Graph):**
- Endpoint: `https://graph.microsoft.com/v1.0/identityGovernance/privilegedAccess/group/...`
- Filter: `principalId eq '{userId}'`
- `accessId` field: `"member"` or `"owner"` — determines group membership vs ownership activation.
- Group display names resolved via batch `GET /groups?$filter=id in (...)` calls.
- Gracefully returns empty if tenant doesn't support PIM for Groups (400/403/404).

## Config

`~/.config/azure-pim-tui/config.toml` — auto-created with defaults on first run:

```toml
default_justification = "Local development"
default_duration_hours = 8
auto_refresh_secs = 60
```
