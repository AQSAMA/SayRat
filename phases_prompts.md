# SayRat — Build Phases & Agent Prompts

This document breaks the SayRat project (see `prd_srs.md` and `AGENTS.md`) into **five sequential phases**. Each phase contains:

1. A **Build Prompt** — given to an AI agent to fully complete the phase.
2. A **Verification Prompt** — given (ideally to a *fresh* agent session) to independently audit that the phase was completed correctly.

Phases are intentionally ordered so each builds on a working, testable foundation:

| # | Phase | Outcome |
|---|-------|---------|
| 1 | Foundation & Workspace | Cargo workspace, CI, lints, license headers, skeleton binaries |
| 2 | IPC Protocol & Daemon Core | `sayratd` daemon, shared protocol crate, socket transport, indexer + `redb` store |
| 3 | Fuzzy Search Engine & Chunked Streaming | `nucleo` matcher, paged result streaming, cancellation, hot-path zero-copy |
| 4 | UI Client: Slint + Wayland Layer Shell | `sayrat-ui` overlay, `PagedResultModel`, hotkey, sub-frame show/hide |
| 5 | Wasm Plugin System & Hardening | `wasmtime` pooling/AOT, capability manifests, memory-budget gates, release polish |

> **Standing rules for every phase** (paste at the top of every Build Prompt if the agent has no persistent memory):
> - The governing docs are `prd_srs.md` + `AGENTS.md`. Do not contradict them; surface conflicts explicitly.
> - Approved crates only (see `AGENTS.md` §2). No Electron / Tauri / Wry / WebView.
> - Every new source file starts with `// SPDX-License-Identifier: GPL-3.0-or-later`.
> - No `.unwrap()` / `.expect()` in non-test code. Use `thiserror` (libs) / `anyhow` (bins).
> - Code must pass `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`.
> - Memory budgets (UI < 5MB, daemon < 15MB idle, < 20MB combined) and 16ms input-to-render latency are gates for phase acceptance. If measurement shows a gate is unrealistic, report evidence and propose a follow-up instead of silently weakening it.

---

## Phase 1 — Foundation & Workspace

**Goal:** A clean, idiomatic Rust workspace that any contributor (human or agent) can build, lint, and test on day one. No business logic yet — only the scaffolding the next four phases will plug into.

### 1.1 Build Prompt

> You are working on the SayRat repository. Read `prd_srs.md` and `AGENTS.md` in full before writing any code; they are the governing docs for this phase.
>
> **Deliverables for Phase 1 (Foundation & Workspace):**
>
> 1. **Cargo workspace** at the repo root with three member crates:
>    - `crates/sayrat-protocol` — `lib` crate. Will hold shared IPC types. For now, expose an empty `pub mod messages;` and a crate-level doc comment describing its role.
>    - `crates/sayratd` — `bin` crate. The background daemon. `main` should parse `--socket <path>` and `--version` via `pico-args` and exit cleanly after logging "sayratd starting" through `log` + `env_logger` with default features disabled.
>    - `crates/sayrat-ui` — `bin` crate. The UI client. `main` should parse `--socket <path>` and `--version` and exit cleanly after logging "sayrat-ui starting".
> 2. **Workspace `Cargo.toml`** with:
>    - `resolver = "2"`.
>    - `[workspace.package]` defining shared `version`, `edition = "2021"` (or `2024` if the toolchain supports it — pick one and justify), `license = "GPL-3.0-or-later"`, `repository`, `rust-version`.
>    - `[workspace.dependencies]` table pinning only the approved crates actually used in Phase 1 to current, compatible versions. Member crates inherit via `dep.workspace = true`. Do **not** add any crate that is not on the approved list.
>    - `[profile.release]` tuned for size: `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`, `strip = "symbols"`, `opt-level = "z"` or `"s"` (justify).
> 3. **License & headers:**
>    - Top-level `LICENSE` file containing the GPL-3.0-or-later text.
>    - Every `.rs` file in the workspace begins with `// SPDX-License-Identifier: GPL-3.0-or-later`.
> 4. **README.md** (concise) with: project tagline from `prd_srs.md`, architecture diagram from `AGENTS.md` §1, build/run instructions, and a link to `AGENTS.md` for AI contributors.
> 5. **Tooling configuration:**
>    - `rustfmt.toml` with conservative settings (e.g., `edition`, `max_width = 100`, `use_small_heuristics = "Max"`).
>    - `clippy.toml` if any lint thresholds need raising; otherwise omit it.
>    - `.editorconfig` enforcing UTF-8, LF, final newline, 4-space indent for Rust, 2-space for TOML/YAML.
>    - `.gitignore` covering `target/`, `Cargo.lock` policy (keep it for binaries — it should be committed), editor scratch dirs, and `*.cwasm`.
> 6. **CI** at `.github/workflows/ci.yml` running on `push` and `pull_request` against `main`:
>    - Job matrix: `ubuntu-latest` (required), and a non-blocking `macos-latest` job.
>    - Steps: checkout, install stable toolchain with `rustfmt` + `clippy` components, cache `~/.cargo` and `target/`, then run in order: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build --workspace --release`, `cargo test --workspace`.
>    - Use `dtolnay/rust-toolchain@stable` and `Swatinem/rust-cache@v2` (or current equivalents).
> 7. **Repository hygiene:**
>    - Add a `docs/` directory with a placeholder `docs/plugin-manifest.md` containing only a `# Plugin Manifest (TBD — Phase 5)` heading and a one-line note. `AGENTS.md` already references this path; do not break that reference.
>    - Add `CONTRIBUTING.md` summarising the standing rules listed at the top of this phases document (SPDX, no unwrap, fmt/clippy, approved crates).
>
> **Constraints:**
> - Do **not** implement IPC, search, UI rendering, or plugin loading yet. Stubs only.
> - Do **not** commit `target/` or any build artifacts.
> - Each crate must compile independently with `cargo build -p <crate>`.
> - The full workspace must pass `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo build --release`, and `cargo test` with zero warnings.
>
> When done, prepare a PR titled `Phase 1: workspace foundation, CI, and crate skeletons` from a branch named `phase-1-foundation` if your environment supports branch operations. The PR description must list every deliverable above and link to the relevant section of `AGENTS.md`.

