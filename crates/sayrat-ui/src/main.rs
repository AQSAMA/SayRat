// SPDX-License-Identifier: GPL-3.0-or-later

//! `sayrat-ui` — SayRat overlay client entry point.
//!
//! Phase 1 only parses `--socket <path>` and `--version`, initialises
//! logging, and exits cleanly. Slint window construction, layer-shell
//! attachment, and IPC connection all arrive in later phases.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};

const HELP: &str = "\
sayrat-ui — SayRat overlay client

USAGE:
    sayrat-ui [--socket <PATH>]
    sayrat-ui --version
    sayrat-ui --help

OPTIONS:
    --socket <PATH>   Unix domain socket path used to reach sayratd.
    -V, --version     Print version information and exit.
    -h, --help        Print this help text and exit.
";

#[derive(Debug)]
struct Args {
    socket: Option<PathBuf>,
}

/// Parse argv with `pico-args` for the same reasons spelled out in
/// `sayratd::main` — minimal code size, zero proc-macro footprint.
fn parse_args() -> Result<Option<Args>> {
    let mut pargs = pico_args::Arguments::from_env();

    if pargs.contains(["-h", "--help"]) {
        print!("{HELP}");
        return Ok(None);
    }
    if pargs.contains(["-V", "--version"]) {
        println!("sayrat-ui {}", env!("CARGO_PKG_VERSION"));
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

fn main() -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = match parse_args() {
        Ok(Some(a)) => a,
        Ok(None) => return ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("sayrat-ui: {e:#}");
            return ExitCode::from(2);
        }
    };

    log::info!("sayrat-ui starting");
    match &args.socket {
        Some(path) => log::debug!("socket path: {}", path.display()),
        None => log::debug!("socket path: <unset>"),
    }

    // Phase 1 stub: real implementation will mount a Slint window on
    // the wlr-layer-shell overlay and start forwarding keystrokes to
    // `sayratd` over the socket. Exit cleanly for now.
    ExitCode::SUCCESS
}
