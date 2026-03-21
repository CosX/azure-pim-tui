# Azure PIM TUI

A terminal UI for managing Azure Privileged Identity Management (PIM) role activations. Discover all your eligible roles across subscriptions and activate them without leaving the terminal.

![Rust](https://img.shields.io/badge/rust-2021-orange) ![License](https://img.shields.io/badge/license-MIT-blue)

## Features

- **Auto-discovery** — finds all eligible PIM roles across every subscription in your tenant
- **Activate / deactivate** — single role or bulk select multiple roles
- **Live status** — shows active roles with countdown timers, auto-refreshes in the background
- **Filter & search** — cycle views (all/eligible/active) or search by name
- **Configurable defaults** — justification and duration pre-filled from config

## Prerequisites

- [Rust toolchain](https://rustup.rs/)
- [Azure CLI](https://learn.microsoft.com/en-us/cli/azure/install-azure-cli) (`az`) — authenticated via `az login`

## Install

```bash
git clone https://github.com/your-user/azure-pim-tui.git
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
| `j` / `k` / `↑` / `↓` | Navigate |
| `g` / `G` | First / last |
| `a` / `Enter` | Activate selected role |
| `d` | Deactivate selected role |
| `Space` | Toggle selection (for bulk) |
| `A` | Bulk activate all selected |
| `r` / `F5` | Refresh |
| `/` | Filter by name |
| `v` | Cycle view: all / eligible / active |
| `?` | Help |
| `q` / `Ctrl+C` | Quit |

### Activation modal

| Key | Action |
|-----|--------|
| `Tab` | Switch between justification / duration fields |
| `Enter` | Confirm activation |
| `Esc` | Cancel |

## Configuration

Config is created automatically on first run at `~/.config/azure-pim-tui/config.toml`:

```toml
default_justification = "Local development"
default_duration_hours = 8
auto_refresh_secs = 60
```

## How it works

1. Authenticates using your existing `az login` session (no credentials stored)
2. Queries `roleEligibilitySchedules` across all subscriptions to discover eligible roles
3. Queries `roleAssignmentScheduleInstances` to determine which are currently active
4. Activations/deactivations go through the `roleAssignmentScheduleRequests` API

All API calls use scoped endpoints with `assignedTo()` filters, which correctly resolves group-based PIM eligibility.

## License

MIT
