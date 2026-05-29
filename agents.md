# AI Agent Project Directives: Project SayRat

**Project Vision:** SayRat is an open-source, ultra-lightweight keyboard launcher for desktop power users. It relies on a Wayland-first, Wasm-extensible, dual-process architecture written in Rust, targeting a strict sub-20MB total memory footprint and instant cold-starts.

This document serves as the absolute source of truth for all AI agents, coding assistants (e.g., Cursor, Aider, Copilot), and automated pipelines interacting with this codebase.

---

## 1. System Architecture & Boundaries

The application is strictly decoupled into two separate processes. AI agents must never bridge these boundaries except via the defined IPC layer.

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

### 1.1 Background Daemon (`sayratd`)
* **Role:** State management, filesystem tracking, fuzzy search execution, and plugin orchestration.
* **Lifecycle:** Runs continuously as a background user service (systemd user unit / native daemon).
* **Storage:** Leverages a lightweight embedded key-value store (`redb` or `fjall`) for caching applications, desktop entries, and indexing history.

### 1.2 UI Client (`sayrat-ui`)
* **Role:** Frameless overlay rendering and keystroke capture.
* **Lifecycle:** Ephemeral but persistent in memory. To avoid startup latency, it remains initialized but suspended/hidden until triggered by a global hotkey.
* **State Limitation:** The client is entirely "dumb". It must not perform filesystem scanning, database queries, or plugin logic. It forwards keystrokes to the daemon and visualizes incoming chunks.

---

## 2. Hardened Technology Stack

Agents must not add arbitrary dependencies. Only the following approved crates and tools should be utilized:

| Subsystem | Approved Technology | Implementation Constraints |
| :--- | :--- | :--- |
| **Core Language** | Rust (Latest Stable) | Idiomatic, zero-copy architecture where applicable. MSRV: latest stable, reviewed quarterly. |
| **UI Framework** | `slint` | Declarative UI DSL compiled natively down to bare metal. |
| **IPC Transport** | `interprocess` | Local Unix domain sockets (Linux/macOS) / Named Pipes (Windows). |
| **Serialization** | `postcard` (primary) / `bincode` (approved fallback) | Strictly typed, compact binary format with zero-copy features. New code should default to `postcard`. |
| **Fuzzy Matching**| `nucleo` (primary) / `frizbee` (approved fallback) | Lock-free concurrent streaming or SIMD-accelerated matching. |
| **Wasm Runtime** | `wasmtime` (primary) / `extism` (approved fallback) | WASI execution environment configured for lowest possible overhead. Note: `extism` wraps `wasmtime` and constrains the plugin manifest format to the Extism PDK; choose deliberately. |
| **Database** | `redb` (primary) / `fjall` (approved fallback) | Pure Rust, ACID-compliant, low-footprint alternative to SQLite. |
| **Wayland Native**| `smithay-client-toolkit` + `wayland-protocols` | Layer-shell window positioning and surface lifecycle. |
| **Global Hotkeys**| `global-hotkey` | Native hooks with `zbus` (D-Bus) fallback for Wayland compositors. |
| **CLI Parsing** | `pico-args` | Minimal argv parsing for the daemon/client flags (`--socket`, `--version`). No proc-macros; chosen over `clap` to protect code size. |
| **Logging** | `log` + `env_logger` | Lightweight logging facade + env-driven backend. `env_logger` must be declared `default-features = false` (no `humantime`/`jiff`, no `regex`) to protect the footprint budget. `tracing` may be layered later behind a feature flag if structured spans are needed. |

---

## 3. Strict Non-Functional Requirements & Budgets

All code generation and structural changes must validate against these explicit performance baselines:

* **Memory Budgets:**
  * UI Client (`sayrat-ui`): **< 5MB RAM** active footprint.
  * Background Daemon (`sayratd`): **< 15MB RAM** idle footprint.
  * *Total System Target:* **< 20MB RAM**.
* **Latency Profile:**
  * **Input-to-Render Latency:** < 16ms (Targeting a stable 60Hz frame budget for UI updates upon keystroke).
  * **Cold Start / Wake Latency:** Imperceptible (instant UI draw).
