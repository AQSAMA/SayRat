Product Requirement Document (PRD) & Software Requirements Specification (SRS)
Project Codename: SayRat
License: GPL-3.0 (Open Source)
Part 1: Product Requirement Document (PRD)
 1. Product Vision & Problem Statement
   Vision: To build an open-source, ultra-lightweight keyboard launcher for desktop power users. It aims to deliver instant cold-starts, near-zero input latency, and a sandboxed WebAssembly plugin ecosystem, all while striving for a sub-20MB total memory footprint.
Problem Statement: Existing launcher solutions often face challenges in the modern desktop environment:
 1. Web-based Heavyweights: Electron/WebView-based launchers rely heavily on embedded browsers, which can result in larger memory footprints (150–400MB RAM), perceptible input lag, and slower cold starts.
 2. Legacy Native Limitations: Native but X11-centric launchers may struggle on modern Wayland compositors due to their reliance on older window management primitives, and they often lack modern, isolated extensibility.
 3. Target Audience & Use Cases
   Primary Audience: Linux Power Users, system administrators, minimalists, and developers using tiling window managers (Sway, Hyprland) or Wayland desktops.
   Secondary Audience: macOS and Windows developers seeking a fast, hyper-minimalist alternative to Spotlight or PowerToys Run.
   Core Use Cases:
   Rapid application launching and window switching.
   Blazing-fast fuzzy file navigation.
   Interacting with third-party APIs via sandboxed plugins.
 4. MVP Features (Phase 1)
   Instant UI Rendering: The interface should ideally appear instantly upon a global hotkey press.
   Fuzzy Finding Engine: As-you-type, typo-tolerant search across entries (applications, files) with minimal UI blocking.
   Dual-Process Architecture: It is highly recommended to use a headless background daemon handling state/Wasm execution, alongside an ephemeral UI client for pure rendering.
   Wasm Plugin Engine: Load and execute compiled Wasm plugins with restricted capabilities (WASI).
   Wayland-First Native Implementation: Native Wayland window generation, suggested to utilize layer-shell integration.
Part 2: Software Requirements Specification (SRS)
 4. Functional Requirements (Suggested Guidelines)
4.1 Daemon Process (sayratd)
Lifecycle & State: Should run as a background user service, bind to a local Unix domain socket, and maintain the application state and plugin lifecycle.
Asynchronous Indexing: Suggested to monitor filesystem changes (via inotify/OS events) and dynamically update the search index in the background.
Plugin Hosting: Recommended to load .wasm plugins into an isolated sandbox utilizing capability-based security (e.g., explicitly denying/granting network access).
4.2 UI Client Process (sayrat-ui)
View Rendering: Render the frameless, floating search interface using the Slint DSL.
Input Capture: Capture keystrokes and forward them to the Daemon via IPC to minimize local processing overhead.
Suspended State: Ideally maintain an invisible state in memory, waking swiftly upon receiving IPC signaling.
4.3 Inter-Process Communication (IPC)
Protocol: Bi-directional, low-latency communication over Local Sockets is recommended.
Payloads: Using strictly typed, zero-copy serialization (such as postcard or bincode) is highly encouraged.
 5. Non-Functional Requirements (Targets)
   Memory Budgets: The project should aim for the Slint UI Client to consume < 5MB RAM, and the background Daemon to idle at < 15MB RAM.
   Latency: Search results should ideally update within 16ms (60Hz frame rate) after a keystroke.
   Security (WASI): Plugins should execute inside a wasmtime (or similar) engine configured with no ambient authority, preventing arbitrary host filesystem or network access by default.
 6. Architectural Edge-Cases & Suggested Mitigations
To assist developers and AI agents, the following approaches are suggested to handle known edge cases. These are recommendations, not strict constraints; superior alternatives may be used if discovered.
6.1 Challenge 1: Slint VectorModel Bottleneck
The Problem: Slint’s dynamic list models (VectorModel) emit full row_data_changed notifications when a dataset is replaced. Pushing thousands of fuzzy search results simultaneously can cause UI frame drops.
Suggested Solution: Implement result chunking over IPC. The Daemon could send results in fixed-size chunks (e.g., top 50 matches). The Client can then maintain a PagedResultModel, rendering the first visible page instantly and re-requesting subsequent chunks incrementally without resetting the entire model.
6.2 Challenge 2: Wasm Runtime Memory Bloat
The Problem: Initializing a default Wasm engine (like Wasmtime with Cranelift) can occasionally consume 30-50MB of RAM for code caches and environment setup.
Suggested Solution: The Wasm runtime could be tuned to reduce footprint. Potential strategies include:
 1. Pooling Allocator: Pre-allocating instance resources to eliminate per-instance overhead.
 2. Low-Overhead Compilation: Configuring the compiler (e.g., OptLevel::None) to minimize compile-time memory footprint.
 3. AOT Pre-compilation: Pre-compiling plugins to native binaries at install time, bypassing runtime JIT compilation entirely.
6.3 Challenge 3: Wayland Visibility & Window Management
The Problem: Standard Wayland security protocols prevent applications from drawing frameless, floating overlay windows that globally steal keyboard focus. Furthermore, Wayland does not traditionally support "hiding" a window.
Suggested Solution:
 1. Layer Shell: The client could bypass standard window mapping and explicitly utilize the wlr-layer-shell-unstable-v1 protocol to draw an Overlay layer surface.
 2. Sub-frame Toggling: To achieve instant invocation without termination overhead, the UI could be "hidden" by detaching the wl_buffer and committing a null buffer, whilst setting KeyboardInteractivity::None.
 3. Recommended Tech Stack
   The following ecosystem of crates is highly recommended for achieving the project goals, though alternative crates may be utilized if they better respect the resource targets.
Core UI: slint (Declarative DSL, minimal runtime).
IPC Transport & Serialization: interprocess + postcard or bincode (Local sockets with minimal wire overhead).
Fuzzy Searching: nucleo or frizbee (Lock-free concurrent streaming or SIMD-accelerated matching).
Wasm Host: wasmtime or extism (Production-grade WASI support, configurable for low memory).
Embedded Database: redb or fjall (Pure Rust, ACID, zero-copy alternatives to SQLite).
Wayland Integration: smithay-client-toolkit + wayland-protocols.
Global Hotkeys: global-hotkey (with suggested fallback integration via D-Bus/zbus for Wayland compositors).