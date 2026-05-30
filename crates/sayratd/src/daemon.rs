// SPDX-License-Identifier: GPL-3.0-or-later

//! IPC server and daemon state machine.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use sayrat_protocol::PROTOCOL_VERSION;
use sayrat_protocol::codec;
use sayrat_protocol::messages::{EntryRef, Request, Response};

use crate::indexer::{AppIndex, IndexError, IndexOperation};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};

/// Daemon error.
#[derive(Debug)]
pub enum DaemonError {
    /// I/O failed.
    Io(io::Error),
    /// IPC codec failed.
    Codec(codec::CodecError),
    /// Index operation failed.
    Index(IndexError),
    /// Unsupported platform feature.
    Unsupported(&'static str),
}

impl std::fmt::Display for DaemonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::Codec(err) => write!(f, "codec error: {err}"),
            Self::Index(err) => write!(f, "index error: {err}"),
            Self::Unsupported(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for DaemonError {}

impl From<io::Error> for DaemonError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<codec::CodecError> for DaemonError {
    fn from(value: codec::CodecError) -> Self {
        Self::Codec(value)
    }
}

impl From<IndexError> for DaemonError {
    fn from(value: IndexError) -> Self {
        Self::Index(value)
    }
}

/// Daemon result.
pub type Result<T> = std::result::Result<T, DaemonError>;

/// Request dispatcher.
pub trait Handler: Send + Sync + 'static {
    /// Dispatch one request.
    fn handle(&self, request: Request) -> Result<Response<'static>>;
}

/// Shared daemon state.
pub struct DaemonState {
    index: AppIndex,
    shutdown: AtomicBool,
    in_flight: AtomicUsize,
    drain: (Mutex<()>, Condvar),
    _indexer_task: Mutex<Option<JoinHandle<()>>>,
}

impl DaemonState {
    /// Create state and perform startup full rescan.
    pub fn new(index: AppIndex) -> Result<Arc<Self>> {
        index.apply(IndexOperation::FullRescan)?;
        Ok(Arc::new(Self {
            index,
            shutdown: AtomicBool::new(false),
            in_flight: AtomicUsize::new(0),
            drain: (Mutex::new(()), Condvar::new()),
            _indexer_task: Mutex::new(None),
        }))
    }

    /// Request graceful shutdown.
    pub fn request_shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }

    /// Whether shutdown has been requested.
    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::SeqCst)
    }

    /// Drain in-flight handlers for up to `timeout`.
    pub fn drain_in_flight(&self, timeout: Duration) {
        let start = Instant::now();
        let mut guard = match self.drain.0.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        while self.in_flight.load(Ordering::SeqCst) != 0 {
            let elapsed = start.elapsed();
            if elapsed >= timeout {
                break;
            }
            let remaining = timeout.saturating_sub(elapsed);
            match self.drain.1.wait_timeout(guard, remaining) {
                Ok((next, _)) => guard = next,
                Err(poisoned) => guard = poisoned.into_inner().0,
            }
        }
    }

    fn enter_handler(self: &Arc<Self>) -> HandlerGuard {
        self.in_flight.fetch_add(1, Ordering::SeqCst);
        HandlerGuard { state: Arc::clone(self) }
    }
}

impl Handler for DaemonState {
    fn handle(&self, request: Request) -> Result<Response<'static>> {
        match request {
            Request::Hello { client_version: _ } => Ok(Response::Hello {
                daemon_version: env!("CARGO_PKG_VERSION").to_owned(),
                protocol_version: PROTOCOL_VERSION,
            }),
            Request::Ping => Ok(Response::Pong),
            Request::Shutdown => {
                self.request_shutdown();
                Ok(Response::Ack)
            }
            Request::ListEntries { limit } => {
                let (entries, more) = self.index.list_entries(limit)?;
                let items = entries
                    .into_iter()
                    .map(|entry| EntryRef {
                        id: entry.id,
                        kind: entry.kind,
                        name: entry.name.into(),
                        subtitle: entry.subtitle.map(Into::into),
                        exec: entry.exec.map(Into::into),
                        icon: entry.icon.map(Into::into),
                    })
                    .collect();
                Ok(Response::Entries { items, more })
            }
        }
    }
}

struct HandlerGuard {
    state: Arc<DaemonState>,
}

impl Drop for HandlerGuard {
    fn drop(&mut self) {
        self.state.in_flight.fetch_sub(1, Ordering::SeqCst);
        self.state.drain.1.notify_all();
    }
}

/// Run the daemon until shutdown.
pub fn run(socket_path: &Path, index: AppIndex) -> Result<()> {
    let state = DaemonState::new(index)?;
    run_with_state(socket_path, state)
}

/// Run with prebuilt state. Useful for tests.
pub fn run_with_state(socket_path: &Path, state: Arc<DaemonState>) -> Result<()> {
    bind_and_accept(socket_path, state)?;
    Ok(())
}

#[cfg(unix)]
fn bind_and_accept(socket_path: &Path, state: Arc<DaemonState>) -> Result<()> {
    if socket_path.exists() {
        std::fs::remove_file(socket_path)?;
    }
    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let listener = UnixListener::bind(socket_path)?;
    std::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(0o600))?;
    listener.set_nonblocking(true)?;
    log::info!("sayratd listening on {}", socket_path.display());

    while !state.is_shutdown() {
        match listener.accept() {
            Ok((stream, _addr)) => {
                let state = Arc::clone(&state);
                thread::spawn(move || handle_client(stream, state));
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(20));
            }
            Err(err) => return Err(DaemonError::Io(err)),
        }
    }

    state.drain_in_flight(Duration::from_secs(2));
    if let Err(err) = std::fs::remove_file(socket_path)
        && err.kind() != io::ErrorKind::NotFound
    {
        log::debug!("socket cleanup failed: {err}");
    }
    Ok(())
}

#[cfg(not(unix))]
fn bind_and_accept(_socket_path: &Path, _state: Arc<DaemonState>) -> Result<()> {
    Err(DaemonError::Unsupported("named-pipe IPC is not implemented in this bootstrap build"))
}

#[cfg(unix)]
fn handle_client(mut stream: UnixStream, state: Arc<DaemonState>) {
    loop {
        let request = match codec::read_message::<_, Request>(&mut stream) {
            Ok(request) => request,
            Err(codec::CodecError::Io(err))
                if matches!(
                    err.kind(),
                    io::ErrorKind::UnexpectedEof
                        | io::ErrorKind::ConnectionReset
                        | io::ErrorKind::BrokenPipe
                ) =>
            {
                log::debug!("client disconnected: {err}");
                break;
            }
            Err(err) => {
                log::debug!("dropping client after IPC decode error: {err}");
                break;
            }
        };
        let _guard = state.enter_handler();
        let response = match state.handle(request) {
            Ok(response) => response,
            Err(err) => Response::Error { message: err.to_string() },
        };
        if let Err(err) = codec::write_message(&mut stream, &response) {
            log::debug!("client disconnected while writing response: {err}");
            break;
        }
        if state.is_shutdown() {
            break;
        }
    }
}

/// Default socket path.
pub fn default_socket_path() -> PathBuf {
    #[cfg(unix)]
    {
        std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("sayrat.sock")
    }
    #[cfg(not(unix))]
    {
        PathBuf::from(r"\\.\pipe\sayrat")
    }
}