### 1.2 Verification Prompt

> You are an independent reviewer auditing the `phase-1-foundation` branch of SayRat. Do not trust the implementer's claims; verify each item directly against the repository and against `AGENTS.md` / `prd_srs.md`.
>
> Produce a checklist report with **PASS / FAIL / N/A** plus a short justification for each:
>
> 1. Workspace layout matches: `crates/sayrat-protocol`, `crates/sayratd`, `crates/sayrat-ui` exist with the correct crate types (`lib` vs `bin`).
> 2. Root `Cargo.toml` declares `[workspace]` with `resolver = "2"` and a populated `[workspace.dependencies]` table referencing only approved crates from `AGENTS.md` §2.
> 3. `[profile.release]` is size-tuned (LTO, codegen-units = 1, panic = abort, strip, opt-level z/s).
> 4. Every committed `.rs` file starts with the SPDX header `// SPDX-License-Identifier: GPL-3.0-or-later`. Run a grep and report any offenders.
> 5. `LICENSE` is GPL-3.0-or-later, and `README.md`, `CONTRIBUTING.md`, `rustfmt.toml`, `.editorconfig`, `.gitignore` exist and contain the required content.
> 6. `docs/plugin-manifest.md` placeholder exists at the path referenced in `AGENTS.md` §6.
> 7. `.github/workflows/ci.yml` runs, in this order: fmt check, clippy with `-D warnings`, build, test — on at least Ubuntu — and uses caching.
> 8. Run locally (or via CI logs) and confirm:
>    - `cargo fmt --all -- --check` exits 0.
>    - `cargo clippy --workspace --all-targets -- -D warnings` exits 0 with zero warnings.
>    - `cargo build --workspace --release` succeeds.
>    - `cargo test --workspace` succeeds (even if zero tests).
>    - `cargo run -p sayratd -- --version` and `cargo run -p sayrat-ui -- --version` both print and exit cleanly.
> 9. No forbidden dependencies (Electron, Tauri, Wry, WebView2, browser runtimes, or anything outside `AGENTS.md` §2) appear in `Cargo.lock`.
> 10. No `.unwrap()` or `.expect()` outside `#[cfg(test)]` blocks. Run a grep and report any.
>
> Conclude with an overall verdict: **APPROVED**, **APPROVED WITH NITS** (list them), or **REJECTED** (list blocking issues, each tied to a specific deliverable).

---

## Phase 2 — IPC Protocol & Daemon Core

**Goal:** A running `sayratd` that binds a Unix domain socket, accepts client connections, indexes desktop applications into `redb`, watches the filesystem for changes, and answers a minimal request set over a typed `postcard` protocol. Still no UI, no fuzzy search, no Wasm.

### 2.1 Build Prompt

