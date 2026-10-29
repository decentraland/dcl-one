//! Closing a Chrome: the one that runs on the profile when a run starts, and the one a run
//! opened. A node process stands in for Chrome (the commands are the same: it has the profile as a
//! `--user-data-dir` argument); skipped, with the reason printed, where node is not installed.

mod support;

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use benchmark::{chrome, profile};
use support::scratch;

fn node_available() -> bool {
    let found = Command::new("node")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success());
    if !found {
        println!("skipped: node is not installed; it stands in for a Chrome process here");
    }
    found
}

/// A process that does nothing for a minute, with `--user-data-dir=<profile>` among its
/// arguments, as a Chrome has.
fn stand_in(profile: &Path) -> Child {
    Command::new("node")
        .args(["-e", "setTimeout(() => {}, 60000)", "--"])
        .arg(format!("--user-data-dir={}", profile.display()))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("node starts")
}

fn exits_within(child: &mut Child, limit: Duration) -> bool {
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        if child.try_wait().unwrap().is_some() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

#[test]
fn a_chrome_left_on_the_profile_is_closed_and_other_processes_are_not_touched() {
    if !node_available() {
        return;
    }
    let dir = scratch("strays");
    let profile = dir.join("chrome-profile");
    let elsewhere = dir.join("another-profile");
    std::fs::create_dir_all(&profile).unwrap();

    let mut stray = stand_in(&profile);
    let mut other = stand_in(&elsewhere);

    // The listing may need a moment to show a process that has just started.
    let listed = (0..50).any(|_| {
        let found = profile::running(&profile).unwrap().contains(&stray.id());
        if !found {
            std::thread::sleep(Duration::from_millis(200));
        }
        found
    });
    assert!(
        listed,
        "the process list shows a process started on the profile"
    );
    assert!(
        !profile::running(&profile).unwrap().contains(&other.id()),
        "and not one on another profile"
    );

    let closed = profile::close_strays(&profile);
    let stray_gone = exits_within(&mut stray, Duration::from_secs(15));
    let other_alive = other.try_wait().unwrap().is_none();

    let _ = stray.kill();
    let _ = other.kill();
    let _ = stray.wait();
    let _ = other.wait();

    closed.expect("the stray was closed");
    assert!(stray_gone, "the process on the profile was stopped");
    assert!(other_alive, "a process on another profile was left alone");
}

#[test]
fn with_no_stray_on_the_profile_nothing_happens() {
    let profile = scratch("no-strays").join("chrome-profile");
    profile::close_strays(&profile).unwrap();
}

#[tokio::test]
async fn chrome_is_closed_after_a_run_whether_or_not_it_has_a_window_to_close() {
    if !node_available() {
        return;
    }
    let profile = scratch("close").join("chrome-profile");
    let mut child = tokio::process::Command::new("node")
        .args(["-e", "setTimeout(() => {}, 60000)", "--"])
        .arg(format!("--user-data-dir={}", profile.display()))
        .stdin(Stdio::null())
        .spawn()
        .expect("node starts");

    let started = Instant::now();
    chrome::close(&mut child, chrome::Patience::RUN).await;
    assert!(
        child.try_wait().unwrap().is_some(),
        "the process was gone when close returned"
    );
    assert!(
        started.elapsed() < Duration::from_secs(30),
        "{:?}",
        started.elapsed()
    );

    // Closing what has already exited is a no-op.
    chrome::close(&mut child, chrome::Patience::RUN).await;
}

#[tokio::test]
async fn after_an_interrupt_a_chrome_that_ignores_the_request_is_killed_within_seconds() {
    if !node_available() {
        return;
    }
    let profile = scratch("close-interrupted").join("chrome-profile");
    // Ignores the polite request (SIGTERM, where there is one) and does nothing for a minute.
    let mut child = tokio::process::Command::new("node")
        .args([
            "-e",
            "process.on('SIGTERM', () => {}); setTimeout(() => {}, 60000)",
            "--",
        ])
        .arg(format!("--user-data-dir={}", profile.display()))
        .stdin(Stdio::null())
        .spawn()
        .expect("node starts");

    let started = Instant::now();
    chrome::close(&mut child, chrome::Patience::INTERRUPTED).await;
    assert!(
        child.try_wait().unwrap().is_some(),
        "the process was gone when close returned"
    );
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "{:?}",
        started.elapsed()
    );
}
