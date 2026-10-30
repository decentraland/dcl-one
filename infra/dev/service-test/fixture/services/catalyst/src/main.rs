// Stands in for the catalyst binary: the schema subcommands dev.py runs and an
// /about endpoint whose body changes when the test edits VERSION.
use std::io::{Read, Write};
use std::net::TcpListener;

const VERSION: &str = "one";

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("schema") => {}
        Some("serve") => serve(),
        other => panic!("unexpected command {other:?}"),
    }
}

fn serve() {
    let host = std::env::var("HTTP_SERVER_HOST").unwrap();
    let port = std::env::var("HTTP_SERVER_PORT").unwrap();
    for stream in TcpListener::bind(format!("{host}:{port}")).unwrap().incoming() {
        let mut stream = stream.unwrap();
        let _ = stream.read(&mut [0; 4096]);
        let body = format!("{{\"version\":\"{VERSION}\"}}");
        let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
    }
}
