# Analysis & Improvements for PRD-SRS & AGENTS.md

## Suggested Modifications

### For `prd_srs.md`:
* **Explicit Phase Breakdown:** While the MVP is mentioned, breaking down the project into concrete milestones (e.g., Workspace Setup, IPC, UI, Engine, Plugins) would provide a clearer roadmap.
* **Refined Database Choice:** Provide clear criteria for when to choose `redb` over `fjall` (e.g., `redb` is B-Tree based, `fjall` is LSM-Tree based; for mostly read-heavy workloads like application indexing, `redb` might be preferable).
* **Wayland Hotkeys:** Mention that `global-hotkey` might need explicit compositor-specific configurations (e.g., Hyprland/Sway specific IPC or generic wlroots protocols) since Wayland's security model strictly restricts global keybinding interception.

### For `agents.md`:
* **Directory Structure:** As an AI directive, establishing a strict directory layout for the dual-process workspace (e.g., `/sayratd`, `/sayrat-ui`, `/sayrat-core` for shared IPC types) would prevent architectural drift and enforce the boundary.
* **Testing Mandates:** Add a rule requiring test cases for serialization, IPC boundaries, and plugin isolation to ensure agents verify their work before committing.

---

# Development Phases & AI Prompts

The development of SayRat will be divided into 5 distinct phases. The repository currently being empty necessitates starting from the foundational workspace configuration.

## Phase 1: Project Initialization & Workspace Setup
**Goal:** Establish the Cargo workspace, create the skeleton for the background daemon (`sayratd`) and the UI client (`sayrat-ui`), and set up a shared library for common types (`sayrat-core`).

**Completion Prompt:**
> "Initialize a new Cargo workspace for the 'SayRat' project. Create three packages: `sayratd` (binary), `sayrat-ui` (binary), and `sayrat-core` (library). Configure the `Cargo.toml` at the root to include these members. Ensure that both binaries print a basic 'Hello from [component]' message. Add SPDX license headers (`// SPDX-License-Identifier: GPL-3.0-or-later`) to all Rust files. Verify that `cargo check` and `cargo fmt` pass without errors across the workspace."

**Verification Prompt:**
> "Run `cargo build --workspace` to ensure all components compile. Execute `cargo run --bin sayratd` and `cargo run --bin sayrat-ui` sequentially to confirm they both print their respective 'Hello' messages. Check that `Cargo.toml` correctly defines the workspace and that the codebase adheres to standard Rust formatting by running `cargo fmt --check`. Confirm SPDX headers exist in all `.rs` files."

## Phase 2: Inter-Process Communication (IPC) & Core Data Models
**Goal:** Implement the bi-directional communication layer between the daemon and the client using local Unix domain sockets and zero-copy serialization.

**Completion Prompt:**
> "In `sayrat-core`, define the shared data models for IPC communication (e.g., `SearchRequest`, `SearchResultChunk`) using the `serde` and `postcard` crates. Implement a basic IPC server in `sayratd` using the `interprocess` crate to listen on a local Unix domain socket. Implement the corresponding IPC client in `sayrat-ui` to connect to this socket. Create a simple ping-pong loop where the UI sends a placeholder search string and the daemon responds with a mock result chunk. Ensure memory allocations in the IPC hot path are minimized as per `agents.md`."

**Verification Prompt:**
> "Start the `sayratd` process in the background. Then, run the `sayrat-ui` process. Verify through terminal output or logs that the UI successfully connects to the daemon's Unix socket, sends a test request, and correctly receives and deserializes the `SearchResultChunk` using `postcard`. Ensure there are no panics or unhandled `unwrap()` calls on connection failure. Finally, gracefully kill both processes."

## Phase 3: UI Client Rendering & Wayland Integration
**Goal:** Build the frameless floating search interface using Slint and configure the Wayland layer-shell logic for instant showing/hiding without window destruction.

**Completion Prompt:**
> "Implement the UI for `sayrat-ui` using the `slint` framework. Create a simple search bar and a list view designed to handle paginated results. Integrate `smithay-client-toolkit` to map the Slint window as a Wayland `Overlay` layer surface via `wlr-layer-shell-unstable-v1`. Implement the hide/show logic described in `agents.md`: do not destroy the window to hide it; instead, detach the `wl_buffer`, commit a null buffer, and set `KeyboardInteractivity::None`. Restore these to show the window. Bind a basic global hotkey mechanism to toggle this visibility."

**Verification Prompt:**
> "Launch `sayrat-ui` in a Wayland environment (e.g., Sway or an isolated Weston/Hyprland session). Verify that the UI renders as an unmanaged overlay (no window borders). Press the configured global hotkey and confirm the UI instantly hides without the process terminating. Press the hotkey again and verify it reappears instantly and re-acquires keyboard focus. Monitor the process RAM usage to ensure it remains close to the < 5MB budget."

## Phase 4: Background Daemon Search Engine & Indexing
**Goal:** Implement file and application indexing using a lightweight database, and integrate high-performance fuzzy searching.

**Completion Prompt:**
> "In `sayratd`, integrate the `redb` embedded database to store cached desktop entries and system paths. Implement a background task that reads `.desktop` files from standard Linux directories (e.g., `/usr/share/applications`) and populates this database. Integrate the `nucleo` crate to perform lock-free, concurrent fuzzy matching against the database entries based on IPC requests from the UI. Ensure the results are chunked (e.g., max 50 items) before being sent back over IPC, addressing the Slint `VectorModel` bottleneck."

**Verification Prompt:**
> "Run `sayratd` and inspect its logs to verify it successfully discovers, parses, and inserts `.desktop` entries into the `redb` database. Use `sayrat-ui` to send a partial search string (e.g., 'term'). Confirm that `sayratd` executes a fuzzy search, matches applications like 'Terminal', and sends back a chunked IPC response. Verify that the daemon's idle memory footprint remains under the 15MB budget."

## Phase 5: Wasm Plugin Sandbox & Security Capabilities
**Goal:** Implement the WebAssembly plugin architecture to allow safe extensibility with zero ambient authority.

**Completion Prompt:**
> "Add a Wasm execution engine to `sayratd` using `wasmtime`. Configure it with a Pooling Allocator and WASI support to minimize runtime memory bloat. Implement a plugin loader that reads `.wasm` or `.cwasm` files along with a TOML manifest defining explicit capabilities (e.g., allowed filesystem paths). Ensure the engine runs with 'Zero-Ambient-Authority': explicitly deny all network and filesystem access unless strictly granted by the manifest. Create a trivial 'Hello World' plugin to test the execution flow."

**Verification Prompt:**
> "Write a test plugin that attempts to read a sensitive host file (e.g., `/etc/passwd`). Run `sayratd` and load this plugin without granting filesystem capabilities in its manifest. Verify that the WASI environment correctly denies access and throws a permissions error. Update the manifest to allow access to a specific mock directory, reload the plugin, and verify it can read files only within that directory. Ensure Wasmtime initialization does not cause massive RAM spikes."