> You are continuing the SayRat project. Phase 1 is merged; Phases 3–5 will follow. Read `prd_srs.md` and `AGENTS.md` again before starting and respect the standing rules at the top of `phases_prompts.md`.
>
> **Deliverables for Phase 2 (IPC Protocol & Daemon Core):**
>
> 1. **Shared protocol crate (`sayrat-protocol`)**
>    - Define `Request` and `Response` enums in `messages.rs`. Minimum variants for this phase:
>      - `Request::Hello { client_version: String }` → `Response::Hello { daemon_version: String, protocol_version: u16 }`
>      - `Request::Ping` → `Response::Pong`
>      - `Request::Shutdown` → `Response::Ack` (only honoured if the request comes over the local socket; document this).
>      - `Request::ListEntries { limit: u16 }` → `Response::Entries { items: Vec<EntryRef<'a>>, more: bool }` where `EntryRef` is a zero-copy borrowed struct (use `Cow<'a, str>` or `&'a str` with a `'de` lifetime where appropriate).
>    - Define `Entry { id: u64, kind: EntryKind, name: String, subtitle: Option<String>, exec: Option<String>, icon: Option<String> }` and `EntryKind { Application, File, PluginCommand }` (only `Application` is populated this phase).
>    - Implement framing helpers: a length-prefixed (`u32` little-endian) `postcard` codec with `read_message<R: Read, T: DeserializeOwned>` / `write_message<W: Write, T: Serialize>` plus async equivalents. Include a `MAX_FRAME_BYTES` constant (e.g., 1 MiB) and reject oversized frames.
>    - Export a `PROTOCOL_VERSION: u16` constant. Bump policy: any wire-incompatible change increments it.
>    - 100% unit-test coverage on round-trip (de)serialization for every `Request`/`Response` variant.
> 2. **Daemon binary (`sayratd`)**
>    - **Async runtime:** use `smol` (or `async-executor` + `async-io`). Do not introduce `tokio`; if you believe it is required, document the justification and use `default-features = false` with the minimal feature set per `AGENTS.md` §5.
>    - **Socket binding:** use `interprocess::local_socket` to listen on `$XDG_RUNTIME_DIR/sayrat.sock` (Linux/macOS), creating the file with `0600` permissions and removing any stale socket on startup. On Windows use the named-pipe equivalent. Make the path overridable via `--socket <path>`.
>    - **Connection handler:** spawn one task per client. Decode `Request`, dispatch via a `Handler` trait, encode `Response`. Disconnects must be logged at `debug` and never crash the daemon.
>    - **State machine:** a `DaemonState` struct holding the index handle, indexer task handle, and a shutdown signal. Implement graceful shutdown on `SIGINT` / `SIGTERM` and on receipt of a `Shutdown` request from the local socket: stop accepting connections, drain in-flight handlers (with a 2 s timeout), close the database, then exit 0.
> 3. **Application indexer**
>    - On startup, scan the standard XDG application directories (`$XDG_DATA_HOME/applications`, `$XDG_DATA_DIRS/applications`, plus the equivalents on macOS/Windows) for `.desktop` entries. Parse with the `freedesktop_entry_parser` crate **only if it is added to the approved list** (otherwise write a minimal hand-rolled parser, since `.desktop` is simple INI). Justify the decision in a comment.
>    - Persist parsed entries to a `redb` database at `$XDG_DATA_HOME/sayrat/index.redb`. Schema: a single table `entries: u64 → postcard(Entry)` plus a `meta` table for schema version and last-scan timestamp.
>    - Implement a `FullRescan` and an `IncrementalUpdate(path)` operation. Both must be idempotent.
> 4. **Background filesystem watcher**
>    - Use `notify` (Linux/macOS) or the OS-native equivalent only after updating the approved list in `AGENTS.md` with justification. If the team rejects it, fall back to a periodic rescan every 60 s and document the trade-off.
>    - Watch the same XDG application directories. Debounce events (200 ms) and trigger `IncrementalUpdate` for changed files only.
> 5. **Logging & error handling**
>    - Use `log` + `env_logger` by default. Add `tracing`/`tracing-subscriber` only behind a feature flag if the phase needs spans for measured latency evidence, and document the footprint impact.
>    - All daemon errors derive `thiserror::Error`. The `main` function uses `anyhow::Result`. Every IPC disconnect is handled, never panicked.
> 6. **Tests**
>    - Unit tests for the protocol round-trip (already required above).
>    - Integration test: spawn `sayratd` with a temp socket path and a temp `redb` directory, send a `Hello` and a `Ping` from a test client built on `sayrat-protocol`, assert the responses.
>    - Indexer test: drop two synthetic `.desktop` files into a temp dir, point the indexer at it, assert that both `Entry` rows appear in `redb`.
>
> **Constraints:**
> - Hot-path code (request decode → handler dispatch → response encode) must avoid heap allocations beyond the postcard buffer. No `String::from`/`format!` in handlers.
> - The daemon's idle RSS, measured by `/proc/self/statm` after a 5-second warmup with a 100-entry index, must be **< 15 MB** on Linux. Add a `cargo run -p sayratd -- --measure-rss` mode that prints the value and exits, and document the result in the PR.
> - No new approved-list crates without an explicit justification block in the PR description naming the subsystem and rejected alternatives.
>
> Prepare a PR titled `Phase 2: protocol crate, daemon, indexer, and filesystem watcher` from branch `phase-2-daemon-ipc` if branch operations are available. The PR body must include the measured idle RSS.

### 2.2 Verification Prompt

