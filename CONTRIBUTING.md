# Contributing to SayRat

Thanks for considering a contribution. SayRat targets a strict sub-20 MB
total memory footprint and a Wayland-first dual-process architecture, so
the bar for merging code is deliberately high. Please read
[`AGENTS.md`](./AGENTS.md) end-to-end before sending a patch — that file
is the governing reference and supersedes anything below if they conflict.

## Standing rules

These apply to every PR, every phase, every contributor (human or AI):

1. **SPDX header.** Every `.rs` file must begin with
   `// SPDX-License-Identifier: GPL-3.0-or-later`. The project is
   GPL-3.0-or-later; the full text lives in [`LICENSE`](./LICENSE).

2. **No `unwrap` / no `expect`.** Use `thiserror` for library error
   types and `anyhow` for top-level binary error plumbing. IPC
   disconnections must trigger reconnect / graceful shutdown, never a
   panic. (See [`AGENTS.md` §5](./AGENTS.md#5-agent-operating-model--code-style-constraints).)

3. **Formatting & lints.** CI runs

   ```sh
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   ```

   and rejects any warning. Run both locally before pushing.

4. **Approved crates only.** New dependencies must already be listed in
   [`AGENTS.md` §2 — Hardened Technology Stack](./AGENTS.md#2-hardened-technology-stack).
   If you genuinely need something outside the list, update `AGENTS.md` with
   a justification block covering subsystem, rejected alternatives, and
   estimated binary-size / RSS impact. Pin used dependency versions through
   `[workspace.dependencies]` in the root `Cargo.toml`; member crates
   inherit via `dep.workspace = true`.

5. **No web runtimes.** Electron, Tauri, Wry, WebView2, browser
   embeddings — none of them, ever.

6. **Hot-path allocations.** Reuse buffers and prefer zero-copy parsing
   (`postcard::from_bytes` straight off the IPC stream). Heap traffic
   inside the keystroke loop will be flagged in review.

## PR checklist

- [ ] SPDX headers present on new `.rs` files.
- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo build --workspace --release` succeeds.
- [ ] `cargo test --workspace` passes.
- [ ] No new dependencies outside the approved list without an `AGENTS.md`
      update and PR justification block.
- [ ] Memory-budget impact noted in the PR description if you touch
      `sayratd` or `sayrat-ui` runtime paths.
