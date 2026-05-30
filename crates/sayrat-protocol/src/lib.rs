// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared IPC vocabulary between `sayratd` and `sayrat-ui`.
//!
//! The daemon and UI are strictly decoupled; all cross-process values live in
//! [`messages`] and are transported with the length-prefixed codec in
//! [`codec`]. `PROTOCOL_VERSION` must be incremented for any wire-incompatible
//! change.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

/// Current SayRat IPC protocol version.
pub const PROTOCOL_VERSION: u16 = 1;

pub mod codec;
pub mod messages;
