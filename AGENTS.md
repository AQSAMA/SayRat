# AI Agent Project Directives: Project SayRat

**Project Vision:** SayRat is an open-source, ultra-lightweight keyboard launcher for desktop power users. It relies on a Wayland-first, Wasm-extensible, dual-process architecture written in Rust, targeting a strict sub-20 MB total memory footprint and instant invocation.

`AGENTS.md` and `prd_srs.md` are the canonical project documents for all AI agents, coding assistants (e.g., Codex, Claude Code, Cursor, Aider, Copilot), and automated pipelines interacting with this codebase. If a phase-specific prompt conflicts with either canonical document, the canonical documents win; the conflict must be surfaced in the PR description rather than silently resolved.

Prefer concrete diffs, measurable verification, explicit trade-off notes, and rerunnable commands over persona prompts, hidden context, or speculative implementation plans.

---

## 1. System Architecture & Boundaries

The application is strictly decoupled into two separate processes. Agents must never bridge these boundaries except through the defined IPC layer.

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

Agents must not add arbitrary dependencies. Only the following approved crates and tools may be used. Any dependency not listed here requires an explicit justification block in the PR naming the subsystem, rejected alternatives, and estimated binary-size / RSS impact.

### 2.1 Core & Runtime

| Subsystem | Approved Technology | Implementation Constraints |
| :--- | :--- | :--- |
| **Core Language** | Rust stable toolchain | Pin `rust-version` in `[workspace.package]`; MSRV increases require an explicit PR note and quarterly review. Write idiomatic Rust with zero-copy architecture where applicable. |
| **UI Framework** | `slint` | Declarative UI DSL compiled natively. No web view; see §7. |
| **IPC Transport** | `interprocess` | Local Unix domain sockets (Linux/macOS) / Named Pipes (Windows). |
| **Serialization** | `postcard` (primary) / `bincode` (approved fallback) | Strictly typed, compact binary format. New code must default to `postcard`. |
| **Fuzzy Matching** | `nucleo` (primary) / `frizbee` (approved fallback) | Lock-free concurrent streaming or SIMD-accelerated matching. |
| **Wasm Runtime** | `wasmtime` (primary) / `extism` (approved fallback) | WASI execution with pooling allocator and epoch interruption (see §4.2). `wasmtime-wasi` is implicitly approved as part of the `wasmtime` crate suite. Note: `extism` wraps `wasmtime` and constrains the plugin manifest format to the Extism PDK; choose deliberately. |
| **Database** | `redb` (primary) / `fjall` (approved fallback) | Pure Rust, ACID-compliant, low-footprint alternative to SQLite. |
| **Wayland Native** | `smithay-client-toolkit` + `wayland-protocols` | Layer-shell window positioning and surface lifecycle (see §4.3). |
| **Global Hotkeys** | `global-hotkey` | Native hooks with `zbus` (D-Bus) fallback for Wayland compositors. |
| **CLI Parsing** | `pico-args` | Minimal argv parsing for daemon/client flags (`--socket`, `--version`). No proc-macros. `clap` is **not approved**; it conflicts with the binary-size budget. |
| **Desktop Entry Parsing** | `freedesktop_entry_parser` | INI-style `.desktop` parser for the daemon indexer only. If it adds > 50 KB to the stripped binary, a minimal hand-rolled parser is an accepted substitute; document the choice and measure the delta. |
| **Filesystem Events** | `notify` v7.x | Cross-platform inotify / FSEvents / ReadDirectoryChanges watcher. Debounce window: 200 ms. |

### 2.2 Infrastructure & Dev Tooling

| Subsystem | Approved Technology | Implementation Constraints |
| :--- | :--- | :--- |
| **Async Runtime** | `smol` (primary) / `async-executor + async-io` (component alternative) | Lightweight executors chosen to protect the footprint budget. If `tokio` is required, it **must** be declared with `default-features = false` and only the minimal features needed (e.g., `rt`, `net`, `macros`); `full` or default-feature `tokio` is **not permitted**. Any `tokio` use must be justified in the PR. |
| **Logging / Instrumentation** | `tracing` + `tracing-subscriber` (primary) | Use one facade across workspace binaries. `tracing-subscriber` must be declared with `default-features = false` and only the minimum features required for formatting/env filtering. Record binary-size/RSS impact in the Phase 1 PR. `log` + `env_logger` is an approved fallback only if `tracing` is rejected for a documented reason; once chosen, use the same facade across all crates. |
| **Error Handling** | `thiserror` (library crates) + `anyhow` (binary crates) | `thiserror` for structured, typed errors in `sayrat-protocol` and other library crates. `anyhow` for top-level binary error plumbing. Do not use `Box<dyn Error>` as a substitute. |
| **Property Testing** | `proptest` | Approved for `#[cfg(test)]` contexts only. Zero production-binary impact. |
| **Dependency Auditing** | `cargo-deny` (CI only) | License and advisory checking. Deny any license stricter than GPL-3.0-or-later compatible. Runs in CI; does not ship in any binary. |