> Audit the `phase-2-daemon-ipc` branch. For each item below, mark **PASS / FAIL** with evidence (file paths, line numbers, command output).
>
> 1. `sayrat-protocol` exports `Request`, `Response`, `Entry`, `EntryKind`, `PROTOCOL_VERSION`, framing helpers, and `MAX_FRAME_BYTES`. Round-trip unit tests exist for every variant and pass.
> 2. Wire format is length-prefixed `postcard`. Frames over `MAX_FRAME_BYTES` are rejected, not panicked. Verify with a unit test or write one.
> 3. `sayratd` uses `interprocess::local_socket`, binds at `$XDG_RUNTIME_DIR/sayrat.sock` (or `--socket` override), creates the file with `0600` permissions on Unix, and removes stale sockets on startup. Confirm by reading the source and by running it twice in succession.
> 4. The async runtime is `smol` or another lightweight executor. If `tokio` is present, confirm `default-features = false` and only the minimal feature set, with a justification in the PR.
> 5. Graceful shutdown works: send `SIGINT` to a running daemon and confirm the socket file is removed and the process exits 0 within 2 seconds. Repeat with a `Shutdown` request.
> 6. Indexer correctness:
>    - Drop a synthetic `.desktop` file into a watched directory; confirm a new row appears in `redb` within ~250 ms.
>    - Modify the file; confirm the row updates without duplication.
>    - Delete the file; confirm the row is removed.
> 7. Inspect `redb` schema: tables `entries` and `meta` exist; entries are postcard-encoded; `meta` records the schema version.
> 8. Hot-path allocation review: read the request handler and confirm no `String::from` / `format!` / `Vec::new` is invoked per request beyond the postcard scratch buffer.
> 9. Run `cargo run -p sayratd -- --measure-rss` after the daemon has indexed at least 100 synthetic entries. Idle RSS must be **< 15 MB**. Record the value.
> 10. Standing rules: SPDX headers on all new files, no `.unwrap()`/`.expect()` outside tests, `cargo fmt --check` and `cargo clippy -- -D warnings` clean, integration tests passing in CI.
>
> Conclude with **APPROVED / APPROVED WITH NITS / REJECTED** and reference each failed deliverable.

---

## Phase 3 — Fuzzy Search Engine & Chunked Streaming

**Goal:** The daemon answers `Search { query }` requests with chunked, streamed, ranked results using `nucleo`. Support typed cancellation when a new query supersedes an old one. Still no UI; tested entirely via a CLI test client.

### 3.1 Build Prompt

> Continuing SayRat. Phase 2 is merged. Read `AGENTS.md` §4.1 carefully — this phase implements the chunked-streaming mitigation for the Slint VectorModel bottleneck.
>
> **Deliverables for Phase 3 (Fuzzy Search & Streaming):**
>
> 1. **Protocol additions in `sayrat-protocol`**
>    - `Request::Search { query: String, query_id: u64, limit: u16 }`.
>    - `Request::CancelSearch { query_id: u64 }`.
>    - `Response::SearchChunk { query_id: u64, chunk_index: u16, items: Vec<Match>, more: bool }` where `Match { entry_id: u64, score: i32, indices: Vec<u16> }` (`indices` are the matched character positions for highlight rendering).
>    - Bump `PROTOCOL_VERSION`. Update round-trip tests.
> 2. **Fuzzy matcher in `sayratd`**
>    - Use `nucleo` (primary). Build a single shared `Nucleo` matcher instance owned by `DaemonState`.
>    - Feed it the indexed `Entry` corpus on startup and on each `IncrementalUpdate`.
>    - On `Search`, push the query into `nucleo`'s injector and consume ranked results from its tick output.
>    - **Streaming contract:** flush results to the client in chunks of **≤ 50 matches**. The first chunk must reach the client within **< 16 ms** of receiving the request, even if `nucleo` has not finished. Subsequent chunks follow as more results become available, up to `limit` total or until the matcher reports completion. The final chunk has `more = false`.
>    - **Cancellation:** on `CancelSearch { query_id }` or on receipt of a newer `Search` from the same connection, abort the in-flight search promptly. The cancelled stream emits one final empty chunk with `more = false` so the client can clean up.
> 3. **Hot-path discipline**
>    - The query handler must not allocate per keystroke beyond `nucleo`'s internal buffers and the postcard scratch buffer. Document, with a profiling note in the PR, that input-to-first-chunk on a 10k-entry corpus stays under 16 ms on a developer laptop. Provide a `cargo bench` or a `--bench-search <corpus_size>` flag on `sayratd` to reproduce.
>    - Reuse `Vec<Match>` buffers between chunks via a small free-list or `mem::take` pattern.
> 4. **Reference test client**
>    - Add `crates/sayrat-cli` (a tiny `bin` crate; new addition — declare in the workspace) that connects to `sayratd`, sends a `Search`, and prints the streamed chunks with timestamps. Used for development and verification, not shipped to end users. Include a `--cancel-after <ms>` flag to exercise cancellation.
> 5. **Tests**
>    - Unit: protocol round-trip for the new variants.
>    - Integration: spawn `sayratd` with a 1k-entry synthetic corpus, send `Search { query: "fox" }`, assert that (a) the first chunk arrives in < 16 ms, (b) chunks have `≤ 50` items, (c) total items match an oracle implementation, and (d) cancellation aborts within < 5 ms.
>    - Property test (with `proptest` or hand-rolled randomized): random queries against random corpora never panic, never produce duplicates, and respect `limit`.
>
> **Constraints:**
> - Do not regress Phase 2: existing requests still work, idle RSS still < 15 MB, fmt/clippy still clean.
> - If `nucleo` cannot satisfy the 16 ms first-chunk SLA on the test machine, fall back to the approved alternative `frizbee` and document the switch in the PR.
> - Do not implement the UI yet. The agent's job ends when the CLI test client demonstrates the full search lifecycle.
>
> Prepare a PR titled `Phase 3: nucleo-backed search with chunked streaming and cancellation` from branch `phase-3-fuzzy-search` if branch operations are available. The PR body must include benchmark numbers.

