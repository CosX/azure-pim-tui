---
description: 'Add a new async background operation to the TUI using the sentinel dispatch pattern. Use when: add new operation, new async task, new background action, sentinel dispatch, new BgEvent, spawn async operation, add feature that calls Azure Service Bus API.'
---

# Add Async Operation

Adds a new background operation using the sentinel dispatch pattern. This is the central workflow for any feature that calls the Azure Service Bus API (or any async work) from the TUI.

## Architecture

The TUI uses a sync/async hybrid event loop:
1. `event.rs` handles keystrokes and sets `app.status_message` to a **sentinel string**
2. `main.rs` matches sentinel strings each tick and spawns `tokio::spawn` tasks
3. Spawned tasks send results via `app.bg_tx` (unbounded `mpsc` channel)
4. `main.rs` drains `app.bg_rx.try_recv()` and applies results to `App` state

## Files Touched (in order)

| Step | File | What |
|------|------|------|
| 1 | `src/app.rs` | Add `BgEvent` variant for the result type |
| 2 | `src/client/management.rs` or `src/client/data_plane.rs` | Add client method |
| 3 | `src/event.rs` | Add key handler that sets sentinel via `app.set_status("MyOp...")` |
| 4 | `src/main.rs` | Match sentinel → spawn task → send `BgEvent` |
| 5 | `src/main.rs` | Handle `BgEvent` variant in `bg_rx.try_recv()` match block |
| 6 | `src/ui/help.rs` | Document the new keybinding in the help overlay |

## Procedure

### Step 1: Define the BgEvent variant

In `src/app.rs`, add a variant to the `BgEvent` enum. Use a descriptive name and include all data the main loop needs to update state.

```rust
pub enum BgEvent {
    // ... existing variants ...
    MyOperationComplete {
        result_field: String,
    },
}
```

**Rules:**
- If the operation returns large data, `Box` it (see `DetailLoaded(Box<DetailView>)`)
- If the operation can partially succeed, include error counts (see `ResendComplete`)
- Progress updates reuse the existing `Progress(String)` variant

### Step 2: Add the client method

Add the API call in the appropriate client module:
- Management plane (ATOM XML CRUD) → `src/client/management.rs`
- Data plane (messages, peek, send, purge) → `src/client/data_plane.rs`

The client structs are `Clone` (they hold `reqwest::Client` + `ConnectionConfig`). They are cloned into spawned tasks.

**Rules:**
- Return `Result<T>` using the crate's `ServiceBusError`
- Management API uses PascalCase paths (`/Subscriptions/`), data plane uses lowercase (`/subscriptions/`)
- Use `self.config.namespace_token().await?` for auth headers

### Step 3: Add the key handler + sentinel

In `src/event.rs`, add the key handler in the appropriate function:
- `handle_tree_input()` — tree panel keybindings
- `handle_message_input()` — message panel keybindings
- `handle_modal_input()` — modal overlay keybindings
- `handle_detail_edit_input()` — inline edit keybindings

Set a **unique** sentinel string:

```rust
KeyCode::Char('X') => {
    if app.bg_running {
        app.set_status("A background operation is in progress...");
    } else if let Some((path, entity_type)) = app.selected_entity() {
        // Guard: only valid entity types
        match entity_type {
            EntityType::Queue | EntityType::Subscription | EntityType::Topic => {
                app.set_status("My operation...");
            }
            _ => {
                app.set_status("Select a valid entity");
            }
        }
    }
}
```

**Critical rules:**
- Always guard with `if app.bg_running` to prevent concurrent ops
- The sentinel string MUST be unique across all sentinels (check existing ones in `main.rs`)
- For operations that need user input first, open a modal instead of setting the sentinel directly

### Step 4: Match sentinel and spawn in main.rs

In `src/main.rs`, add a sentinel match block in `run_app()` after the existing sentinel blocks:

```rust
// My operation (spawned)
if app.status_message == "My operation..." && app.data_plane.is_some() {
    if let Some((path, entity_type)) = app.selected_entity() {
        let dp = app.data_plane.clone().unwrap();
        let entity_path = path.to_string();
        let is_topic = *entity_type == EntityType::Topic;
        let tx = app.bg_tx.clone();

        app.set_status("Running...");

        // For topic fan-out pattern, see "Topic Fan-Out" section below
        tokio::spawn(async move {
            match dp.my_method(&entity_path).await {
                Ok(result) => {
                    let _ = tx.send(BgEvent::MyOperationComplete {
                        result_field: result,
                    });
                }
                Err(e) => {
                    let _ = tx.send(BgEvent::Failed(format!("Operation failed: {}", e)));
                }
            }
        });
    }
}
```

**Critical rules:**
- Clone `data_plane`/`management` client before moving into spawn
- Clone `app.bg_tx` for sending results
- For cancellable long-running operations, use `app.new_cancel_token()` and set `app.bg_running = true`
- Always handle `Err` by sending `BgEvent::Failed`

### Step 5: Handle the BgEvent result

In the `while let Ok(event) = app.bg_rx.try_recv()` block in `main.rs`:

```rust
BgEvent::MyOperationComplete { result_field } => {
    app.set_status(format!("Operation complete: {}", result_field));
    app.bg_running = false;
    // Update any relevant app state here
    needs_refresh = true; // if tree counts may have changed
}
```

### Step 6: Document the keybinding

Add the new keybinding to `src/ui/help.rs` in the appropriate section.

## Topic Fan-Out Pattern

If the operation can target a Topic, it must fan out across all subscriptions. Use `resolve_purge_paths()` or follow this pattern:

```rust
if is_topic {
    let mgmt = app.management.as_ref().cloned();
    tokio::spawn(async move {
        if let Some(mgmt) = mgmt {
            match mgmt.list_subscriptions(&entity_path).await {
                Ok(subs) => {
                    for s in &subs {
                        let sub_path = format!("{}/subscriptions/{}", entity_path, s.name);
                        // Perform operation on each sub_path
                    }
                }
                Err(e) => {
                    let _ = tx.send(BgEvent::Failed(format!("Failed: {}", e)));
                    return;
                }
            }
        }
    });
}
```

For DLQ operations on topics, append `/$deadletterqueue` to each subscription path. For sends, route to the parent topic using `send_path()`.

## Existing Sentinels (avoid collision)

| Sentinel | Operation |
|----------|-----------|
| `"Peeking messages..."` | Peek active/DLQ messages |
| `"Refreshing..."` | Reload entity tree |
| `"Submitting..."` | Send message / create entity / edit entity (disambiguated by modal) |
| `"Deleting..."` | Delete entity |
| `"Bulk resending..."` | Bulk resend from DLQ |
| `"Bulk deleting..."` | Bulk delete messages |
| `"Clearing (delete)..."` | Purge active messages |
| `"Clearing (delete DLQ)..."` | Purge DLQ messages |
| `"Clearing (resend)..."` | Resend all DLQ messages |
| `"Discovering namespaces..."` | Azure AD namespace discovery |
| `"Copying messages..."` | Copy messages to another entity |

## Checklist

- [ ] `BgEvent` variant added to `app.rs`
- [ ] Client method added and returns `Result<T>`
- [ ] Key handler guards `bg_running` and validates entity type
- [ ] Sentinel string is unique (no collision with table above)
- [ ] Spawn block clones clients and `bg_tx` before `move`
- [ ] Error path sends `BgEvent::Failed`
- [ ] `BgEvent` handled in `bg_rx` drain loop
- [ ] `bg_running` reset to `false` in result handler
- [ ] Topic fan-out handled (if applicable)
- [ ] Help overlay updated with new keybinding
