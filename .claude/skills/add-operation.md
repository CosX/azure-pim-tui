---
description: 'Add a new async background operation to the TUI. Use when: add new operation, new async task, new background action, new BgEvent, spawn async operation, add feature that calls Azure ARM or Graph API.'
---

# Add Background Operation

Async work runs in `tokio::spawn` tasks and reports back over `app.bg_tx` as a `BgEvent`. The main loop drains `bg_rx` every tick and calls `App::handle_bg_event`, the only place results change state.

## Files touched

| Step | File | What |
|------|------|------|
| 1 | `src/client/pim.rs` or `src/client/group.rs` | API method returning `anyhow::Result<T>` (see `azure-api` skill) |
| 2 | `src/app.rs` | `BgEvent` variant and its arm in `handle_bg_event` |
| 3 | `src/event.rs` / `src/event_modal.rs` | Trigger: return `EventAction` or `ModalAction` |
| 4 | `src/main.rs` | Spawn the task from `run_app` or `handle_modal_action` |

## Rules

**Identify targets by key and pane.** A result may arrive after the user switched panes or after a refresh re-sorted the list. Carry `pane: Pane` and `key: RoleKey` in the event and resolve with `app.find_role(pane, &key)`. Never carry a `usize` index.

**Fetches use generations.** Call `app.begin_fetch(pane)` and include the returned `generation` in the event. `handle_bg_event` drops results whose generation is not current, so overlapping fetches can't overwrite newer data.

**Build clients from `AuthData`.** Use `resource_client(&auth)` / `group_client(&auth)` in `main.rs`. They share one `reqwest::Client` (`auth.http`). Don't call `reqwest::Client::new()`.

**Send HTTP through `client::http::send_with_retry`.** It retries 429 responses with `Retry-After`.

**Refresh after mutations from the result, not the request.** `run_app` collects panes from `ActivationResult`/`DeactivationResult` and calls `spawn_fetch(app, pane, ACTION_REFRESH_DELAY)`. Do the same for new mutating operations.

**Convert errors to `String` at the channel boundary** (`result.map_err(|e| e.to_string())`), after mapping known API error codes to typed `PimError` variants.

## Skeleton

```rust
let tx = app.bg_tx.clone();
let auth = auth.clone();
tokio::spawn(async move {
    let result = resource_client(&auth).do_thing(&role).await;
    let _ = tx.send(BgEvent::ThingDone {
        pane,
        key: role.key(),
        result: result.map_err(|e| e.to_string()),
    });
});
```

## Checklist

- [ ] Event carries `Pane` + `RoleKey` (or a generation for fetches)
- [ ] Status message set via the pane's status (`status_message` / `group_status_message`)
- [ ] Unit test in `app.rs` `tests` for the state transition
