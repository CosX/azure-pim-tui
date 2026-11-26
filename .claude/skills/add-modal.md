---
description: 'Add a new modal dialog overlay to the TUI. Use when: add modal, add dialog, add popup, add form modal, add confirmation dialog, new overlay, new input dialog, add prompt dialog, user input form.'
---

# Add Modal Dialog

Modals are variants of `ActiveModal` in `src/app.rs`. While `app.modal != ActiveModal::None`, keys go to `event_modal::handle_modal_key` and mouse clicks to `event_mouse::handle_modal_mouse`. `ui/layout.rs` draws the modal over the main layout.

## Files touched

| Step | File | What |
|------|------|------|
| 1 | `src/app.rs` | `ActiveModal` variant + `App::open_*_modal()` method |
| 2 | `src/ui/modals.rs` | Render arm + draw function |
| 3 | `src/event_modal.rs` | Key handling, returns `Option<ModalAction>` |
| 4 | `src/event_mouse.rs` | Click handling in `handle_modal_mouse`, if any |
| 5 | `src/main.rs` | `handle_modal_action` arm, if the modal submits async work |
| 6 | `src/app.rs` | `drop_stale_modal` arm if the modal references roles |

## Rules

- Store `RoleKey` (or `Vec<RoleKey>`) in the variant, never list indices. Auto-refresh runs while modals are open and re-sorts the role vectors.
- Render by looking up the role with `app.find_role(app.active_pane, key)`. If it returns `None`, render nothing: `drop_stale_modal` closes the modal on the next fetch.
- On submit, resolve keys to cloned `PimRole`s inside `handle_modal_key` and put those in the `ModalAction`. Re-check the status (`can_activate`, `is_active`) at that point.
- Use `centered_rect` for placement. If you add clickable rows, add a hit-test function next to the renderer (see `activate_field_at`) that uses the same area and line offsets.
- `Esc` always closes. `Enter` confirms.

## Checklist

- [ ] Variant holds keys, not indices
- [ ] Hint line in the modal lists its keys
- [ ] `drop_stale_modal` handles the new variant
- [ ] Test in `app.rs` `tests` that the modal survives a reordering refresh