### 3.2 Verification Prompt

> Audit `phase-3-fuzzy-search`. Mark each item PASS/FAIL with evidence.
>
> 1. New protocol variants (`Search`, `CancelSearch`, `SearchChunk`, `Match`) exist; round-trip tests cover them; `PROTOCOL_VERSION` was bumped.
> 2. `sayratd` uses `nucleo` (or documented `frizbee` fallback) with a single shared matcher fed from the `redb` corpus.
> 3. Build and run the daemon with a 10k-entry synthetic corpus (use `--bench-search 10000` or equivalent). Confirm the first chunk reaches the client in **< 16 ms** at the p50 and stays under **~30 ms** at p99 on a typical developer laptop. Capture the numbers.
> 4. Each chunk contains **≤ 50** items. The final chunk has `more = false`. No chunk is ever empty except the cancellation tombstone.
> 5. Cancellation: send `Search`, then `CancelSearch` after 1 ms. The daemon must stop emitting chunks within ~5 ms and emit exactly one empty `more = false` tombstone. Confirm via the CLI test client and via an integration test.
> 6. Issuing a new `Search` on the same connection while a previous one is in flight cancels the previous query (no interleaved chunks across `query_id`s).
> 7. Hot-path allocation review: confirm `Vec<Match>` buffers are reused, no per-keystroke `String::from`/`format!` in the search path.
> 8. Run the property/fuzz test and confirm zero panics across at least 1,000 randomized cases.
> 9. Phase 2 functionality is unbroken: `Hello`, `Ping`, indexer, watcher, graceful shutdown all still pass their integration tests.
> 10. Standing rules: SPDX, no unwrap/expect, fmt + clippy clean, CI green.
>
> Verdict: **APPROVED / APPROVED WITH NITS / REJECTED**.

---

## Phase 4 — UI Client: Slint + Wayland Layer Shell

**Goal:** A working overlay launcher. `sayrat-ui` opens a frameless overlay via `wlr-layer-shell-unstable-v1`, renders a Slint search bar + result list backed by a `PagedResultModel` consuming Phase 3 chunks, and supports sub-frame show/hide via buffer detach. A global hotkey (with D-Bus fallback) toggles visibility.

### 4.1 Build Prompt

