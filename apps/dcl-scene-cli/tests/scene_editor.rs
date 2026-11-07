use std::process::Command;

#[test]
fn scene_edits_preserve_intent_and_support_the_keyboard() {
    if Command::new("node").arg("--version").output().is_err() {
        eprintln!("skipping: node is not on PATH");
        return;
    }
    let output = Command::new("node")
        .args(["--test", "scripts/scene-editor.test.mjs"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("run scene editor tests");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
