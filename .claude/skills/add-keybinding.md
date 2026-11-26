---
description: 'Add or modify a keyboard shortcut in the TUI. Use when: add keybinding, add shortcut, add key handler, new keyboard command, add hotkey, modify key mapping, input handling, key event.'
---

# Add Keybinding

## Input routing

Keys flow through this chain. A key consumed earlier never reaches later stages.

```
main.rs run_app
├── KeyEventKind != Press           → ignored
├── Ctrl+L                          → terminal.clear() (global)
├── app.modal != None               → event_modal::handle_modal_key() → Option<ModalAction>
└── otherwise                       → event::handle_key() → EventAction
    ├── Ctrl+C                      → quit
    ├── Ctrl+D / Ctrl+U             → detail scroll
    ├── app.filtering               → handle_filter_key()
    └── main match on key.code      → navigation, actions, pane/view switches
```

Mouse events go through `event_mouse::handle_mouse()` instead. If a key has a mouse equivalent, both must call the same `App` method.

## Files touched

| Step | File | What |
|------|------|------|
| 1 | `src/event.rs` or `src/event_modal.rs` | Match arm |
| 2 | `src/app.rs` | State change as an `App` method, not inline logic |
| 3 | `src/main.rs` | Only if the key needs async work: return an `EventAction` variant and handle it in `run_app` |
| 4 | `src/ui/help.rs` | Help overlay line |
| 5 | `README.md` | Keybindings table |

## Rules

- Put state changes in `App` methods (`open_activate_modal`, `open_deactivate_modal`, `select`, `switch_pane`) so keyboard and mouse share them.
- Refer to roles by `RoleKey`, never by index into `roles`/`group_roles`. Background refreshes replace and re-sort those vectors at any time.
- `event.rs` must not spawn tasks. Return an `EventAction` and let `main.rs` spawn.
- Check the key is free in every stage of the chain above, including filter mode, where any `Char` is text input.

## Checklist

- [ ] No conflict with existing bindings (`?` shows them all)
- [ ] Works in both panes or is guarded by `app.active_pane`
- [ ] Help overlay and README updated
- [ ] `cargo fmt --all` and clippy pass
