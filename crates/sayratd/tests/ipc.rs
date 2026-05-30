// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg(unix)]

use std::fs;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use sayrat_protocol::codec::{read_message, write_message};
use sayrat_protocol::messages::{Request, Response};
use sayratd::daemon;
use sayratd::indexer::AppIndex;

#[test]
fn daemon_answers_hello_and_ping() {
    let base = unique_temp_dir("ipc");
    fs::create_dir_all(&base).unwrap_or_else(|err| panic!("create base: {err}"));
    let socket = base.join("sayrat.sock");
    let index = AppIndex::new(base.join("index.redb"), vec![base.join("apps")])
        .unwrap_or_else(|err| panic!("index: {err}"));

    let server_socket = socket.clone();
    let handle = thread::spawn(move || daemon::run(&server_socket, index));
    wait_for_socket(&socket);

    let mut stream = UnixStream::connect(&socket).unwrap_or_else(|err| panic!("connect: {err}"));
    write_message(&mut stream, &Request::Hello { client_version: String::from("test") })
        .unwrap_or_else(|err| panic!("write hello: {err}"));
    let hello: Response<'_> =
        read_message(&mut stream).unwrap_or_else(|err| panic!("read hello: {err}"));
    match hello {
        Response::Hello { protocol_version, .. } => assert_eq!(protocol_version, 1),
        other => panic!("unexpected hello response: {other:?}"),
    }

    write_message(&mut stream, &Request::Ping).unwrap_or_else(|err| panic!("write ping: {err}"));
    let pong: Response<'_> =
        read_message(&mut stream).unwrap_or_else(|err| panic!("read pong: {err}"));
    assert!(matches!(pong, Response::Pong));

    write_message(&mut stream, &Request::Shutdown)
        .unwrap_or_else(|err| panic!("write shutdown: {err}"));
    let ack: Response<'_> =
        read_message(&mut stream).unwrap_or_else(|err| panic!("read ack: {err}"));
    assert!(matches!(ack, Response::Ack));
    handle
        .join()
        .unwrap_or_else(|_| panic!("server panicked"))
        .unwrap_or_else(|err| panic!("server error: {err}"));
    fs::remove_dir_all(base).unwrap_or_else(|err| panic!("cleanup: {err}"));
}

fn wait_for_socket(socket: &Path) {
    let start = Instant::now();
    while !socket.exists() {
        assert!(start.elapsed() < Duration::from_secs(5), "socket was not created");
        thread::sleep(Duration::from_millis(20));
    }
}

fn unique_temp_dir(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("sayrat-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    path
}
