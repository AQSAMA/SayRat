# Product Requirement Document (PRD) & Software Requirements Specification (SRS)

**Project Codename:** SayRat<br>
**License:** GPL-3.0-or-later<br>
**Canonical Implementation Reference:** `AGENTS.md` is the authoritative source for the hardened technology stack, architectural mandates, and standing rules that govern all implementation decisions. This document defines *what* is required; `AGENTS.md` defines *how* it must be built.

---

## Part 1 — Product Requirement Document (PRD)

### 1. Product Vision & Problem Statement

**Vision:** Build an open-source, ultra-lightweight keyboard launcher for desktop power users delivering instant invocation, near-zero input latency, and a sandboxed WebAssembly plugin ecosystem within a sub-20 MB total memory footprint.

**Problem Statement:** Existing launcher solutions face two common failure modes on modern desktops:

1. **Web-based Heavyweights:** Electron/WebView-based launchers embed full browser runtimes, resulting in memory footprints of 150–400 MB, perceptible input lag, and slow cold starts.
2. **Legacy Native Limitations:** X11-centric native launchers struggle on modern Wayland compositors because they rely on older window-management primitives and typically lack modern, isolated extensibility.

### 2. Target Audience & Use Cases

**Primary Audience:** Linux power users, system administrators, minimalists, and developers using tiling window managers (Sway, Hyprland) or modern Wayland desktops.

**Secondary Audience:** macOS and Windows developers seeking a fast, hyper-minimalist alternative to Spotlight or PowerToys Run.

**Core Use Cases:**

- Rapid application launching and window switching.
- Blazing-fast fuzzy file navigation.
- Interacting with third-party APIs via sandboxed plugins.

### 3. MVP Feature Set

| Feature | Description |
| :--- | :--- |
| **Instant UI Rendering** | The launcher interface appears immediately on global hotkey press with no perceptible delay. |
| **Fuzzy Finding Engine** | As-you-type, typo-tolerant search across applications and files with sub-16 ms result updates under the benchmark workload. |
| **Dual-Process Architecture** | A headless background daemon manages state and Wasm execution; an ephemeral UI client handles pure rendering. This split is mandatory — see `AGENTS.md` §1. |
| **Wasm Plugin Engine** | Load and execute compiled Wasm plugins with restricted WASI capabilities under a zero-ambient-authority sandbox. |
| **Wayland-First Native** | Frameless overlay via `wlr-layer-shell-unstable-v1` with sub-frame show/hide toggling where available — see `AGENTS.md` §4.3. |

---

## Part 2 — Software Requirements Specification (SRS)

> **Language convention:** **shall / must** = binding requirement. **should** = strongly recommended default. **may** = permitted but not mandated.

### 4. Functional Requirements

#### 4.1 Daemon Process (`sayratd`)

- **Lifecycle & State:** The daemon shall run as a background user service, bind to a local socket, and maintain full application state and plugin lifecycle across UI invocations.
- **Asynchronous Indexing:** The daemon shall monitor the standard XDG application directories for filesystem changes where supported and update the search index in the background without blocking IPC handling.
- **Plugin Hosting:** The daemon shall load `.cwasm` plugins into an isolated Wasm sandbox with capability-based security. Network access, host filesystem access, and environment variables shall be denied by default and granted only through the validated plugin manifest.

#### 4.2 UI Client Process (`sayrat-ui`)

- **View Rendering:** The client shall render the frameless, floating search interface using the Slint DSL.
- **Input Capture:** The client shall capture keystrokes and forward them to the daemon via IPC. All search logic runs in the daemon; the client performs no local matching.
- **Suspended State:** The client shall remain initialized in memory between invocations, waking on global hotkey or IPC signal with no perceptible latency.
- **Read-Only Boundary:** The client must not access the filesystem, database, or plugin runtime directly. Any cross-process need goes through the `sayrat-protocol` IPC layer.

#### 4.3 Inter-Process Communication (IPC)

- **Transport:** Bidirectional communication shall use `interprocess` local sockets on Linux/macOS and Named Pipes on Windows.
- **Serialization:** All payloads shall use `postcard` (primary) or `bincode` (approved fallback) — strictly typed and zero-copy where possible.
- **Framing:** A length-prefixed (`u32` little-endian) frame codec shall be used. A `MAX_FRAME_BYTES` constant (1 MiB) shall be enforced; oversized frames shall be rejected, not panicked.

