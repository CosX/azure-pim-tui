---
description: 'Add or modify a UI panel, widget, or rendering component in the TUI. Use when: add panel, add widget, modify layout, new UI section, render new view, change detail view, add tree column, modify message list, TUI rendering, ratatui widget.'
---

# Add UI Panel / Widget

Adds or modifies TUI rendering components using ratatui. The project uses a 3-panel layout with modal overlays.

## Layout Structure

```
┌─────────────── Title Bar ────────────────────┐
│ Service Bus Explorer — namespace              │
├──────────┬───────────────────────────────────┤
│          │          Detail (40%)              │
│ Tree     ├───────────────────────────────────┤
│ (30%)    │          Messages (60%)           │
│          │                                    │
├──────────┴───────────────────────────────────┤
│ Status Bar                                    │
└──────────────────────────────────────────────┘
```

Layout is defined in `src/ui/layout.rs`. Each panel is a separate render function.

## Module Map

| Module | Renders | Has App State? |
|--------|---------|----------------|
| `ui/layout.rs` | Top-level layout constraints + dispatch | No — calls other modules |
| `ui/tree.rs` | Entity tree with inline counts | Uses `app.flat_nodes`, `app.tree_selected` |
| `ui/detail.rs` | Entity properties/runtime info | Uses `app.detail_view` |
| `ui/messages.rs` | Message list + detail view + inline edit | Uses `app.messages`, `app.dlq_messages`, etc. |
| `ui/modals.rs` | Modal overlays | Uses `app.modal`, `app.input_fields` |
| `ui/status_bar.rs` | Bottom status bar | Uses `app.status_message` |
| `ui/help.rs` | Help overlay (`?` key) | Static content |
| `ui/sanitize.rs` | Terminal escape stripping | Pure function |

## Procedure

### Adding a new render section within an existing panel

1. Read the target panel's module (e.g., `ui/detail.rs`)
2. Add your rendering inside the existing `render_*` function
3. Use ratatui's `Layout` to subdivide the panel area

### Adding a new standalone panel

**Step 1:** Create the module in `src/ui/`:

```rust
// src/ui/my_panel.rs
use ratatui::prelude::*;
use ratatui::widgets::*;
use ratatui::Frame;

use crate::app::App;
use crate::ui::sanitize::sanitize_for_terminal;

pub fn render_my_panel(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" My Panel ")
        .border_style(Style::default().fg(Color::White));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Render content inside `inner`
}
```

**Step 2:** Register in `src/ui/mod.rs`:

```rust
pub mod my_panel;
```

**Step 3:** Add to layout in `src/ui/layout.rs`:

```rust
use super::my_panel::render_my_panel;

// Adjust constraints and call render_my_panel(frame, app, area)
```

**Step 4:** If the panel is focusable, add `FocusPanel` variant in `app.rs` and update Tab cycling in `event.rs`.

## Rendering Conventions

### Security: sanitize untrusted content

**Always** call `sanitize_for_terminal()` before rendering Service Bus message bodies or custom properties. These come from external systems and may contain terminal escape sequences.

```rust
use crate::ui::sanitize::sanitize_for_terminal;

let safe_body = sanitize_for_terminal(&msg.body, true);  // allow newlines
let safe_prop = sanitize_for_terminal(&value, false);     // no newlines
```

### Focus indication

Active panel borders use `Color::Cyan`, inactive use `Color::DarkGray`:

```rust
let border_color = if app.focus == FocusPanel::MyPanel {
    Color::Cyan
} else {
    Color::DarkGray
};
```

### Scrollable lists

Use ratatui's `ListState` or `TableState` for stateful scrolling. The app already maintains `app.message_list_state` (ListState) and `app.tree_table_state` (TableState).

### Layout splitting

```rust
let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
        Constraint::Length(3),   // fixed height
        Constraint::Min(1),      // fill remaining
        Constraint::Percentage(30), // proportional
    ])
    .split(area);
```

### Styling patterns used in this project

| Element | Style |
|---------|-------|
| Active panel border | `Color::Cyan` |
| Inactive panel border | `Color::DarkGray` |
| Title bar | `bg(Blue).fg(White).bold()` |
| Status message | `Color::Yellow` |
| Error message | `Color::Red` |
| Selected item | `bg(DarkGray).fg(White)` or `add_modifier(Modifier::REVERSED)` |
| Destructive action | `Color::Red` |
| Count badges | `Color::Green` (active), `Color::Red` (DLQ) |

## Checklist

- [ ] Render function takes `(frame: &mut Frame, app: &App, area: Rect)`
- [ ] Untrusted content passed through `sanitize_for_terminal()`
- [ ] Focus indication uses `Color::Cyan` / `Color::DarkGray` pattern
- [ ] Module registered in `ui/mod.rs`
- [ ] Layout updated in `ui/layout.rs` (if new panel)
- [ ] `FocusPanel` variant added + Tab cycling updated (if focusable)
- [ ] Panel handler added in `event.rs` (if interactive)
