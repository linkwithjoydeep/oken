<p align="center">
  <img src="assets/oken-icon.svg" width="120" />
</p>

<h1 align="center">oken</h1>
<p align="center">A smarter SSH CLI — fully backward-compatible, zero config required.</p>

<p align="center">
  <a href="#installation">Install</a> ·
  <a href="#quick-start">Quick Start</a> ·
  <a href="#features">Features</a> ·
  <a href="#configuration">Configuration</a> ·
  <a href="#command-reference">Commands</a>
</p>

---

`oken` is a drop-in replacement for the `ssh` command. It passes every flag and argument through to the system SSH binary unchanged, so anything that works with `ssh` works with `oken` — no migration, no learning curve.

On top of that, it adds the features that `ssh` has never had: a fuzzy host picker, connection history, auto-reconnect, keep-alive injection, production host warnings, named tunnel profiles, and an audit log. All opt-in, all skippable.

---

## Why oken?

**You're already using SSH every day.** The friction is in finding the right host, typing `user@10.0.1.something` from memory, losing your session to a flaky network, or accidentally running a command on prod when you meant staging.

`oken` removes that friction without changing how SSH works:

- **No host to remember** — open the picker, type a few letters, connect.
- **No session lost to network hiccups** — auto-reconnect brings you back automatically.
- **No silent idle timeouts** — keep-alive is injected by default.
- **No accidental prod commands** — tagged production hosts prompt for confirmation.
- **No complex tunnel management** — save a tunnel profile once, start it with one word.

---

## Installation

> **Prerequisite:** `oken` delegates all SSH work to your system's `ssh` binary — it does not implement SSH itself. Make sure OpenSSH is installed before using `oken`. It is pre-installed on macOS and most Linux distributions. On Windows, enable it via *Settings → Optional Features → OpenSSH Client*, or install [Git for Windows](https://gitforwindows.org) which bundles it.

### macOS and Linux

```bash
curl -LsSf https://github.com/linkwithjoydeep/oken/releases/latest/download/oken-installer.sh | sh
```

This downloads the correct binary for your platform and installs it to `~/.cargo/bin/oken` (or `~/.local/bin/oken` if Cargo is not present).

### Windows

```powershell
powershell -c "irm https://github.com/linkwithjoydeep/oken/releases/latest/download/oken-installer.ps1 | iex"
```

### Direct download

