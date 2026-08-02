# ☁ Kumokara（雲殻）

> **Agents never sleep in Kumokara.**
>
> A self-hosted Agent Development Environment — persistent workspaces, 24/7 agent runtime, accessible from any browser.

[![CI](https://github.com/suxiaogang223/kumokara/actions/workflows/ci.yml/badge.svg)](https://github.com/suxiaogang223/kumokara/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.88%2B-orange.svg)](https://www.rust-lang.org)

---

## What is Kumokara?

Kumokara（雲殻 — "cloud shell"）flips the traditional terminal model on its head. Instead of you opening a terminal to run commands, Kumokara is a **persistent home for AI agents** — they run 24/7 in the cloud (or on your local machine), and you check in from any device's browser to review progress, assign new tasks, and approve critical decisions.

```
传统终端： 人 → 打开终端 → 输入命令 → 关终端 → 进程终止
Kumokara： Agent → 常驻云壳 → 24h 运行 → 你随时通过浏览器视察
```

### Positioning

| | Local/Desktop | Web/Cloud |
|---|---|---|
| **Traditional Terminal** | iTerm, Kitty, Warp | ttyd, wetty |
| **Agent-Native Terminal** | Otty | **★ Kumokara (ADE)** |
| **Cloud IDE** | Cursor, Windsurf | Replit, Codespaces |

Kumokara's unique combination: **Self-hosted + Agent 24/7 online + Web access from any device.**

---

## Features

- **Workspace** — Project-level workspaces with isolated filesystems, environment variables, and agent configs
- **Session** — 0..N terminal sessions per workspace (shell or agent); sessions survive browser close
- **Agent Integration** — Run Claude Code / Codex / OpenCode in persistent PTY sessions with state reporting via hooks
- **Shell Observability** — OSC 133/7 injection parses terminal output into structured events (command boundaries, exit codes, cwd tracking)
- **Crash Recovery** — tmux-wrapped PTYs survive server restarts; agent `--resume` restores context
- **Web Terminal** — Full xterm.js terminal in the browser, with workspace sidebar, session tabs, and status badges
- **Local & Remote** — Same codebase, same protocol; run locally with one command or deploy to a VPS

---

## Quick Start

### Prerequisites

- **Rust** 1.88+
- **Node.js** 20+
- **tmux** (optional, for session recovery)

### Run Locally

```bash
# Clone the repo
git clone https://github.com/suxiaogang223/kumokara.git
cd kumokara

# Build frontend
cd web && npm install && npm run build && cd ..

# Start Kumokara (Local mode — auto-opens browser)
cargo run
```

```
$ cargo run

  _  __                     __
 | |/ /_  _  _ __ ___   ___| | ____ _ _ __ __ _
 | ' /| || || | '  \ _ \ / _ \ |/ / _` | '__/ _` |
 | . \ \_,_||_|_|_|_\___/\___/_/\_\__,_|_|  \__,_|
 |_|\_\

 Kumokara（雲殻）— Agents never sleep in Kumokara.

⚠ tmux not found — session recovery disabled. Install tmux for 24h agent persistence.
✓ Workspace directory: ~/.kumokara
→ Token: e2e7bbb2195ad89b...
→ Server listening on http://127.0.0.1:9876
→ Opening browser...
```

Then open `http://localhost:9876`, enter the token, create a workspace, and click **+ New Shell** to open a terminal.

### Server Mode (VPS / Home Server)

```bash
kumokara server --bind 0.0.0.0:9876
```

---

## Architecture

```
┌──────────────────────────────────────────────────────┐
│                    CLIENTS                            │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐           │
│  │ Desktop  │  │  Mobile  │  │  Tablet  │           │
│  │ Browser  │  │ Browser  │  │ Browser  │           │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘           │
│       └──────────────┼──────────────┘                │
│                      │ WebSocket + REST               │
└──────────────────────┼──────────────────────────────┘
                       │
┌──────────────────────┼──────────────────────────────┐
│              Kumokara Server (axum)                   │
│  ┌───────────────────┴───────────────────┐           │
│  │         Auth / API Gateway             │           │
│  └───────────────────┬───────────────────┘           │
│  ┌───────────────────┴───────────────────┐           │
│  │         Workspace Manager              │           │
│  │  ┌──────────────────────────────────┐ │           │
│  │  │  Workspace                       │ │           │
│  │  │  ├── Sessions (tmux/portable PTY)│ │           │
│  │  │  ├── Event Bus                   │ │           │
│  │  │  ├── Event Log (SQLite)          │ │           │
│  │  │  └── Output Ring Buffer          │ │           │
│  │  └──────────────────────────────────┘ │           │
│  └───────────────────────────────────────┘           │
│                                                       │
│  ┌──────────────────────────────────────────────────┐│
│  │ PTY Output Pipeline                               ││
│  │ tmux %output → OSC 133/7 Parser → Structured Events││
│  └──────────────────────────────────────────────────┘│
│  ┌───────────────┐  ┌───────────────┐                │
│  │ OSC Parser    │  │ Agent Hooks   │                │
│  │ (shellint)    │  │ Receiver      │                │
│  └───────────────┘  └───────────────┘                │
└──────────────────────────────────────────────────────┘
```

### Tech Stack

| Layer | Choice |
|---|---|
| Server | **Rust** + tokio + axum |
| PTY | **portable-pty** (wezterm) + tmux control mode |
| Database | **SQLite** via sqlx (WAL mode) |
| Protocol | WebSocket (JSON control + binary terminal I/O) |
| Frontend | **React** + Vite + TypeScript |
| Terminal | **xterm.js** + fit/webgl addons |
| State | **zustand** |
| CLI | **clap** |
| Auth | Token-based (Phase 0) → OAuth (Phase 3) |

### Crate Map

```
crates/
├── kumokara-protocol/     # Shared types & WebSocket messages
├── kumokara-engine/       # PTY management (portable-pty + tmux)
├── kumokara-shellint/     # OSC 133/7 parser + shell scripts
├── kumokara-event/        # Event bus + SQLite log + ring buffer
├── kumokara-workspace/    # Workspace & session lifecycle
├── kumokara-auth/         # Token authentication
├── kumokara-agent/        # AgentProvider trait + CLI wrappers
├── kumokara-ssh/          # SSH connector (Phase 3)
├── kumokara-server/       # Axum HTTP + WebSocket server
└── kumokara-cli/          # CLI entry point
```

---

## Development Phases

| Phase | Status | Focus |
|---|---|---|
| **0 — Bootstrap** | ✅ Done | Cargo workspace, PTY engine, WS server, workspace CRUD, web prototype, E2E tests |
| **1 — Agent Runtime** | 🚧 Next | Claude Code provider, agent lifecycle, shell integration injection, tmux session wrap |
| **2 — Orchestration** | 📋 Planned | Agent state detection, approval UI, multi-workspace, notifications, History view |
| **3 — Remote & Access** | 📋 Planned | SSH targets, OAuth, TLS, Codex/OpenCode providers, full crash recovery, session IPC |
| **4 — Platform** | 📋 Planned | Tauri app, multi-user, plugin marketplace, mobile PWA, workspace templates |

---

## Data Directory

```
~/.kumokara/
├── config.yaml              # Global config (token, settings)
├── kumokara.db              # Global database
├── workspaces/
│   └── {workspace_id}/
│       ├── workspace.yaml   # Workspace config (env vars, agent settings)
│       ├── files/           # Working directory
│       ├── events.db        # Structured event log (SQLite)
│       └── sessions/
│           └── {session_id}.log
└── auth/
```

Backup = back up `~/.kumokara/`.

---

## License

MIT © Kumokara Contributors

---

*雲（cloud）+ 殻（shell）— the shell in the cloud where agents never sleep.*