* **Security & Isolation:**
  * Wasm plugins must execute under a strict **Zero-Ambient-Authority Capability Model**.
  * Network access, host filesystem access, and environment variable access must be denied by default, and explicitly injected only if declared in the plugin manifest.

---

## 4. Architectural Edge-Cases & Approved Mitigations

When generating code for subsystems, agents must follow these explicit mitigation patterns to resolve inherent platform limitations:

### 4.1 Slint VectorModel Bottleneck (UI Stuttering)
* **Problem:** Slint's `VectorModel` triggers wholesale `row_data_changed` notifications when updating underlying collections. Injecting thousands of matching search results directly causes massive rendering frame-drops.
* **Mandatory Solution:** Implement a paged/chunked results architecture.
  1. `sayratd` streams results back over IPC in fixed chunks (e.g., maximum of 50 matches per packet).
  2. `sayrat-ui` implements a custom `PagedResultModel` or updates a fixed-size buffer. It renders the first visible page instantly.
  3. Incremental rows are requested and appended down-stream as the user scrolls, avoiding model resets.

### 4.2 Wasm Runtime Memory Bloat
* **Problem:** Instantiating a default Wasmtime engine with Cranelift compilation can consume 30-50MB of RAM instantly due to internal buffers, code cache, and allocator structures.
* **Mandatory Solution:**
  1. Configure `wasmtime` with a **Pooling Allocator** to explicitly cap maximum per-instance resource consumption.
  2. Set compiler optimizations to `OptLevel::None` at runtime if dynamic loading is needed, or ideally use **Ahead-of-Time (AOT) Pre-compilation** to compile `.wasm` modules into wasmtime's serialized `.cwasm` (compiled module) format during plugin installation, bypassing JIT memory overhead entirely.

### 4.3 Wayland Visibility, Grabbing, and Instant Toggling
* **Problem:** Standard Wayland security primitives deliberately block applications from mapping frameless floating windows that grab global keyboard focus. Additionally, standard Wayland windows cannot be dynamically "hidden" or "shown" without complete client teardown.
* **Mandatory Solution:**
  1. Use the `wlr-layer-shell-unstable-v1` protocol to configure the UI window as an `Overlay` layer surface.
  2. To hide the application instantly without killing the process, **detach the `wl_buffer`** from the surface, commit a null buffer to clear the screen, and set `KeyboardInteractivity::None`.
  3. To show the window, reattach the active Slint buffer and toggle interactivity back to `Exclusive` or `OnDemand`.

---

## 5. Agent Instructions & Code Style Constraints

* **No Web Views:** Under no circumstances should Electron, Tauri, Wry, WebView2, or any browser-based runtime be imported. Code additions doing so will be rejected.
* **Memory Allocations:** Minimize allocations in the hot path (as-you-type fuzzy searching). Reuse vectors, utilize references, and leverage zero-copy parsing (`postcard::from_bytes`) directly out of the IPC stream buffers.
* **Async Framework:** Prefer a lightweight async runtime (`smol` / `async-executor`) or direct OS thread channels to keep the footprint small. If `tokio` is required, it must be declared with `default-features = false` and only the minimal feature set needed (e.g., `rt`, `net`, `macros`); pulling in `tokio` with default or `full` features is not permitted.
* **Error Handling:** Avoid `.unwrap()` and `.expect()`. Use `thiserror` for library components and `anyhow` for top-level binaries. All IPC disconnection events must trigger smooth client state adjustments or automated reconnect attempts rather than crashing.

---

## 6. Project Conventions

* **License:** The project is licensed under **GPL-3.0**. All new source files must include an SPDX header (`// SPDX-License-Identifier: GPL-3.0-or-later`).
* **Lints & Formatting:** Code must pass `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` before merge. CI must enforce both.
* **Plugin Manifest:** Plugins declare their requested capabilities (filesystem paths, network hosts, environment variables) in a TOML manifest co-located with the `.wasm`/`.cwasm` artifact. The canonical schema lives in `docs/plugin-manifest.md`; agents must not invent ad-hoc manifest formats. Capabilities not declared in the manifest must never be granted at runtime.