> Continuing SayRat. Phase 3 is merged. Read `AGENTS.md` §4.1 (paged model) and §4.3 (Wayland visibility) carefully — both have **mandatory** solutions.
>
> **Deliverables for Phase 4 (UI Client):**
>
> 1. **Slint UI definition (`crates/sayrat-ui/ui/launcher.slint`)**
>    - A frameless 600×400 overlay with rounded corners, a single-line search input at the top, and a vertically scrolling list of results below.
>    - Each result row shows: icon (16×16, optional), `name` (bold), `subtitle` (dim, optional). Highlighted matches use the `indices` from the `Match` payload to bold the matched characters.
>    - Keyboard navigation: `Up`/`Down` to move selection, `Enter` to activate, `Esc` to hide.
>    - Expose a `PagedResultModel` Slint `Model` implementation (in Rust) that the daemon's `SearchChunk`s feed into. Appending a chunk must **not** trigger `row_data_changed` for already-rendered rows; it must only emit `row_added` per new row.
> 2. **Wayland layer-shell integration**
>    - Use `smithay-client-toolkit` + `wayland-protocols` to create an `Overlay` layer surface using `wlr-layer-shell-unstable-v1`.
>    - Anchor: top-center, with a configurable Y offset. Margin: 100 px from the top by default.
>    - Keyboard interactivity defaults to `Exclusive`, with `OnDemand` as a fallback for compositors that reject `Exclusive`. Implement and log the fallback.
>    - **Sub-frame show/hide (mandatory):** to hide, detach the `wl_buffer`, commit a null buffer, set `KeyboardInteractivity::None`. To show, reattach the active Slint buffer and restore interactivity to `Exclusive` (or `OnDemand`). Do **not** destroy or recreate the surface across toggles.
> 3. **Global hotkey**
>    - Use the `global-hotkey` crate to bind `Super+Space` (configurable later). On Wayland compositors that do not honour it, fall back to a D-Bus interface registered via `zbus` so external tools (e.g., Sway/Hyprland keybinds) can call `org.sayrat.Launcher.Toggle`.
>    - Pressing the hotkey while hidden → show + grab keyboard. Pressing while shown → hide. Pressing `Esc` while shown → hide.
> 4. **IPC client wiring**
>    - On startup, `sayrat-ui` connects to `sayratd`'s socket and sends `Hello`. If the daemon is not running, attempt to start it (spawn detached) once, then retry for up to 1 s before erroring out.
>    - On every keystroke in the search input, send `Search { query, query_id = monotonic counter, limit = 200 }`. The previous in-flight `query_id` is implicitly cancelled by the daemon when the new one arrives; there is no need to send `CancelSearch` here.
>    - Stream `SearchChunk` payloads into the `PagedResultModel`. Render the first chunk immediately; do not wait for `more = false`.
>    - On disconnect, retry with exponential backoff (start at 100 ms, cap at 5 s); show an unobtrusive "Reconnecting..." status row.
> 5. **Activation**
>    - On `Enter`, send `Activate { entry_id }` (new protocol variant — bump `PROTOCOL_VERSION` and add round-trip tests). The daemon launches the application via `Exec=` parsing per the freedesktop spec, detached, with environment scrubbed of UI-specific variables. Hide the UI immediately on activation.
> 6. **Tests**
>    - Unit: `PagedResultModel` correctness — appending chunks emits only `row_added`, never `row_data_changed`.
>    - Integration (headless, no real compositor): mock the IPC layer and drive the model with synthetic chunks; assert frame-time bounds.
>    - Manual smoke test instructions in the PR for Sway and/or Hyprland: launch daemon, launch UI, hit the hotkey, type, navigate, activate, hide.
>
> **Constraints:**
> - UI client RSS after first show must be **< 5 MB** on Linux. Add `--measure-rss` mode and report the value in the PR.
> - Input-to-first-frame latency must stay **< 16 ms**. Wire lightweight timing around `keystroke → first chunk rendered` (a feature-gated tracing span is acceptable) and report p50/p99 in the PR.
> - The UI must never perform filesystem scans, database queries, or plugin work. If you find yourself reaching for `std::fs` or `redb` here, stop and route through IPC instead.
> - macOS / Windows support: the layer-shell and hotkey paths must compile-gate cleanly on those platforms even if they are not fully functional yet (use `#[cfg(target_os = "linux")]` blocks and stub the rest with a lightweight warning log).
>
> Prepare a PR titled `Phase 4: Slint overlay UI with wlr-layer-shell and paged result model` from branch `phase-4-ui-layer-shell` if branch operations are available.

### 4.2 Verification Prompt

> Audit `phase-4-ui-layer-shell`. PASS/FAIL with evidence for each.
>
> 1. `crates/sayrat-ui/ui/launcher.slint` exists with the search input + result list described above. Build the UI client and confirm it compiles cleanly.
> 2. The Wayland surface is created with `wlr-layer-shell-unstable-v1` at the `Overlay` layer. Run on Sway or Hyprland and confirm the window appears as a floating overlay above all other clients.
> 3. Show/hide implementation: read the source. Confirm that hiding **detaches the `wl_buffer`, commits a null buffer, sets `KeyboardInteractivity::None`** — and does **not** destroy the surface. Confirm that showing reattaches and restores interactivity. Toggle 100 times and confirm zero leaks (RSS stable).
> 4. Hotkey: `Super+Space` toggles visibility on at least one tested compositor. The D-Bus fallback `org.sayrat.Launcher.Toggle` is callable via `gdbus`/`busctl` and works.
> 5. `PagedResultModel` review: appending a `SearchChunk` only emits `row_added` events; existing rows are never re-rendered. Verify with the unit test and read the source to confirm no `set_vec`/`reset` calls per chunk.
> 6. Type a query that returns 500 matches. Confirm the first 50 appear within 16 ms (use the timing evidence added by the implementation), and subsequent rows stream in without dropped frames. Capture screenshots or a screen recording in the PR.
> 7. `--measure-rss` after first show reports **< 5 MB**. Capture the value.
> 8. Activation: select an entry, press Enter, confirm the application launches detached and the UI hides immediately.
> 9. Reconnect logic: kill the daemon while the UI is open. Confirm a "Reconnecting..." status appears, the UI stays alive, and reconnection succeeds when the daemon comes back.
> 10. Cross-platform compile: `cargo check -p sayrat-ui --target x86_64-pc-windows-gnu` (if available) and `--target x86_64-apple-darwin` succeed (functional on Linux only is fine).
> 11. Standing rules: SPDX, no unwrap/expect, fmt + clippy clean, no forbidden deps.
>
> Verdict: **APPROVED / APPROVED WITH NITS / REJECTED**.

