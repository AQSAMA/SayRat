# SayRat

> An open-source, ultra-lightweight keyboard launcher for desktop power users —
> instant cold-starts, near-zero input latency, and a sandboxed WebAssembly
> plugin ecosystem, all aimed at a sub-20 MB total memory footprint.

SayRat is Wayland-first and dual-process by design: a long-lived background
daemon owns state, indexing, and Wasm plugin orchestration, while an ephemeral
overlay client renders a frameless Slint window and forwards keystrokes.

## Architecture

```
+--------------------------------------------------------------+
|                     sayratd (Daemon)                         |
|  - Lifecycle / State Engine      - Background Indexer        |
|  - Wasmtime Plugin Sandbox       - redb / fjall Database     |
+--------------------------------------------------------------+
                               |
                               | [IPC: interprocess + postcard]
                               v
+--------------------------------------------------------------+
|                    sayrat-ui (Client)                        |
|  - Slint Declarative UI          - Input Capture Only        |
|  - wlr-layer-shell Overlay       - PagedResultModel          |
+--------------------------------------------------------------+
```

(Source: [`AGENTS.md` §1](./AGENTS.md#1-system-architecture--boundaries).)

## Workspace layout

| Crate                       | Kind   | Role                                              |
| --------------------------- | ------ | ------------------------------------------------- |
| `crates/sayrat-protocol`    | lib    | Shared IPC types exchanged over the local socket. |
| `crates/sayratd`            | bin    | Background daemon: state, indexer, Wasm host.     |
| `crates/sayrat-ui`          | bin    | Slint overlay client + keystroke forwarder.       |

## Build & run

Requires a current stable Rust toolchain (1.85+ for edition 2024).

```sh
# Format + lint + build everything
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --release

# Run the Phase 1 stubs
./target/release/sayratd   --version
./target/release/sayrat-ui --version

./target/release/sayratd   --socket /tmp/sayrat.sock
./target/release/sayrat-ui --socket /tmp/sayrat.sock
```

Each crate also builds independently:

```sh
cargo build -p sayrat-protocol
cargo build -p sayratd
cargo build -p sayrat-ui
```

## License

[GPL-3.0-or-later](./LICENSE). Every source file carries an
`SPDX-License-Identifier: GPL-3.0-or-later` header.

## For AI contributors

[`AGENTS.md`](./AGENTS.md) is the governing reference for architecture
boundaries, the approved technology stack, memory budgets, and code-style
constraints. Read it before opening a PR; CI will reject contributions
that violate the rules listed there.
