// SPDX-License-Identifier: GPL-3.0-or-later

//! `sayratd` — SayRat background daemon entry point.

use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use anyhow::{Context, Result};
use sayratd::daemon;
use sayratd::indexer::AppIndex;

const HELP: &str = "\
sayratd — SayRat background daemon

USAGE:
    sayratd [--socket <PATH>] [--data-home <PATH>]
    sayratd --measure-rss
    sayratd --version
    sayratd --help

OPTIONS:
    --socket <PATH>      Local socket path the daemon will bind to.
    --data-home <PATH>   Override XDG data home for index.redb.
    --measure-rss        Warm up a 100-entry index, print idle RSS bytes, exit.
    -V, --version        Print version information and exit.
    -h, --help           Print this help text and exit.
";

#[derive(Debug)]
struct Args {
    socket: Option<PathBuf>,
    data_home: Option<PathBuf>,
    measure_rss: bool,
}

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

    let args = Args {
        socket: pargs.opt_value_from_str("--socket").context("failed to parse --socket")?,
        data_home: pargs
            .opt_value_from_str("--data-home")
            .context("failed to parse --data-home")?,
        measure_rss: pargs.contains("--measure-rss"),
    };

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
            eprintln!("sayratd: {e:#}");
            return ExitCode::from(2);
        }
    };

    let result = if args.measure_rss { measure_rss(args.data_home) } else { run_daemon(args) };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("sayratd: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn run_daemon(args: Args) -> Result<()> {
    log::info!("sayratd starting");
    let socket_path = args.socket.unwrap_or_else(daemon::default_socket_path);
    let index = AppIndex::from_environment(args.data_home).context("failed to open app index")?;
    daemon::run(&socket_path, index).context("daemon failed")
}

fn measure_rss(data_home: Option<PathBuf>) -> Result<()> {
    let base = data_home.unwrap_or_else(|| std::env::temp_dir().join("sayrat-measure-rss"));
    let apps = base.join("applications");
    fs::create_dir_all(&apps).context("failed to create synthetic apps dir")?;
    for i in 0..100_u16 {
        let path = apps.join(format!("app-{i}.desktop"));
        fs::write(path, format!("[Desktop Entry]\nType=Application\nName=App {i}\nExec=app-{i}\n"))
            .context("failed to write synthetic desktop entry")?;
    }
    let index = AppIndex::new(base.join("sayrat").join("index.redb"), vec![apps])?;
    index.apply(sayratd::indexer::IndexOperation::FullRescan)?;
    std::thread::sleep(Duration::from_secs(5));
    let rss = current_rss_bytes().context("failed to measure rss")?;
    println!("{rss}");
    Ok(())
}

#[cfg(target_os = "linux")]
fn current_rss_bytes() -> io::Result<u64> {
    let mut statm = String::new();
    fs::File::open("/proc/self/statm")?.read_to_string(&mut statm)?;
    let pages = statm
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "bad statm"))?;
    Ok(pages * 4096)
}

#[cfg(not(target_os = "linux"))]
fn current_rss_bytes() -> io::Result<u64> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "--measure-rss is Linux-only"))
}
