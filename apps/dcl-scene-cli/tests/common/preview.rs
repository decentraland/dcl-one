//! Where a `start --port 0` preview listens: the port it bound, read from the
//! `Local:` line of its banner. A port the test picks and passes on can be
//! taken by any other socket on the machine between the two binds.
#![allow(dead_code)]

use std::path::Path;
use std::process::Child;
use std::time::Duration;

/// The port in a banner's `Local:` line, once one is printed.
pub fn banner_port(text: &str) -> Option<u16> {
    text.lines().find_map(|line| {
        line.trim()
            .strip_prefix("Local:")?
            .trim()
            .strip_prefix("http://127.0.0.1:")?
            .trim()
            .parse()
            .ok()
    })
}

/// Waits for `log` to name the port, failing at once if `child` exits first.
/// `guard` only bounds a hang.
pub async fn wait_for_port(log: &Path, child: &mut Child, guard: Duration) -> u16 {
    let text = || std::fs::read_to_string(log).unwrap_or_default();
    super::wait::until(guard, async || {
        let seen = text();
        if let Some(port) = banner_port(&seen) {
            return Some(port);
        }
        if let Some(status) = child.try_wait().unwrap() {
            panic!("start exited ({status}) before printing its port:\n{seen}");
        }
        None
    })
    .await
    .unwrap_or_else(|| panic!("start never printed its Local URL:\n{}", text()))
}
