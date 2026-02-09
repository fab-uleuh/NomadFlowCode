# NomadFlow CLI

Interactive terminal UI for managing git worktrees and tmux sessions. Start a coding session on your phone, pick it up on desktop — or the other way around.

## Prerequisites

- **Node.js** >= 18
- **tmux** installed (`brew install tmux` on macOS)
- **NomadFlow server** running (`cd server && ./start.sh`)

## Install

```bash
cd cli
pnpm install
pnpm build
```

### Global link (optional)

To use `nomadflow` from anywhere:

```bash
pnpm link --global
```

Or run directly:

```bash
node dist/cli.js
```

## Usage

```bash
nomadflow                        # Interactive wizard (default)
nomadflow --status               # Quick status: tmux windows + processes
nomadflow --attach <feature>     # Attach directly to a feature
nomadflow --repo <name>          # Skip repo selection (use with --attach)
nomadflow --help                 # Show help
```

### Interactive wizard

Run `nomadflow` without arguments to enter the wizard:

1. **Resume** — If a previous session exists, offers to reattach
2. **Server** — Pick a server (skipped if only localhost)
3. **Repository** — Pick a repo from the server
4. **Feature** — Pick a feature or create a new one
5. **Attach** — Calls the API, then attaches to tmux

### Quick status

```bash
nomadflow --status
```

Shows all tmux windows with running processes:

```
Session: nomadflow
3 window(s)

  0: zsh                  idle
  1: myproject:add-login  idle
  2: myproject:refactor   ● claude
```

### Direct attach

```bash
nomadflow --attach add-login
nomadflow --attach add-login --repo myproject
```

Skips the wizard and attaches directly to a feature.

## Keyboard shortcuts

### In the CLI wizard

| Key         | Action              |
|-------------|---------------------|
| `Up` / `Down` | Navigate options  |
| `Enter`     | Confirm selection   |
| `Escape`    | Go back one step    |
| `q`         | Quit                |

### Inside tmux (after attach)

| Key             | Action                         |
|-----------------|--------------------------------|
| `Ctrl+b` `d`   | **Detach** (exit tmux, session keeps running) |
| `Ctrl+b` `c`   | New window                     |
| `Ctrl+b` `n`   | Next window                    |
| `Ctrl+b` `p`   | Previous window                |
| `Ctrl+b` `w`   | List windows                   |
| `Ctrl+b` `%`   | Split pane vertically          |
| `Ctrl+b` `"`   | Split pane horizontally        |
| `Ctrl+b` `o`   | Switch pane                    |
| `Ctrl+b` `z`   | Zoom/unzoom current pane       |
| `Ctrl+b` `[`   | Scroll mode (q to exit)        |
| `Ctrl+b` `x`   | Kill current pane              |

> On macOS, it's `Ctrl` (not `Cmd`).

## Configuration

Config file: `~/.nomadflowcode/config.toml`

```toml
[paths]
base_dir = "~/.nomadflowcode"

[tmux]
session = "nomadflow"

[ttyd]
port = 7681

[api]
port = 8080

[auth]
# secret = "your-secret-here"
```

The CLI also stores the last session in `~/.nomadflowcode/cli-state.json` for quick resume.

## Development

```bash
pnpm dev          # Watch mode (recompiles on save)
pnpm build        # One-shot build
pnpm start        # Run the CLI
```

## How mobile/desktop sync works

Both the mobile app and this CLI talk to the same NomadFlow server, which manages a single tmux session. When you select a feature:

1. CLI calls `POST /api/switch-feature` (creates/selects the tmux window)
2. CLI runs `tmux attach-session` (takes over your terminal)
3. You're in the exact same tmux window the mobile app connects to via ttyd

Start on mobile, finish on desktop. Or start on desktop, continue on mobile. tmux is the backbone — everything stays in sync.
