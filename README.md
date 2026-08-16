# Kumokara（雲殻）

> Persistent, agent-neutral shells in the browser.

[![Crates.io](https://img.shields.io/crates/v/kumokara.svg)](https://crates.io/crates/kumokara)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Kumokara is a lightweight, self-hosted terminal for long-running coding agents.
It keeps the terminal as the primary interface: open a workspace, create a
session, then run Claude Code, Codex, OpenCode, or any other CLI inside it.

![Kumokara workspace and session interface](docs/assets/kumokara-workspaces.png)

## Highlights

- **One-command installation** — the browser UI is bundled into the binary; a
  Cargo installation does not require Node.js or a source checkout.
- **Workspace navigation** — browser-local workspaces organize sessions by
  directory in a compact tree.
- **Persistent sessions** — each terminal is a server-owned PTY that stays alive
  when the browser disconnects or reconnects.
- **Agent neutral** — every CLI works through the generic terminal path instead
  of requiring a Kumokara-specific launcher.
- **Progressive awareness** — known agent processes are detected automatically,
  while adapters and hooks can provide richer state over time.
- **Comfortable terminal UI** — responsive workspace sidebar, light and dark
  themes, configurable terminal palettes, fonts, and font sizes.

Known-agent discovery currently supports Claude Code, Codex, OpenCode, Kimi
Code, and Mimo Code on macOS and Linux. Unknown tools remain fully usable as
ordinary terminal processes.

## Install

Install the latest release with Rust's Cargo:

```bash
cargo install kumokara
```

Then start Kumokara:

```bash
kumokara
```

Kumokara binds to `127.0.0.1:9876`, opens the browser, and creates a shell in
the directory where it was launched. Use **New Session** to open another shell
in the active workspace.

Rust stable is the only build prerequisite for the Cargo installation. Node.js
is only needed when developing the web interface from source.

Prebuilt archives for Linux x64/ARM64 and macOS Intel/Apple Silicon are also
available from [GitHub Releases](https://github.com/suxiaogang223/kumokara/releases).
Each release includes a `SHA256SUMS` file for integrity verification.

## Settings

Open **Settings** at the bottom of the workspace sidebar to choose `System`,
`Light`, or `Dark`, select separate light and dark terminal themes, and change
the terminal font family or size. These preferences and the workspace list are
stored in the current browser.

Kumokara uses a portable system monospace stack by default. If a prompt such as
Oh My Posh uses Nerd Font icons, install a Nerd Font and enter its exact family
name in Settings.

## Remote access

To listen on a remote interface, require token authentication explicitly:

```bash
kumokara server --bind 0.0.0.0:9876 --require-token
```

`--require-token` prints a random access token at startup. Put TLS or a trusted
reverse proxy in front of an internet-facing deployment, and never expose the
default no-auth mode to an untrusted network.

## How it works

```text
Browser attachments
        │ WebSocket
        ▼
SessionRegistry (runtime source of truth)
        ├── SessionInfo (cwd + optional detected agent)
        ├── AgentAdapterRegistry (built-ins + registered plugins)
        ├── PtySession (server-owned PTY)
        ├── bounded sequenced output history
        └── live output broadcast
```

The browser can detach and attach again. On attachment, Kumokara first replays
retained output and then switches to the live stream without a replay/live race.
Each browser fits its own xterm viewport locally; the focused browser owns the
active PTY resize.

Workspace navigation is browser-local: it stores directory paths and view
preferences without creating a second server runtime model. The canonical
session working directory remains the source of truth on the server.

The Rust workspace contains six focused crates:

- `kumokara-protocol`: client/server wire types;
- `kumokara-agent`: public adapter trait, registry, and built-in providers;
- `kumokara-engine`: PTY lifecycle, I/O, and ordered input/resize commands;
- `kumokara-auth`: token generation and validation;
- `kumokara-server`: session runtime and HTTP/WebSocket boundary;
- `kumokara`: local and remote server entry points.

## Project status

Kumokara `0.1.x` is an early public preview. The core local workflow is usable,
but APIs and protocol details may still change.

- Browser reconnect is supported while the Kumokara service keeps running.
- Restarting the service ends its PTY sessions. Agent context is resumed through
  each agent's own persisted-session mechanism.
- Process-based agent discovery is best-effort.
- Replay is bounded raw terminal output, not a server-side screen emulator.
- SSH targets, OAuth, notifications, and multi-user isolation are not yet
  implemented.

See [DESIGN.md](DESIGN.md) for the design and implementation boundaries.

## Build from source

Prerequisites: Rust stable and Node.js 20+.

```bash
git clone https://github.com/suxiaogang223/kumokara.git
cd kumokara
npm --prefix web ci
npm --prefix web run build
cargo run --release
```

Before submitting a change, run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm --prefix web run build
```

## License

[MIT](LICENSE) © Kumokara Contributors