### 5. Non-Functional Requirements

These targets are hard acceptance gates for the relevant implementation phases and become CI-enforced once the measurement jobs are introduced in Phase 5.

| Metric | Target | Measured As |
| :--- | :--- | :--- |
| UI Client RSS | < 5 MB | Active, post-first-show |
| Daemon RSS | < 15 MB | Idle, post-warmup with ≥ 100 indexed entries |
| Combined RSS | < 20 MB | Steady-state (both processes running) |
| Input-to-first-render latency | < 16 ms p50 | Keystroke → first search chunk rendered on screen |
| Plugin execution budget | ≤ 100 ms | Per Wasm call, enforced via epoch interruption |

**Security (WASI):** Plugins shall execute inside a `wasmtime` engine configured with no ambient authority. A `WasiCtx` starts empty; capabilities are added only after manifest validation. Anything undeclared in the manifest must result in a permission-denied trap, not a silent fallback.

### 6. Architecture Edge Cases & Mandatory Mitigations

The following mitigations address known platform limitations. They are mandatory implementation requirements — see `AGENTS.md` §4 for full implementation detail and rationale.

#### 6.1 Slint VectorModel Bottleneck

Replacing the `VectorModel` dataset can trigger broad `row_data_changed` notifications, causing frame drops on large result sets.

**Required mitigation:** The daemon shall stream results in chunks of ≤ 50 matches over IPC. The UI client shall implement a `PagedResultModel` that emits only `row_added` for incoming items — never a dataset reset or `row_data_changed` on already-rendered rows.

#### 6.2 Wasm Runtime Memory Bloat

Default Wasmtime with Cranelift JIT compilation can allocate 30–50 MB on initialization, violating the daemon RSS budget.

**Required mitigation:** The daemon shall configure `wasmtime` with a `PoolingAllocationConfig` capping per-instance resource consumption. Plugins shall be pre-compiled from `.wasm` to `.cwasm` (AOT) at install time via the plugin install flow, bypassing JIT overhead entirely at runtime where supported.

#### 6.3 Wayland Visibility & Window Management

Standard Wayland prevents regular application windows from grabbing keyboard focus globally and provides no cheap show/hide toggle; naive teardown/recreate adds perceptible latency.

**Required mitigation:** The client shall use `wlr-layer-shell-unstable-v1` to create an `Overlay` layer surface where available. Hide is implemented by detaching the `wl_buffer`, committing a null buffer, and setting `KeyboardInteractivity::None`. Show restores the active buffer and `Exclusive` (or `OnDemand`) interactivity. The surface must not be destroyed between visibility toggles.

### 7. Required Technology Stack

The crates below are required or approved by this project. See `AGENTS.md` §2 for the complete approved list, implementation constraints, crate addition policy, feature restrictions, and pre-approved fallbacks for each subsystem.

| Subsystem | Primary Crate(s) | Approved Fallback |
| :--- | :--- | :--- |
| UI | `slint` | — |
| IPC Transport | `interprocess` | — |
| Serialization | `postcard` | `bincode` |
| Fuzzy Search | `nucleo` | `frizbee` |
| Wasm Host | `wasmtime` + `wasmtime-wasi` | `extism` |
| Database | `redb` | `fjall` |
| Wayland | `smithay-client-toolkit` + `wayland-protocols` | — |
| Global Hotkeys | `global-hotkey` + `zbus` (D-Bus fallback) | — |
| CLI Parsing | `pico-args` | — |
| Async Runtime | `smol` | `tokio` with `default-features = false` and justification |
| Logging / Instrumentation | `tracing` + `tracing-subscriber` | `log` + `env_logger` with documented reason |
| Error Handling | `thiserror` (libs) + `anyhow` (bins) | — |
| Filesystem Events | `notify` v7.x | Periodic rescan if watcher is rejected by platform/policy |
| Desktop Entry Parsing | `freedesktop_entry_parser` | Hand-rolled minimal parser if binary delta > 50 KB |