### 2.3 Developer Utilities (Not Shipped)

| Crate | Purpose |
| :--- | :--- |
| `crates/sayrat-cli` | Minimal CLI test client added in Phase 3. Connects to `sayratd`, drives search queries, and exercises cancellation. Used for development and integration testing. Not distributed in releases or compiled into production builds. |

---

## 3. Strict Non-Functional Requirements & Budgets

All code generation and structural changes must validate against these explicit performance baselines. These budgets are hard acceptance gates for relevant phases; they are reported in phase PRs and become CI-enforced when the measurement jobs are introduced in Phase 5. If a dependency or platform makes a budget impossible, do not silently weaken the requirement: measure it, explain the trade-off in the PR, and propose a smaller follow-up before merging.

* **Memory Budgets:**
  * UI Client (`sayrat-ui`): **< 5 MB RAM** active footprint (measured post-first-show).
  * Background Daemon (`sayratd`): **< 15 MB RAM** idle footprint (measured post-warmup with ≥ 100 indexed entries).
  * *Total System Target:* **< 20 MB RAM** steady-state combined footprint.
* **Latency Profile:**
  * **Input-to-First-Render Latency:** < 16 ms p50 (keystroke → first chunk rendered on screen).
  * **Cold Start / Wake Latency:** Imperceptible (instant UI draw; the client must remain initialized in memory between invocations).
* **Security & Isolation:**
  * Wasm plugins must execute under a strict **Zero-Ambient-Authority Capability Model**.
  * Network access, host filesystem access, and environment variable access must be denied by default and explicitly injected only if declared in the plugin manifest.

---

## 4. Architectural Edge-Cases & Mandatory Mitigations

The following patterns are **mandatory** for the named subsystems. They are not suggestions; superior alternatives may be used only if they demonstrably satisfy the same constraints and the deviation is documented in the PR.

### 4.1 Slint VectorModel Bottleneck (UI Stuttering)

* **Problem:** Slint's `VectorModel` triggers wholesale `row_data_changed` notifications when updating underlying collections. Pushing thousands of matching search results in one shot causes rendering frame-drops.
* **Mandatory Solution:** Implement a paged/chunked results architecture.
  1. `sayratd` streams results over IPC in fixed chunks (≤ 50 matches per packet).
  2. `sayrat-ui` implements a custom `PagedResultModel` that emits only `row_added` per new item — **never** `row_data_changed` for already-rendered rows — and renders the first visible page immediately.
  3. Incremental rows are requested and appended as the user scrolls, avoiding model resets.

### 4.2 Wasm Runtime Memory Bloat

* **Problem:** Instantiating a default Wasmtime engine with Cranelift compilation can consume 30–50 MB of RAM due to internal buffers, code cache, and allocator structures.
* **Mandatory Solution:**
  1. Configure `wasmtime` with a **Pooling Allocator** (`PoolingAllocationConfig`) to explicitly cap maximum per-instance resource consumption (memory, tables, instances).
  2. Prefer **Ahead-of-Time (AOT) Pre-compilation**: compile `.wasm` modules into Wasmtime's serialized `.cwasm` format during plugin installation, bypassing JIT memory overhead entirely. `OptLevel::None` is an approved fallback for dynamic loading when AOT is unavailable.
  3. Enable **Epoch Interruption** (`Engine::epoch_interruption`) with a hard per-call budget (≤ 100 ms) so runaway plugins are killed cleanly without taking down the daemon.

### 4.3 Wayland Visibility, Grabbing, and Instant Toggling