Pre-built archives for every platform are attached to each [GitHub Release](https://github.com/linkwithjoydeep/oken/releases/latest):

| Platform | File |
|---|---|
| macOS Apple Silicon | `oken-aarch64-apple-darwin.tar.xz` |
| macOS Intel | `oken-x86_64-apple-darwin.tar.xz` |
| Linux arm64 | `oken-aarch64-unknown-linux-gnu.tar.xz` |
| Linux x86_64 | `oken-x86_64-unknown-linux-gnu.tar.xz` |
| Windows x86_64 | `oken-x86_64-pc-windows-msvc.zip` |

Each archive contains the `oken` binary, `LICENSE`, and `README.md`. SHA-256 checksums are provided alongside every file.

### From source

Requires [Rust](https://rustup.rs) 1.80 or later.

```bash
cargo install --git https://github.com/linkwithjoydeep/oken
```

### Updating

Run the same installer command again — it always fetches the latest release:

```bash
# macOS and Linux
curl -LsSf https://github.com/linkwithjoydeep/oken/releases/latest/download/oken-installer.sh | sh

# Windows
powershell -c "irm https://github.com/linkwithjoydeep/oken/releases/latest/download/oken-installer.ps1 | iex"
```

`oken` also checks for new versions automatically once every 24 hours and prints a one-line notice when one is available. The check runs in the background and never delays a connection.

### Optional: alias as `ssh`

```bash
# ~/.zshrc or ~/.bashrc
alias ssh=oken
```

`oken` detects when it is aliased as `ssh` and skips itself when searching for the SSH binary, so this is completely safe.

---

## Quick Start

```bash
# Open the interactive fuzzy picker — no arguments needed
oken

# Connect to a host by name (exact match connects directly)
oken prod-web

# Works exactly like ssh — all flags pass through unchanged
oken -p 2222 -i ~/.ssh/deploy_key ubuntu@10.0.1.50

# Save a host so it appears in the picker
oken host add prod-web ubuntu@10.0.1.50 --tag prod

# Connect to any host tagged "prod" — picks directly if one match, opens filtered picker otherwise
oken --tag prod
```

---

## Features

### Fuzzy Host Picker

Run `oken` with no arguments to open the interactive picker. Type to filter in real time against host aliases, hostnames, usernames, and tags. Hosts are sorted by recency — the ones you connect to most appear first.

```
  Search: █                                          12 / 12 hosts
 ─────────────────────────────────────────────────────────────────
  prod
> prod-web         ubuntu@10.0.1.50       [prod]        2h ago
  prod-db          deploy@10.0.1.51       [prod]        3d ago
  staging
  staging-web      ubuntu@10.0.2.10       [staging]     1w ago
  other
  dev-laptop       joy@192.168.1.5        []
```

Hosts are grouped visually by tag. Use `↑` / `↓` to navigate, `Enter` to connect, `Esc` to cancel.

**Tag filter:** prefix your search with `#` to filter exclusively by tag.

```
  Search: #prod█                                      2 / 12 hosts
```

### Pre-filtered from the command line

Pass a partial name to open the picker pre-filtered, or connect directly if only one host matches:

```bash
oken prod         # opens picker filtered to "prod"
oken prod-web     # connects directly — only one match
```

### Automatic Host Saving

When you connect to an unknown `user@host` for the first time, `oken` asks if you want to save it:

```
Looks like a new host. Save ubuntu@10.0.1.50 so it shows up in the picker?
Save as (Enter = 10.0.1.50, "n" to skip): prod-web
Tags (comma-separated, Enter to skip): prod, aws
Saved host 'prod-web'
```

Skippable with Enter. Never intrusive.

### Auto-Reconnect

Dropped connections reconnect automatically. `oken` detects SSH exit code 255 (connection error) and retries with a countdown:

```
Connection lost. Reconnecting (1/3)…
```

Disable per-session with `--no-reconnect`, or configure retries and delay in `~/.config/oken/config.toml`.

### Keep-Alive

`ServerAliveInterval` and `ServerAliveCountMax` are injected into every SSH session by default, preventing idle timeouts silently dropping your connection. The interval is configurable. If you set `ServerAliveInterval` yourself, `oken` won't override it.

### Production Host Warnings

Tag a host as `prod` or `production` and `oken` shows a warning banner before connecting:

```
⚠  WARNING: 'prod-db' is tagged [prod]
Continue? [y/N]
```

Skip the prompt with `--yes` for scripting. Configure which tags are "dangerous" in `~/.config/oken/config.toml`.

### Named Tunnel Profiles

Save tunnel configurations by name and start them with a single command:

```bash
# Save a tunnel profile
oken tunnel add db-tunnel -L 5432:localhost:5432 prod-db

# Start it in the background (uses SSH ControlMaster)
oken tunnel start db-tunnel

# Check what's running
oken tunnel list

# Stop it
oken tunnel stop db-tunnel

# Remove a saved profile
oken tunnel remove db-tunnel
```

Tunnel state is tracked via SSH ControlMaster sockets — no PID files, no daemons. If a tunnel fails to start, the error from SSH is shown immediately.

### Print Resolved SSH Command

Useful for scripting, debugging, or sharing the exact command `oken` would run:

```bash
$ oken print prod-web
/usr/bin/ssh -o ServerAliveInterval=60 -o ServerAliveCountMax=3 ubuntu@10.0.1.50
```

### Audit Log

Every connection is appended to `~/.local/share/oken/audit.log`. View recent history with:

```bash
oken audit          # last 50 connections
oken audit -n 100   # last 100
```

```
TIME                 ALIAS          TARGET              DURATION  EXIT
2026-02-28 10:42:01  prod-web       ubuntu@10.0.1.50    42m 07s   0
2026-02-28 09:15:33  prod-db        deploy@10.0.1.51    5m 02s    0
2026-02-27 18:03:11  staging-web    ubuntu@10.0.2.10    3s        255
```

### Shell Completions

`oken completions` auto-detects your shell from `$SHELL`, finds (or creates) the right completion directory, and writes the file:

```bash
oken completions
# Installed zsh completions → /Users/you/.zfunc/_oken
```

For non-standard setups, point it at the right directory explicitly:

```bash
oken completions --dir ~/.config/zsh/.zfunc
```

Force a specific shell regardless of `$SHELL`:

```bash
oken completions --shell bash
```

**zsh:** the file is written as `_oken`. If the target directory isn't a well-known fpath location, `oken` prints the exact `fpath=...` line to add to your `.zshrc`.

**bash / fish:** completions are written to the standard auto-sourced directories — no extra config required.

---

## Host Management

```bash
# Add a host
oken host add <name> <user@host> [--port N] [--key path] [--tag tag1 tag2]

# Examples
oken host add prod-web   ubuntu@10.0.1.50  --tag prod
oken host add prod-db    deploy@10.0.1.51  --port 2222 --tag prod db
oken host add dev-laptop joy@192.168.1.5

# List all saved hosts
oken host list

# Remove a host
oken host remove prod-web

# Open hosts.toml in $EDITOR
oken host edit
```

Hosts are stored in `~/.config/oken/hosts.toml` alongside your existing `~/.ssh/config`. Both sources are merged automatically, with `hosts.toml` winning on conflicts.

`oken host list` shows all hosts from both sources. Hosts from `~/.ssh/config` are marked `ssh config` and are read-only — `oken host remove` and `oken host edit` will reject them with a message pointing you to the right file.

---

## Configuration

`~/.config/oken/config.toml` — all fields are optional, shown here with their defaults:

```toml
# Auto-reconnect on dropped connections
reconnect            = true
reconnect_retries    = 3
reconnect_delay_secs = 5

# SSH keep-alive interval in seconds
keepalive_interval   = 60

# Tags that trigger a confirmation prompt before connecting
danger_tags          = ["prod", "production"]
```

To see the currently active configuration (defaults merged with your overrides):

```bash
oken config
```

---

## Command Reference

```
oken [OPTIONS] [SSH_ARGS]...
oken <COMMAND>

Options:
  --tag <TAG>     Filter by tag — connect directly if one match, open picker otherwise
  --yes           Skip production-host confirmation prompts
  --no-reconnect  Disable auto-reconnect for this session

Commands:
  host                    Manage saved hosts
    host add <name> <user@host> [--port N] [--key path] [--tag tag1 tag2]
    host list
    host remove <name>
    host edit

  tunnel                  Manage tunnel profiles
    tunnel add <name> [ssh-flags] <host>
    tunnel start <name>
    tunnel stop  <name>
    tunnel remove <name>
    tunnel list

  print <host>            Print the resolved SSH command for a host
  audit [-n N]            View last N connection log entries (default 50)
  config                  Show active configuration values
  update                  Check for a newer version
  completions [--shell <shell>] [--dir <dir>]
                          Install shell completions
```

---

## File Locations

| Path | Purpose |
|---|---|
| `~/.config/oken/hosts.toml` | Saved host definitions |
| `~/.config/oken/tunnels.toml` | Named tunnel profiles |
| `~/.config/oken/config.toml` | Settings (reconnect, keep-alive, danger tags) |
| `~/.local/share/oken/history.db` | Connection history (used for picker sorting) |
| `~/.local/share/oken/audit.log` | Append-only connection audit log |
| `~/.local/share/oken/update_state` | Cached update check result (timestamp + latest version) |

All paths respect `$XDG_CONFIG_HOME` and `$XDG_DATA_HOME`.

---

## Compatibility

`oken` delegates all SSH work to your system's `ssh` binary. It does not implement SSH itself.

| Capability | Status |
|---|---|
| All `ssh` flags | Passed through unchanged |
| `~/.ssh/config` | Always read and respected |
| `known_hosts` verification | Handled by system SSH |
| SSH agent (`SSH_AUTH_SOCK`) | Handled by system SSH |
| Exit codes | Match OpenSSH exactly |
| `alias ssh=oken` | Safe — oken skips itself when locating ssh |
| Non-interactive / scripting | Exact alias matches bypass all UI |

---

## License

MIT
