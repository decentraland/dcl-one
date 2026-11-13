//! The type check under every TypeScript line a scene may install: 5.x,
//! 6.x and 7.x (the Go port). `DCL_SCENE_CLI_TEST_TYPESCRIPT` names, colon
//! separated, directories whose `node_modules/typescript` is each one.
#![cfg(unix)]

mod common {
    pub mod env;
    pub mod testgate;
}

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_dcl-scene-cli");
const VAR: &str = "DCL_SCENE_CLI_TEST_TYPESCRIPT";

/// The upstream scene template's shape: `baseUrl` and a bare `paths` target.
const BASE_URL_TSCONFIG: &str = r#"{
  "compilerOptions": {
    "allowJs": true,
    "strict": true,
    "baseUrl": ".",
    "paths": {
      "@dcl/asset-packs/*": ["node_modules/@dcl/inspector/node_modules/@dcl/asset-packs/*"]
    }
  },
  "include": ["src/**/*.ts", "src/**/*.tsx"],
  "extends": "@dcl/sdk/types/tsconfig.ecs7.json"
}
"#;

/// An older scene's tsconfig: `"moduleResolution": "node"`, which 6.x
/// deprecates as a value (TS5107, reported as `node10`) and 7.x has removed
/// (TS5108).
const NODE_RESOLUTION_TSCONFIG: &str = r#"{
  "compilerOptions": {
    "allowJs": true,
    "strict": true,
    "moduleResolution": "node"
  },
  "include": ["src/**/*.ts", "src/**/*.tsx"],
  "extends": "@dcl/sdk/types/tsconfig.ecs7.json"
}
"#;

struct Scaffold(PathBuf);

impl Scaffold {
    fn new(tag: &str, project: &[&str]) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "dcl-scene-cli-typescript-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let scaffold = Scaffold(dir);
        let mut args = vec!["init", "--dir", scaffold.dir(), "--yes"];
        args.extend_from_slice(project);
        let out = run(&args);
        assert!(out.status.success(), "init: {}", stderr_of(&out));
        scaffold
    }

    fn dir(&self) -> &str {
        self.0.to_str().unwrap()
    }

    /// Swap the vendored `typescript` for `install`'s; its version.
    fn use_typescript(&self, install: &Path) -> String {
        let link = self.0.join("node_modules/typescript");
        let _ = std::fs::remove_dir_all(&link);
        let _ = std::fs::remove_file(&link);
        let target = install
            .join("node_modules/typescript")
            .canonicalize()
            .unwrap_or_else(|e| panic!("{}: no node_modules/typescript: {e}", install.display()));
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let manifest = std::fs::read_to_string(target.join("package.json")).unwrap();
        serde_json::from_str::<serde_json::Value>(&manifest).unwrap()["version"]
            .as_str()
            .unwrap()
            .to_string()
    }
}

impl Drop for Scaffold {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run(args: &[&str]) -> Output {
    let mut cmd = Command::new(BIN);
    cmd.args(args).stdin(Stdio::null());
    for k in ["RUST_LOG", "NO_COLOR"] {
        cmd.env_remove(k);
    }
    cmd.output().unwrap()
}

fn stderr_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
#[ignore = "needs DCL_SCENE_CLI_TEST_TYPESCRIPT: installed TypeScript 5, 6 and 7 lines to check the scaffolds with; see docs/testing.md"]
fn every_typescript_line_checks_the_scaffolds_and_names_what_it_dropped() {
    let Some(installs) = common::env::require_env(VAR) else {
        return;
    };
    let mut majors = Vec::new();
    for install in installs.split(':').filter(|s| !s.is_empty()) {
        let install = Path::new(install);
        let scene = Scaffold::new("scene", &[]);
        let version = scene.use_typescript(install);
        let major = dcl_scene_cli::typescript::major_of(&version);
        majors.push(major);

        let out = run(&["build", "--dir", scene.dir(), "--built-in"]);
        assert!(
            out.status.success(),
            "{version}: the scene scaffold does not check: {}",
            stderr_of(&out)
        );
        assert!(
            !stderr_of(&out).contains("ignoreDeprecations"),
            "{version}: the scaffold needed a deprecation pass: {}",
            stderr_of(&out)
        );

        let library = Scaffold::new("library", &["--project", "library"]);
        library.use_typescript(install);
        let out = run(&[
            "build",
            "--dir",
            library.dir(),
            "--built-in",
            "--single",
            "src/index.ts",
            "--emitDeclaration",
        ]);
        assert!(
            out.status.success(),
            "{version}: the library scaffold does not emit declarations: {}",
            stderr_of(&out)
        );
        assert!(
            library.0.join("dist/index.d.ts").is_file(),
            "{version}: no dist/index.d.ts"
        );

        std::fs::write(scene.0.join("tsconfig.json"), BASE_URL_TSCONFIG).unwrap();
        let out = run(&["build", "--dir", scene.dir(), "--built-in"]);
        let err = stderr_of(&out);
        match major {
            7 => {
                assert!(!out.status.success(), "{version}: baseUrl must not check");
                assert!(err.contains("TS5102"), "{version}: {err}");
                assert!(
                    err.contains("remove baseUrl from tsconfig.json"),
                    "{version}: {err}"
                );
            }
            6 => {
                assert!(
                    out.status.success(),
                    "{version}: baseUrl must check once more: {err}"
                );
                assert!(
                    err.contains("--ignoreDeprecations 6.0") && err.contains("baseUrl"),
                    "{version}: {err}"
                );
            }
            _ => {
                assert!(
                    out.status.success(),
                    "{version}: baseUrl is plain 5.x: {err}"
                );
                assert!(!err.contains("ignoreDeprecations"), "{version}: {err}");
            }
        }

        std::fs::write(scene.0.join("tsconfig.json"), NODE_RESOLUTION_TSCONFIG).unwrap();
        let out = run(&["build", "--dir", scene.dir(), "--built-in"]);
        let err = stderr_of(&out);
        match major {
            7 => {
                assert!(
                    !out.status.success(),
                    "{version}: moduleResolution node must not check"
                );
                assert!(err.contains("TS5108"), "{version}: {err}");
                assert!(
                    err.contains("set moduleResolution to bundler"),
                    "{version}: {err}"
                );
            }
            6 => {
                assert!(
                    out.status.success(),
                    "{version}: moduleResolution node must check once more: {err}"
                );
                assert!(
                    err.contains("--ignoreDeprecations 6.0")
                        && err.contains("moduleResolution=node10"),
                    "{version}: {err}"
                );
            }
            _ => {
                assert!(
                    out.status.success(),
                    "{version}: moduleResolution node is plain 5.x: {err}"
                );
                assert!(!err.contains("ignoreDeprecations"), "{version}: {err}");
            }
        }
    }
    for major in [5, 6, 7] {
        assert!(
            majors.contains(&major),
            "{VAR} covers {majors:?}; a {major}.x install is missing"
        );
    }
}