---

## Phase 5 — Wasm Plugin System & Hardening

**Goal:** Plugins extend the launcher. The daemon loads `.cwasm` modules under `wasmtime` with a pooling allocator, validates a TOML manifest, enforces zero-ambient-authority capabilities, and exposes plugin-provided commands as additional search results. Final phase also closes out memory-budget gates, security review, and release tooling.

### 5.1 Build Prompt

> Final phase. Read `AGENTS.md` §4.2 (Wasm bloat mitigation), §3 (security model), and §6 (manifest convention) carefully. Treat them as binding.
>
> **Deliverables for Phase 5 (Plugins & Hardening):**
>
> 1. **Plugin manifest schema**
>    - Replace the placeholder `docs/plugin-manifest.md` with the canonical schema. TOML format. Required fields:
>      ```
>      [plugin]
>      id = "com.example.weather"
>      name = "Weather"
>      version = "0.1.0"
>      entrypoint = "weather.cwasm"
>
>      [capabilities]
>      filesystem = []           # list of allowed absolute paths, default empty
>      network    = []           # list of allowed host:port patterns, default empty
>      env        = []           # list of allowed env var names, default empty
>      ```
>    - Implement a `sayrat-protocol::manifest` module (or sibling crate `sayrat-plugin-manifest`) that parses, validates (semver, ID format, no `..` in paths), and exposes the manifest as a strongly typed struct. Unknown TOML keys are rejected.
> 2. **Wasm host in `sayratd`**
>    - Use `wasmtime` configured with:
>      - `PoolingAllocationConfig` capping per-instance memory (e.g., 16 MiB) and table size.
>      - `Config::cranelift_opt_level(OptLevel::None)` if dynamic loading is supported, **and** prefer the AOT path: ship a `sayratctl plugin install <path>` subcommand (in `sayrat-cli`) that compiles `.wasm` → `.cwasm` once at install time and stores it under `$XDG_DATA_HOME/sayrat/plugins/`.
>    - Plugins are discovered at daemon startup by scanning `$XDG_DATA_HOME/sayrat/plugins/*/manifest.toml`. Each plugin gets its own `Store<PluginCtx>`.
>    - **Zero ambient authority**: build a `wasmtime_wasi::WasiCtx` that grants exactly the directories, env vars, and (if implemented) network sockets declared in the manifest, and **nothing else**. Anything undeclared must result in a permission-denied trap, not a silent fallback.
>    - Enforce per-call execution budget via `Engine::epoch_interruption` (e.g., 100 ms wall clock); plugins that exceed it are killed cleanly without taking down the daemon.
> 3. **Plugin ABI**
>    - Define a minimal stable ABI in `sayrat-protocol::plugin_abi`:
>      - Exported by the plugin: `sayrat_query(query_ptr: u32, query_len: u32) -> u64` returning a packed `(ptr, len)` to a postcard-encoded `Vec<PluginEntry>`.
>      - Exported by the host: `sayrat_log(level: u32, msg_ptr: u32, msg_len: u32)`.
>    - Document the ABI in `docs/plugin-abi.md` with a worked example and a tiny reference plugin under `examples/plugin-echo/` that, given any query, returns `[PluginEntry { name: query.to_uppercase(), ... }]`.
>    - The reference plugin must build to `.wasm` via `cargo build --target wasm32-wasip1 -p plugin-echo` and be installable via `sayratctl plugin install`.
> 4. **Search integration**
>    - On every `Search`, the daemon fans out the query to all loaded plugins **in parallel**, with a hard timeout of 50 ms aggregated. Plugin results are merged into the `nucleo` stream as additional `Entry { kind: PluginCommand, ... }` records, scored by the host (do not trust plugin-supplied scores).
>    - Activating a `PluginCommand` sends `Activate { entry_id }` as before; the daemon dispatches to the originating plugin's `sayrat_activate(entry_id: u64)` exported function.
> 5. **Memory-budget gates in CI**
>    - Add a CI job that builds the workspace in release mode, launches `sayratd` and `sayrat-ui` headlessly, drives a synthetic workload (1k entries, 50 keystrokes), and asserts:
>      - `sayratd` idle RSS < 15 MB.
>      - `sayrat-ui` post-show RSS < 5 MB.
>      - Combined RSS < 20 MB.
>    - The job fails the build on regression. Use `cargo run -- --measure-rss` modes from earlier phases.
> 6. **Release polish**
>    - `cargo deny` configuration checking licenses (deny anything stricter than GPL-3.0-or-later compatible) and known advisories. Add a CI step.
>    - `CHANGELOG.md` seeded with one entry per phase.
>    - `man/` pages or at least a `--help` epilogue for both binaries describing daemon socket, plugin install flow, and hotkey config.
>    - GitHub Actions release workflow that builds tagged releases for `x86_64-unknown-linux-gnu` and `x86_64-unknown-linux-musl`, uploads stripped binaries, and attaches a `SHA256SUMS` file.
> 7. **Tests**
>    - Round-trip tests for the manifest parser, including rejection cases (unknown keys, traversal paths, bad semver).
>    - Sandboxing test: a malicious test plugin attempts `wasi::fs::open("/etc/passwd")` and `wasi::sockets::connect`; both must trap with permission errors and the daemon must remain alive.
>    - Epoch-interruption test: a plugin with an infinite loop is killed within ~120 ms and does not leak its `Store`.
>    - End-to-end test: install `examples/plugin-echo`, start the daemon + UI, search for `hello`, confirm `HELLO` appears as a `PluginCommand` row.
>
> **Constraints:**
> - No new approved-list crates beyond what `AGENTS.md` §2 already names, except `wasmtime-wasi` (assumed implicit) and `cargo-deny` (CI-only). Anything else needs an explicit justification in the PR.
> - Plugins must never default to having any capability. The default `WasiCtx` is empty; capabilities are added only after manifest validation.
> - Memory budgets are hard gates: a regression in CI fails the build.
>
> Prepare a PR titled `Phase 5: wasmtime plugin sandbox, capability manifests, memory gates, and release tooling` from branch `phase-5-plugins-hardening` if branch operations are available. Tag `v0.1.0` only after the PR is merged and release checks pass.

