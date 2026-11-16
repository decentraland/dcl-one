//! `world access edit` on a hosted runner: `CI` in the environment keeps the
//! browser closed, as it does for every other command that signs on a page.

use std::process::{Command, Stdio};

mod common {
    pub mod child;
    pub mod wait;
}
use common::child::{Tail, terminate, wait_bounded};
use common::wait::HANG_GUARD;

const BIN: &str = env!("CARGO_BIN_EXE_dcl-scene-cli");

#[test]
fn ci_in_the_environment_keeps_the_browser_closed() {
    let mut cmd = Command::new(BIN);
    cmd.args([
        "world",
        "access",
        "edit",
        "somebody.dcl.eth",
        "--target-content",
        "http://127.0.0.1:9",
    ]);
    for k in ["DCL_PRIVATE_KEY", "RUST_LOG", "DCL_SCENE_CLI_TARGET_SERVER"] {
        cmd.env_remove(k);
    }
    let mut child = cmd
        .env("CI", "true")
        .env("NO_COLOR", "1")
        .env("DCL_SCENE_CLI_LINKER_TIMEOUT_SECS", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let out = Tail::spawn(child.stdout.take().unwrap());
    let err = Tail::spawn(child.stderr.take().unwrap());
    let status = wait_bounded(&mut child, HANG_GUARD).unwrap_or_else(|| {
        terminate(&mut child);
        panic!(
            "the signing page outlived its timeout\nstdout: {}\nstderr: {}",
            out.text(),
            err.text()
        );
    });
    let text = format!("{}{}", out.text(), err.text());
    assert!(!status.success(), "nobody signed: {text}");
    assert!(text.contains("Sign this change"), "{text}");
    assert!(
        text.contains("browser auto-open disabled"),
        "CI=true alone must keep the browser closed:\n{text}"
    );
}
