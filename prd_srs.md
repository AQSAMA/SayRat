# SayRat PRD & SRS

**Project codename:** SayRat  
**License:** GPL-3.0-or-later

## Part 1 — Product Requirements Document (PRD)

### 1. Product Vision & Problem Statement

**Vision:** Build an open-source, ultra-lightweight keyboard launcher for desktop power users. SayRat aims for instant invocation, near-zero input latency, and a sandboxed WebAssembly plugin ecosystem while keeping the combined UI + daemon footprint near 20 MB.

**Problem statement:** Existing launcher solutions often struggle in modern desktop environments:

1. **Web-based heavyweights:** Electron/WebView launchers can consume 150–400 MB RAM, add perceptible input lag, and slow cold starts.
2. **Legacy native limitations:** X11-centric launchers can struggle on Wayland because they depend on old window-management primitives and often lack modern isolated extensibility.

### 2. Target Audience & Use Cases

**Primary audience:** Linux power users, system administrators, minimalists, and developers using tiling window managers such as Sway or Hyprland, or other Wayland desktops.

**Secondary audience:** macOS and Windows developers seeking a fast, hyper-minimalist alternative to Spotlight or PowerToys Run.

**Core use cases:**

- Rapid application launching and window switching.
- Blazing-fast fuzzy file navigation.
- Interacting with third-party APIs through sandboxed plugins.

### 3. MVP Features

- **Instant UI rendering:** The interface should appear immediately after a global hotkey press.
- **Fuzzy finding engine:** As-you-type, typo-tolerant search across entries such as applications and files with minimal UI blocking.
- **Dual-process architecture:** A headless daemon handles state and Wasm execution; a separate UI client focuses on rendering and input capture.
- **Wasm plugin engine:** Load and execute compiled Wasm plugins with restricted capabilities.
- **Wayland-first native implementation:** Native Wayland window generation with layer-shell integration where available.

## Part 2 — Software Requirements Specification (SRS)

### 4. Functional Requirements

#### 4.1 Daemon Process (`sayratd`)

- **Lifecycle & state:** Run as a background user service, bind to a local socket, and maintain application state and plugin lifecycle.
- **Asynchronous indexing:** Monitor filesystem changes through OS-native events where feasible, with a documented fallback when platform support is unavailable.
- **Plugin hosting:** Load Wasm plugins into an isolated, capability-based sandbox that denies filesystem, network, and environment access by default.

#### 4.2 UI Client Process (`sayrat-ui`)

- **View rendering:** Render the frameless floating search interface using Slint.
- **Input capture:** Capture keystrokes and forward them to the daemon over IPC so the UI stays simple and responsive.
- **Suspended state:** Remain resident but invisible between invocations so showing the launcher avoids process startup latency.

#### 4.3 Inter-Process Communication (IPC)

- **Protocol:** Bidirectional, low-latency communication over local sockets.
- **Payloads:** Strictly typed compact serialization, preferably `postcard` with `bincode` as an approved fallback.

### 5. Non-Functional Requirements

- **Memory budgets:** Target `< 5 MB` RSS for `sayrat-ui`, `< 15 MB` idle RSS for `sayratd`, and roughly `< 20 MB` combined steady-state RSS. These are product targets; implementation phases should measure them and document any evidence-based adjustment before relaxing a gate.
- **Latency:** Search results should update within 16 ms of a keystroke under the benchmark workload defined in the implementation phases.
- **Security:** Plugins execute with no ambient authority. Host filesystem, network, and environment access must be absent unless explicitly declared in the plugin manifest and granted by the host.

### 6. Architectural Edge Cases & Suggested Mitigations

These approaches help developers and AI agents handle known edge cases. They are recommendations at the PRD/SRS level; stronger constraints and approved implementation details live in `AGENTS.md`.

#### 6.1 Slint `VectorModel` Bottleneck

**Problem:** Replacing a large dynamic list can trigger broad `row_data_changed` notifications and cause frame drops.

**Suggested solution:** Chunk results over IPC. The daemon sends fixed-size chunks, and the client maintains a paged result model that renders the first visible page immediately and appends subsequent chunks without resetting the entire model.

#### 6.2 Wasm Runtime Memory Bloat

**Problem:** A default Wasm engine can consume tens of megabytes for compiler, code-cache, and allocator structures.

**Suggested solution:** Tune the runtime for low footprint through pooling allocation, low-overhead compilation settings, and ahead-of-time compilation to `.cwasm` during plugin installation.

#### 6.3 Wayland Visibility & Window Management

**Problem:** Wayland intentionally restricts global focus grabs and cheap show/hide behavior for regular application windows.

**Suggested solution:** Use `wlr-layer-shell-unstable-v1` where available, hide by detaching the `wl_buffer` and disabling keyboard interactivity, and show by reattaching the buffer with compositor-appropriate keyboard interactivity.

### 7. Recommended Tech Stack

The implementation should start from the approved stack in `AGENTS.md` and add dependencies only when they are used and justified.

- **Core UI:** `slint`.
- **IPC transport & serialization:** `interprocess` + `postcard` or `bincode`.
- **Fuzzy searching:** `nucleo` or `frizbee`.
- **Wasm host:** `wasmtime` or `extism`.
- **Embedded database:** `redb` or `fjall`.
- **Wayland integration:** `smithay-client-toolkit` + `wayland-protocols`.
- **Global hotkeys:** `global-hotkey`, with D-Bus fallback where compositor policy requires it.
