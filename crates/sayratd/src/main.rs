// SPDX-License-Identifier: GPL-3.0-or-later

//! `sayratd` — SayRat background daemon entry point.
//!
//! Phase 1 only parses the CLI surface that future phases will rely on
//! (`--socket <path>` and `--version`), initialises tracing, and exits
//! cleanly. No socket binding, indexing, or Wasm host setup yet.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};

const HELP: &str = "\
sayratd — SayRat background daemon

USAGE:
    sayratd [--socket <PATH>]
    sayratd --version
    sayratd --help

OPTIONS:
    --socket <PATH>   Unix domain socket path the daemon will bind to.
    -V, --version     Print version information and exit.
    -h, --help        Print this help text and exit.
";

#[derive(Debug)]
struct Args {
    socket: Option<PathBuf>,
}

/// Parse argv with `pico-args`. Chosen over `clap` because the binary
/// only needs two flags; pulling `clap` in (even with default-features
/// disabled) would add measurable code size for no gain (see workspace
/// `Cargo.toml` for the full rationale).
fn parse_args() -> Result<Option<Args>> {
    let mut pargs = pico_args::Arguments::from_env();

    if pargs.contains(["-h", "--help"]) {
        print!("{HELP}");
        return Ok(None);
    }
    if pargs.contains(["-V", "--version"]) {
        println!("sayratd {}", env!("CARGO_PKG_VERSION"));
        return Ok(None);
    }

    let args =
        Args { socket: pargs.opt_value_from_str("--socket").context("failed to parse --socket")? };

    let remaining = pargs.finish();
    if !remaining.is_empty() {
        anyhow::bail!("unexpected arguments: {remaining:?}");
    }

    Ok(Some(args))
}

fn init_tracing() {
    let filter = match tracing_subscriber::EnvFilter::try_from_default_env() {
        Ok(filter) => filter,
        Err(_) => tracing_subscriber::EnvFilter::new("info"),
    };

    tracing_subscriber::fmt().with_env_filter(filter).init();
}

fn main() -> ExitCode {
    init_tracing();

    let args = match parse_args() {
        Ok(Some(a)) => a,
        Ok(None) => return ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("sayratd: {e:#}");
            return ExitCode::from(2);
        }
    };

    tracing::info!("sayratd starting");
    match &args.socket {
        Some(path) => tracing::debug!(socket_path = %path.display()),
        None => tracing::debug!(socket_path = "<unset>"),
    }

    // Phase 1 stub: socket binding, indexer boot, and Wasm engine setup
    // all land in later phases. Exit cleanly so CI smoke tests pass.
    ExitCode::SUCCESS
}
