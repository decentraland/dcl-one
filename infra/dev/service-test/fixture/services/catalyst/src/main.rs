// Stands in for the catalyst binary: the schema subcommands dev.py runs and an
// /about endpoint whose body changes when the test edits VERSION.
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;

const VERSION: &str = "one";

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("schema") => schema(),
        Some("serve") => serve(format!(
            "{}:{}",
            std::env::var("HTTP_SERVER_HOST").unwrap(),
            std::env::var("HTTP_SERVER_PORT").unwrap()
        )),
        Some("pulse") => serve(std::env::var("PULSE_METRICS_BIND").unwrap()),
        other => panic!("unexpected command {other:?}"),
    }
}

fn schema() {
    assert!(Command::new("psql")
        .arg("--dbname")
        .arg(std::env::var("DATABASE_URL").unwrap())
        .args([
            "-X",
            "-v",
            "ON_ERROR_STOP=1",
            "-c",
            "CREATE SCHEMA IF NOT EXISTS explore_api; CREATE TABLE IF NOT EXISTS explore_api.feature_flags_local (app text NOT NULL, name text NOT NULL, value boolean NOT NULL, PRIMARY KEY (app, name))",
        ])
        .status()
        .unwrap()
        .success());
}

fn serve(address: String) {
    for stream in TcpListener::bind(address).unwrap().incoming() {
        let mut stream = stream.unwrap();
        let _ = stream.read(&mut [0; 4096]);
        let body = format!("{{\"version\":\"{VERSION}\"}}");
        let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
    }
}
