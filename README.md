# NomadFlow

**Resilient mobile terminal with AI assistant for nomad development**

NomadFlow is an open source mobile application (React Native) that provides access to a resilient remote terminal, optimized for mobile development with AI assistance.

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Platform](https://img.shields.io/badge/platform-iOS%20%7C%20Android-lightgrey.svg)
![React Native](https://img.shields.io/badge/React%20Native-0.73-61dafb.svg)

## Features

### Ultra Smooth Workflow
- **3-step selection**: Server → Repo → Feature → Terminal ready!
- **Zero manual commands**: the environment is automatically configured
- **Pre-launched AI agent**: Claude, Ollama or your custom agent awaits you

### Mobile App
- **iOS and Android compatible** via React Native
- **Integrated xterm.js terminal** with native rendering
- **Session persistence** thanks to tmux
- **Offline mode** with local cache of recent selections

### Secure Connection
- **Secure WebSocket (WSS)** to your server
- **Shared secret authentication**: protects API and terminal
- **Smart auto-reconnection** with backoff

### Environment Management
- **Git worktrees**: one branch = one isolated environment
- **Persistent tmux sessions** per feature
- **Server scripts** to automate creation/cleanup

## Screenshots

```
┌─────────────────────────────────────────┐
│  Servers                    Settings    │
├─────────────────────────────────────────┤
│                                         │
│  ┌─────────────────────────────────┐   │
│  │ My Dev Server                   │   │
│  │    wss://192.168.1.100:7681     │   │
│  │    Connected 5 min ago          │   │
│  └─────────────────────────────────┘   │
│                                         │
│  ┌─────────────────────────────────┐   │
│  │ Cloud Server                    │   │
│  │    wss://dev.example.com        │   │
│  │    Never connected              │   │
│  └─────────────────────────────────┘   │
│                                         │
│                              [+]        │
└─────────────────────────────────────────┘
```

## Quick Start

### CLI Installation

**macOS / Linux:**
```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/fab-uleuh/NomadFlowCode/releases/latest/download/nomadflow-installer.sh | sh
```

**Windows (PowerShell):**
```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/fab-uleuh/NomadFlowCode/releases/latest/download/nomadflow-installer.ps1 | iex"
```

**From source (requires Rust):**
```bash
git clone https://github.com/fab-uleuh/NomadFlowCode.git
cd NomadFlowCode/nomadflow-rs
cargo install --path .
```

### Usage

```bash
# Launch the TUI wizard (server + interactive interface)
nomadflow

# Launch HTTP server in foreground (headless/Docker mode)
nomadflow serve

# Start the server as a background daemon
nomadflow start

# Stop the background daemon
nomadflow stop

# Display tmux and daemon status
nomadflow --status

# Attach directly to a session
nomadflow --attach <feature>
```

The server handles **graceful shutdown**: on Ctrl+C or SIGTERM, it stops accepting new connections, completes in-flight requests, then cleanly stops the ttyd subprocess (no orphan processes).

### Configuration

```bash
# The configuration file is automatically created on first launch
nano ~/.nomadflowcode/config.toml
```

### Mobile Side

1. **Clone the repo**:
```bash
git clone https://github.com/fab-uleuh/NomadFlowCode.git
cd NomadFlowCode
```

2. **Install dependencies**:
```bash
npm install
# or
yarn install
```

3. **iOS**:
```bash
cd ios && pod install && cd ..
npm run ios
```

4. **Android**:
```bash
npm run android
```

## Prerequisites

### Server
- Linux/macOS with SSH access
- **ttyd** (web terminal)
- **tmux** (terminal multiplexer)
- **Git** with worktree support
- Optional: **Ollama**, **Claude CLI**, or other AI agent

### Mobile
- Node.js 18+
- React Native CLI
- Xcode (iOS) or Android Studio (Android)

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Mobile App                           │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌────────┐ │
│  │ Servers  │→ │  Repos   │→ │ Features │→ │Terminal│ │
│  └──────────┘  └──────────┘  └──────────┘  └────────┘ │
│       │                                         │      │
│       └──────────── WebSocket ──────────────────┘      │
└────────────────────────┬────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────┐
│                    Server                               │
│  ┌──────────────────────────────────────────────────┐  │
│  │                    ttyd                           │  │
│  │         (WebSocket → PTY bridge)                  │  │
│  └──────────────────────┬───────────────────────────┘  │
│                         │                               │
│  ┌──────────────────────▼───────────────────────────┐  │
│  │                    tmux                           │  │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐          │  │
│  │  │ Window1 │  │ Window2 │  │ Window3 │  ...     │  │
│  │  │feature-a│  │feature-b│  │  main   │          │  │
│  │  └────┬────┘  └────┬────┘  └────┬────┘          │  │
│  └───────┼────────────┼────────────┼────────────────┘  │
│          │            │            │                    │
│  ┌───────▼────┐ ┌─────▼──────┐ ┌──▼───┐               │
│  │  Worktree  │ │  Worktree  │ │ Main │               │
│  │ feature-a  │ │  feature-b │ │ Repo │               │
│  └────────────┘ └────────────┘ └──────┘               │
│                                                        │
│  ┌──────────────────────────────────────────────────┐  │
│  │              AI Agent (Claude/Ollama)            │  │
│  └──────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

## Project Structure

```
NomadFlowCode/
├── nomadflow-rs/               # Rust binary (single binary: server + TUI)
│   ├── src/main.rs             # Entry point, CLI, daemon mode
│   ├── crates/
│   │   ├── nomadflow-core/     # Config, models, shell, git/tmux/ttyd services
│   │   ├── nomadflow-server/   # Axum HTTP server with auth middleware
│   │   └── nomadflow-tui/      # Ratatui TUI wizard
│   └── Cargo.toml
├── nomadflowcode/              # React Native/Expo mobile app
├── docs/                       # Documentation site (Next.js/fumadocs)
└── README.md
```

## Configuration

### Server Configuration (`~/.nomadflowcode/config.toml`)

```toml
[paths]
base_dir = "~/.nomadflowcode"

[tmux]
session = "nomadflow"

[ttyd]
port = 7681

[api]
port = 8080

# Authentication - uncomment to enable
# The same secret must be entered in the mobile app
# [auth]
# secret = "your-secret-here"
```

### App Configuration (in-app)

- **AI Agent**: Claude, Ollama, or custom command
- **Auto-launch agent**: enable/disable
- **Tmux session prefix**: customizable
- **Theme**: Dark, Light, or System
- **Font size**: 10-24px
- **Auto-reconnection**: with parameters

## Security

### Shared Secret Authentication

NomadFlow uses a single shared secret that protects both:
- **The REST API**: via Bearer token (Authorization header)
- **The ttyd terminal**: via Basic Auth (user: `nomadflow`, password: secret)

#### Setup

1. **Server side** (`~/.nomadflowcode/config.toml`):
```toml
[auth]
secret = "your-secure-secret"
```

2. **Mobile side**: enter the same secret in the "Authentication Secret" field when configuring the server.

#### How It Works

- **Without secret**: everything works without authentication (local development)
- **With secret**: the API returns 401 without the correct Bearer token, and ttyd requires credentials

### Recommendations

1. **Use HTTPS/WSS** in production
2. **Enable authentication** with a strong secret
3. **Firewall**: do not expose ports 7681/8080 publicly without a VPN
4. **SSL certificates**: Let's Encrypt or self-signed certificates

## tmux Shortcuts

The app includes overlay buttons for common tmux shortcuts:

| Shortcut | Action |
|-----------|--------|
| `Ctrl-b w` | List windows |
| `Ctrl-b c` | New window |
| `Ctrl-b n` | Next window |
| `Ctrl-b p` | Previous window |
| `Ctrl-b "` | Horizontal split |
| `Ctrl-b %` | Vertical split |
| `Ctrl-b d` | Detach |
| `Ctrl-b [` | Scroll mode |

## Contributing

Contributions are welcome!

1. Fork the project
2. Create your branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

MIT License - see [LICENSE](LICENSE) for more details.

## Acknowledgements

- [ttyd](https://github.com/tsl0922/ttyd) - Web terminal
- [xterm.js](https://xtermjs.org/) - Terminal emulator
- [tmux](https://github.com/tmux/tmux) - Terminal multiplexer
- [React Native](https://reactnative.dev/) - Mobile framework

---

**Made with love for nomad developers**
