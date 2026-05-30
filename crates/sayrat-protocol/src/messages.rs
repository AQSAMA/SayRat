// SPDX-License-Identifier: GPL-3.0-or-later

//! Wire message definitions for SayRat IPC.
//!
//! `Request::Shutdown` is honored only when it arrives on the local daemon
//! socket that the current user can access. Remote transports, if ever added,
//! must reject it.

use std::borrow::Cow;

/// Entry category exposed by the daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// A desktop application entry discovered from application directories.
    Application,
    /// A filesystem result. Populated in a later phase.
    File,
    /// A plugin-provided command. Populated in a later phase.
    PluginCommand,
}

/// Owned entry persisted by the daemon index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Stable daemon-local identifier.
    pub id: u64,
    /// Entry kind.
    pub kind: EntryKind,
    /// Display name.
    pub name: String,
    /// Secondary text.
    pub subtitle: Option<String>,
    /// Command line for application entries.
    pub exec: Option<String>,
    /// Icon name or path.
    pub icon: Option<String>,
}

/// Borrowed entry reference returned over list/search responses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryRef<'a> {
    /// Stable daemon-local identifier.
    pub id: u64,
    /// Entry kind.
    pub kind: EntryKind,
    /// Display name.
    pub name: Cow<'a, str>,
    /// Secondary text.
    pub subtitle: Option<Cow<'a, str>>,
    /// Command line for application entries.
    pub exec: Option<Cow<'a, str>>,
    /// Icon name or path.
    pub icon: Option<Cow<'a, str>>,
}

impl<'a> From<&'a Entry> for EntryRef<'a> {
    fn from(entry: &'a Entry) -> Self {
        Self {
            id: entry.id,
            kind: entry.kind,
            name: Cow::Borrowed(&entry.name),
            subtitle: entry.subtitle.as_deref().map(Cow::Borrowed),
            exec: entry.exec.as_deref().map(Cow::Borrowed),
            icon: entry.icon.as_deref().map(Cow::Borrowed),
        }
    }
}

/// Requests accepted by the daemon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// Version negotiation.
    Hello {
        /// Client package/protocol version string.
        client_version: String,
    },
    /// Liveness probe.
    Ping,
    /// Graceful daemon shutdown. Only honored on the local socket.
    Shutdown,
    /// List indexed entries, capped by `limit`.
    ListEntries {
        /// Maximum number of entries to return.
        limit: u16,
    },
}

/// Responses emitted by the daemon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Response<'a> {
    /// Version negotiation response.
    Hello {
        /// Daemon package version string.
        daemon_version: String,
        /// Wire protocol version.
        protocol_version: u16,
    },
    /// Liveness response.
    Pong,
    /// Generic acknowledgement.
    Ack,
    /// Entry listing response.
    Entries {
        /// Returned items.
        items: Vec<EntryRef<'a>>,
        /// True when more entries exist after this page.
        more: bool,
    },
    /// Request failed.
    Error {
        /// Human-readable error.
        message: String,
    },
}