### 5.2 Verification Prompt

> Audit `phase-5-plugins-hardening`. PASS/FAIL with evidence.
>
> 1. `docs/plugin-manifest.md` defines the canonical TOML schema. Manifest parser rejects unknown keys, path traversal, and bad semver — confirm via the included tests.
> 2. `wasmtime` host configuration:
>    - Read the source. Confirm `PoolingAllocationConfig` is set with explicit per-instance caps.
>    - Confirm AOT path: `sayratctl plugin install` compiles `.wasm` → `.cwasm` at install time. Run it on `examples/plugin-echo` and confirm the resulting `.cwasm` file appears under `$XDG_DATA_HOME/sayrat/plugins/`.
>    - Confirm epoch-interruption is enabled with a sane budget.
> 3. Capability enforcement:
>    - Run the malicious-plugin test (or write one if missing): a plugin attempts undeclared filesystem and network access. Both must trap with permission errors. The daemon process must remain alive (PID unchanged, socket still serving).
>    - Confirm `WasiCtx` starts empty and is populated *only* from manifest declarations.
> 4. Default deny: a plugin with an empty `[capabilities]` block has zero filesystem, network, and env access. Verify by code review and by running the malicious test with that manifest.
> 5. Search integration: install the echo plugin, start daemon + UI, type `hello`, confirm a `PluginCommand` result with name `HELLO` appears within the same chunk window as native results. Aggregate plugin timeout is observed (kill the echo plugin's main loop with a sleep and confirm it is dropped from results, not the whole search).
> 6. Memory gates in CI:
>    - Inspect the new CI job. Confirm it asserts `sayratd < 15 MB`, `sayrat-ui < 5 MB`, combined `< 20 MB`.
>    - Trigger a deliberate regression (e.g., add a `Vec<u8>` of 10 MB to `DaemonState`) on a throwaway branch and confirm CI fails. Revert.
> 7. `cargo deny check licenses advisories` runs in CI and passes. No GPL-incompatible dependency is present.
> 8. Release workflow: tag a dry-run prerelease (`v0.1.0-rc1`), confirm the workflow produces stripped `x86_64-unknown-linux-gnu` and `x86_64-unknown-linux-musl` binaries plus `SHA256SUMS`.
> 9. End-to-end smoke (manual or scripted): cold start daemon → UI hotkey → type `ec` → echo plugin result appears → `Enter` activates → UI hides → daemon still serves.
> 10. Standing rules across the whole tree: every file has SPDX header, no `.unwrap()`/`.expect()` outside tests, fmt + clippy clean across all crates, `AGENTS.md` constraints respected (no Electron/Tauri/etc., no unapproved deps).
>
> Final verdict: **SHIP IT** / **APPROVED WITH NITS** / **REJECTED**, with a one-paragraph executive summary covering correctness, performance, and security posture.

---

## Cross-Cutting Notes for All Phases

- **Branching:** every phase produces exactly one PR off `main`. Do not stack phase branches; merge sequentially.
- **PR description template:** every PR must include (a) a checklist mapping each deliverable in the build prompt to a commit or file, (b) measured numbers (RSS, latency) where applicable, (c) any deviation from `AGENTS.md` with explicit justification.
- **If a build prompt conflicts with `AGENTS.md`/`prd_srs.md`:** the canonical docs win. The agent must surface the conflict in the PR description rather than silently resolve it.
- **Verification independence:** ideally run each Verification Prompt in a fresh agent session with no memory of the Build Prompt, so the audit is genuinely adversarial.
