# Azure PIM TUI

Activate Azure PIM roles from your terminal. No portal clicking, no context switching.

![Rust](https://img.shields.io/badge/rust-2021-orange) ![License](https://img.shields.io/badge/license-MIT-blue)

## What it does

- Finds all your eligible PIM roles across subscriptions and groups
- Activate or deactivate roles, one at a time or in bulk
- Shows which roles are active and how long they have left
- Displays role permissions in a side-by-side detail panel
- Filter by name or switch between all/eligible/active views
- Reads justification and duration defaults from a config file

## Prerequisites

- [Azure CLI](https://learn.microsoft.com/en-us/cli/azure/install-azure-cli) — logged in via `az login`

## Install

### Homebrew (macOS / Linux)

```bash
brew install CosX/tap/azure-pim-tui
```

### Winget (Windows)

```bash
winget install CosX.AzurePimTui
```

### Chocolatey (Windows)

```bash
choco install azure-pim-tui
```

### cargo-binstall (pre-built binary)

```bash
cargo binstall azure-pim-tui
```

### cargo install (build from source)

```bash
cargo install azure-pim-tui
```

### Pre-built binaries

Grab a binary for your platform from the [latest release](https://github.com/CosX/azure-pim-tui/releases/latest).

### Build from source

```bash
git clone https://github.com/CosX/azure-pim-tui.git
cd azure-pim-tui
cargo install --path .
```

## Usage

```bash
az login
azure-pim-tui
```

## Keybindings

| Key | Action |
|-----|--------|
| `j` / `k` / arrows | Navigate |
| `g` / `G` | Jump to first / last |
| `a` / `Enter` | Activate role |
| `d` | Deactivate role |
| `Space` | Toggle selection for bulk ops |
| `A` | Bulk activate selected |
| `r` / `F5` | Refresh |
| `/` | Search by name |
| `v` | Cycle view: all / eligible / active |
| `Ctrl+d` / `Ctrl+u` | Scroll detail panel |
| `?` | Help |
| `q` / `Ctrl+C` | Quit |

In the activation modal: `Tab` switches fields, `Enter` confirms, `Esc` cancels.

### Mouse

| Action | Effect |
|--------|--------|
| Click row | Select role |
| Double-click row | Activate role |
| Click `●` column | Toggle selection for bulk ops |
| Click `Resources` / `Groups` tab | Switch pane |
| Scroll wheel | Move selection in role list / scroll detail panel |
| Click field in activation modal | Focus field |

Mouse capture disables the terminal's native text selection. Hold `Shift` (or `Option` in iTerm2/Terminal.app) while dragging to select text, or set `mouse = false` in the config.

## Configuration

A config file is created on first run at `azure-pim-tui/config.toml` in your OS config directory:

| OS | Path |
|----|------|
| Linux | `~/.config/azure-pim-tui/config.toml` |
| macOS | `~/Library/Application Support/azure-pim-tui/config.toml` |
| Windows | `%APPDATA%\azure-pim-tui\config.toml` |

```toml
default_justification = "Local development"
default_duration_hours = 8
auto_refresh_secs = 60
mouse = true
```

## How it works

1. Authenticates with your existing `az login` session (nothing stored)
2. Queries eligible roles and active assignments across all your subscriptions
3. Fetches role permissions in the background so you can see what each role allows
4. Activations and deactivations hit the ARM or Graph API depending on the role type

API calls use scoped endpoints with `assignedTo()` filters, so group-based eligibility works correctly.

## License

MIT
