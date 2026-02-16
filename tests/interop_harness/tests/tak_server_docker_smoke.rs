use std::env;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use rustak_server::{ConnectionContract, ServerClientConfig, StreamingClient};

#[test]
fn tak_server_streaming_endpoint_smoke_contract() {
    if env::var("RUSTAK_RUN_TAK_SERVER_SMOKE")
        .unwrap_or_default()
        .trim()
        != "1"
    {
        eprintln!(
            "skipping TAK Server docker smoke test (set RUSTAK_RUN_TAK_SERVER_SMOKE=1 to enable)"
        );
        return;
    }

    let host = env::var("RUSTAK_TAK_SERVER_STREAM_HOST").unwrap_or_else(|_| "127.0.0.1".to_owned());
    let port = env::var("RUSTAK_TAK_SERVER_STREAM_PORT").unwrap_or_else(|_| "8089".to_owned());
    let channel_path = env::var("RUSTAK_TAK_SERVER_STREAM_PATH")
        .unwrap_or_else(|_| "/Marti/api/channels/streaming".to_owned());

    let socket = format!("{host}:{port}");
    let addr = socket
        .to_socket_addrs()
        .expect("socket should resolve")
        .next()
        .expect("resolved socket list should not be empty");

    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(3))
        .expect("TAK Server stream endpoint should be reachable");
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .expect("read timeout should apply");
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .expect("write timeout should apply");

    write!(
        stream,
        "GET {channel_path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n"
    )
    .expect("request should write");
    stream.flush().expect("request should flush");

    let mut reader = BufReader::new(stream);
    let mut status_line = String::new();
    reader
        .read_line(&mut status_line)
        .expect("status line should be readable");

    assert!(
        status_line.starts_with("HTTP/1."),
        "unexpected response preface from TAK Server stream endpoint: {status_line:?}"
    );

    let status_code = status_line
        .split_whitespace()
        .nth(1)
        .expect("status line should contain status code")
        .parse::<u16>()
        .expect("status code should parse");
    assert!(
        [200_u16, 401, 403, 404].contains(&status_code),
        "unexpected streaming endpoint status code: {status_code}"
    );

    let client = StreamingClient::new(ServerClientConfig {
        endpoint: format!("http://{host}:{port}"),
        channel_path: channel_path.clone(),
        required_capabilities: vec!["cot-stream".to_owned()],
        ..ServerClientConfig::default()
    })
    .expect("server client config should validate");

    let contract = ConnectionContract {
        server_reachable: true,
        supports_tls: false,
        advertised_channels: vec![channel_path],
        advertised_capabilities: vec!["cot-stream".to_owned()],
    };

    let session = client
        .connect_contract(&contract)
        .expect("streaming contract boundary should accept configured channel");
    assert_eq!(
        session.negotiated_capabilities,
        vec!["cot-stream".to_owned()]
    );
}
