---
description: 'Add or modify a UI panel, widget, or rendering component in the TUI. Use when: add panel, add widget, modify layout, new UI section, render new view, change detail view, add table column, modify role list, TUI rendering, ratatui widget.'
---

# Add UI Panel / Widget

## Layout

```
┌ Azure PIM TUI vX · user ─┬ Resources  Groups (Tab) ┬──── Loading… ┐  title (3 rows)
├──────────────────────────┴───────────┬─────────────┴──────────────┤
│ role_list (70%)                      │ detail (30%)               │  content
├──────────────────────────────────────┴────────────────────────────┤
│ status_bar (1 row)                                                │
└───────────────────────────────────────────────────────────────────┘
```

`ui::layout::areas(size)` computes every region. Both `render` and `event_mouse` use it, so click targets always match what's drawn. Add new regions to `Areas` rather than splitting a `Rect` inside a panel's renderer.

## Rules

- Renderers are `pub fn render(f: &mut Frame, area: Rect, app: &App)` and only read state. The one exception is `app.list_offset` (`Cell<usize>`), which the role list updates because the viewport height is only known at draw time.
- Anything clickable gets a hit-test function next to its renderer (`row_at`, `in_selector_column`, `pane_tab_at`, `activate_field_at`), sharing constants with the draw code.
- Colours: Resources pane uses Cyan, Groups uses Magenta. Status: Green active, Yellow activating/loading, Red failed, Gray eligible. Scope tints come from `SCOPE_PALETTE` (256-colour indexed, works without truecolour).
- Long or API-supplied text in paragraphs should use `.wrap(Wrap { trim: false })`.
- Terminal escape injection: ratatui's buffer drops control characters, so API strings can be rendered directly. Don't write raw strings to stdout outside ratatui.

## Checklist

- [ ] New region added to `layout::Areas`
- [ ] Hit-test helpers have unit tests (see `role_list.rs` `tests`)
- [ ] Looks right at 80x24 and on a wide terminal
