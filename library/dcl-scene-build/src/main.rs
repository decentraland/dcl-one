//! `scene-build <root> [--production] [--ignore-composite]`: build the scene
//! at `root` in place and print a JSON report on stdout. Exit status 1 with
//! `{"ok":false,"error":...}` when the build fails, 2 on bad arguments. This is
//! the wasm32-wasip1 command the sites worker runs (see the crate docs).

use dcl_scene_build::{Options, build, install_sdk};
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut root = None;
    let mut opts = Options {
        production: false,
        ignore_composite: false,
    };
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--production" => opts.production = true,
            "--ignore-composite" => opts.ignore_composite = true,
            _ if root.is_none() && !arg.starts_with("--") => root = Some(PathBuf::from(arg)),
            _ => return usage(&format!("unexpected argument {arg}")),
        }
    }
    let Some(root) = root else {
        return usage("missing <root>");
    };
    let result = install_sdk(&root).and_then(|_| build(&root, &opts));
    match result {
        Ok(report) => {
            println!("{}", report.to_json());
            ExitCode::SUCCESS
        }
        Err(e) => {
            println!(
                "{}",
                serde_json::json!({ "ok": false, "error": format!("{e:#}") })
            );
            ExitCode::FAILURE
        }
    }
}

fn usage(why: &str) -> ExitCode {
    eprintln!("{why}\nusage: scene-build <root> [--production] [--ignore-composite]");
    ExitCode::from(2)
}
