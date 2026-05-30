// SPDX-License-Identifier: GPL-3.0-or-later

//! `sayrat-protocol` — shared IPC vocabulary between [`sayratd`] (the
//! background daemon) and [`sayrat-ui`] (the ephemeral overlay client).
//!
//! Per [`AGENTS.md` §1] the two processes are strictly decoupled and may
//! only communicate through the wire format defined here. To keep the
//! UI client "dumb" and the daemon authoritative, every value that
//! crosses the socket must be expressible as a type in [`messages`].
//!
//! Phase 1 ships only the module skeleton; concrete request / response
//! enums, chunked-result envelopes, and the `postcard` derives land in
//! Phase 2 alongside the IPC transport.
//!
//! [`AGENTS.md` §1]: ../../../AGENTS.md

#![forbid(unsafe_code)]
#![deny(missing_docs)]

/// IPC message catalogue. Populated in Phase 2.
///
/// All cross-process payloads — keystroke updates, paged result chunks,
/// plugin lifecycle events — will be defined here and serialised with
/// `postcard` (see AGENTS.md §2).
pub mod messages {}
