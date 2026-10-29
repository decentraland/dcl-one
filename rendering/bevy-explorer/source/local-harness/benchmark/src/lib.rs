//! The browser leg of the performance benchmark: serves the web explorer from localhost, runs it
//! in Chrome and stores the stream the page sends; then writes the report of the results
//! ([`report`]). The page's side of the stream is `deploy/web/engine/perf_stream.js`; the protocol
//! is in `docs/guidelines/benchmark.md`.

pub mod access;
pub mod app_source;
pub mod byte_stream;
pub mod chrome;
pub mod cli;
pub mod commands;
pub mod failure;
pub mod limits;
pub mod out;
pub mod platform;
pub mod profile;
pub mod report;
pub mod runner;
pub mod server;

/// An empty folder for a unit test: `<temp>/benchmark-<process>/<group>/<name>`.
#[cfg(test)]
pub(crate) fn scratch(group: &str, name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir()
        .join(format!("benchmark-{}", std::process::id()))
        .join(group)
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