* **Problem:** Standard Wayland security primitives deliberately block applications from mapping frameless floating windows that grab global keyboard focus. Standard windows cannot be dynamically hidden or shown without complete surface teardown.
* **Mandatory Solution:**
  1. Use the `wlr-layer-shell-unstable-v1` protocol to configure the UI window as an `Overlay` layer surface.
  2. To hide the application instantly, **detach the `wl_buffer`** from the surface, commit a null buffer to clear the screen, and set `KeyboardInteractivity::None`. Do **not** destroy or recreate the surface.
  3. To show the window, reattach the active Slint buffer and toggle interactivity to `Exclusive` (or `OnDemand` as a fallback for compositors that reject `Exclusive`).

---

## 5. Agent Operating Model & Code Style Constraints

* **Execution Style:** Start from the current repository state, make the smallest coherent change, and verify with commands that future agents can rerun. Do not rely on hidden context, long prompt incantations, or claims without file/test evidence.
* **Planning:** For large work, maintain a short checklist tied to deliverables and update it as facts change. Surface conflicts with this file or `prd_srs.md` instead of improvising around them.
* **Dependencies:** Treat the approved stack as a default allow-list, not a license to add every listed crate immediately. Add dependencies only when used, keep features minimal, and justify any new crate or feature in the PR.
* **No Web Views:** Electron, Tauri, Wry, WebView2, and any browser-based runtime are permanently forbidden. See §7.
* **Memory Allocations:** Minimize allocations in the hot path (as-you-type fuzzy searching). Reuse vectors, use references, and leverage zero-copy parsing (`postcard::from_bytes`) directly out of IPC stream buffers.
* **Async Runtime:** Use `smol` or `async-executor + async-io` (see §2.2). If `tokio` is introduced, it must follow the constraints in §2.2; default or `full`-feature `tokio` is not permitted.
* **Error Handling:** No `.unwrap()` or `.expect()` in non-test code. Use `thiserror` for library components and `anyhow` for top-level binaries (see §2.2). All IPC disconnection events must trigger smooth client state adjustments or automated reconnect attempts — never a crash or panic.

---

## 6. Project Conventions

* **License:** The project is licensed under **GPL-3.0**. All new source files must include an SPDX header: `// SPDX-License-Identifier: GPL-3.0-or-later`.
* **Lints & Formatting:** Code must pass `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` before merge. CI enforces both.
* **Plugin Manifest:** Plugins declare their requested capabilities (filesystem paths, network hosts, environment variables) in a TOML manifest co-located with the `.wasm`/`.cwasm` artifact. The canonical schema lives in `docs/plugin-manifest.md`; agents must not invent ad-hoc manifest formats. Capabilities not declared in the manifest must never be granted at runtime.

---

## 7. Standing Rules (All Agents, All Phases)

These rules apply unconditionally to every code change in the repository. Paste this section at the top of any agent prompt if the agent lacks persistent context; this table is canonical and `phases_prompts.md` references it rather than redefining policy.

| Rule | Requirement |
| :--- | :--- |
| **Canonical Docs** | `prd_srs.md` + `AGENTS.md` govern all implementation decisions. Phase prompts that conflict with either document must be surfaced in the PR, not silently resolved. |
| **SPDX Header** | Every new `.rs` file begins with `// SPDX-License-Identifier: GPL-3.0-or-later`. |
| **Approved Crates Only** | No dependency outside §2. New additions require a PR justification block: subsystem, rejected alternatives, binary-size / RSS impact. |
| **Add Dependencies Lazily** | Approved crates are added when first used, not pre-pinned all at once. Keep features minimal and workspace-managed. |
| **No `.unwrap()` / `.expect()`** | Forbidden outside `#[cfg(test)]` blocks. Use `thiserror` (libs) and `anyhow` (bins). |
| **Code Quality** | All code must pass `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`. CI is the gate. |
| **Memory & Latency Budgets** | §3 values are hard acceptance gates and become CI gates when the measurement job is introduced. A regression in RSS or p50 input-to-render latency fails the relevant phase/release gate. |
| **No Web Runtimes** | Electron, Tauri, Wry, WebView2, and any browser-embedded runtime are permanently forbidden. PRs introducing them will be rejected. |
| **IPC Boundary** | The UI must not touch the filesystem, database, or plugin runtime directly. All cross-process communication goes through the typed `postcard` protocol defined in `sayrat-protocol`. |
