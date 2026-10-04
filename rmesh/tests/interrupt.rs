#![cfg(unix)]

use std::io::Read;
use std::net::TcpListener;
use std::process::{Command, Stdio};
use std::time::Duration;

/// ToRadio { disconnect: true } in the stream framing: START1, START2, length, payload.
const DISCONNECT_FRAME: [u8; 6] = [0x94, 0xc3, 0x00, 0x02, 0x20, 0x01];

/// Ctrl+C while waiting on the radio must still send the disconnect (the firmware keeps
/// Bluetooth off until it hears one) and must not turn into a successful exit.
#[test]
fn ctrl_c_releases_the_radio_and_exits_130() {
    let radio = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = radio.local_addr().expect("addr").port();

    let mut rmesh = Command::new(env!("CARGO_BIN_EXE_rmesh"))
        .args(["--port", &format!("127.0.0.1:{port}"), "info", "radio"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn rmesh");

    let (mut stream, _) = radio.accept().expect("accept");
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .expect("read timeout");

    // The radio never answers want_config, so rmesh sits waiting for the dump.
    let mut first = [0u8; 64];
    let n = stream.read(&mut first).expect("want_config");
    assert!(n > 0, "rmesh sent nothing");

    let pid = rmesh.id().to_string();
    let kill = Command::new("kill").args(["-INT", &pid]).status();
    assert!(kill.expect("kill").success());

    let mut written = first[..n].to_vec();
    stream
        .read_to_end(&mut written)
        .expect("read until rmesh closes");
    let status = rmesh.wait().expect("wait");

    assert!(
        written.ends_with(&DISCONNECT_FRAME),
        "the last frame written must be the disconnect, got {written:02x?}"
    );
    assert_eq!(status.code(), Some(130), "interrupted run must exit 130");
}
