use std::process::Command;

mod common {
    pub mod testgate;
}

/// The Scene page's editor script keeps a draft and takes keyboard input
/// (scripts/scene-editor.test.mjs).
#[test]
fn scene_edits_preserve_intent_and_support_the_keyboard() {
    if dcl_scene_cli::build::find_node().is_none() {
        let _: Option<()> = common::testgate::unavailable(
            "node",
            "install node; scripts/scene-editor.test.mjs runs the Scene page's editor under it",
        );
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
