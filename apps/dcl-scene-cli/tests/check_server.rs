//! The preview's long-lived type checker against the compiler it stands in
//! for: after every kind of edit, its report and exit status are those of a
//! freshly spawned `tsc` given the same arguments.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use dcl_scene_cli::check_server::Checker;

const BIN: &str = env!("CARGO_BIN_EXE_dcl-scene-cli");

struct Scene(PathBuf);

impl Scene {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "dcl-scene-cli-check-server-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let out = Command::new(BIN)
            .args(["init", "--yes", "--dir"])
            .arg(&dir)
            .stdin(Stdio::null())
            .env_remove("RUST_LOG")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "init: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        // The checker is given the real path, as a spawned tsc's cwd is.
        Scene(dir.canonicalize().unwrap())
    }

    fn write(&self, rel: &str, text: &str) {
        std::fs::write(self.0.join(rel), text).unwrap();
    }

    fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.0.join(rel)).unwrap()
    }
}

impl Drop for Scene {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn args(root: &Path, buildinfo: &str, extra: &[&str]) -> Vec<String> {
    let mut args: Vec<String> = [
        "-p",
        "tsconfig.json",
        "--noEmit",
        "--incremental",
        "--tsBuildInfoFile",
    ]
    .map(str::to_string)
    .to_vec();
    args.push(root.join(buildinfo).to_string_lossy().into_owned());
    args.extend(["--pretty", "false"].map(str::to_string));
    args.extend(extra.iter().map(|s| s.to_string()));
    args
}

#[tokio::test(flavor = "multi_thread")]
async fn the_checker_reports_what_a_spawned_tsc_reports_after_every_edit() {
    let node = dcl_scene_cli::build::find_node().expect("node on PATH");
    let scene = Scene::new("parity");
    let root = &scene.0;
    let tsc = root.join("node_modules/typescript/lib/tsc.js");
    let source = scene.read("src/index.ts");
    let tsconfig = scene.read("tsconfig.json");
    let checker = Checker::default();
    let mut extra: Vec<&str> = Vec::new();

    let compare = async |what: &str, extra: &[&str]| -> (i32, String) {
        let warm = checker
            .check(&node, &tsc, root, args(root, ".dcl-cache/warm", extra))
            .await
            .unwrap_or_else(|| panic!("{what}: no checker for the vendored TypeScript"));
        let cold = Command::new(&node)
            .arg(&tsc)
            .args(args(root, ".dcl-cache/cold", extra))
            .current_dir(root)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let cold_out = String::from_utf8_lossy(&cold.stdout).into_owned();
        assert_eq!(warm.out, cold_out, "{what}: report");
        assert_eq!(Some(warm.code), cold.status.code(), "{what}: exit status");
        assert!(cold.stderr.is_empty(), "{what}: tsc wrote to stderr");
        (warm.code, warm.out)
    };

    assert_eq!(compare("the scaffold", &extra).await, (0, String::new()));
    assert_eq!(compare("nothing changed", &extra).await.0, 0);

    scene.write(
        "src/index.ts",
        &format!("{source}\nexport const n: number = 'a'\n"),
    );
    let (code, out) = compare("a type error", &extra).await;
    assert!(code != 0 && out.contains("error TS2322"), "{out}");

    // Same size, and possibly the same clock tick, as the save before.
    scene.write(
        "src/index.ts",
        &format!("{source}\nexport const n: number = 123\n"),
    );
    assert_eq!(compare("fixed in place", &extra).await, (0, String::new()));

    // More report than a pipe holds at once.
    let many: String = (0..2500)
        .map(|i| format!("export const wrong{i}: number = 'a'\n"))
        .collect();
    scene.write("src/index.ts", &format!("{source}\n{many}"));
    let (_, out) = compare("thousands of errors", &extra).await;
    assert!(out.len() > 150_000 && out.matches("error TS2322").count() == 2500);

    scene.write("src/index.ts", &format!("{source}\nconst = (\n"));
    assert!(compare("a syntax error", &extra).await.0 != 0);

    scene.write(
        "src/index.ts",
        &format!("{source}\nimport {{ other }} from './other'\nexport const o: string = other\n"),
    );
    let (_, out) = compare("an import of a missing file", &extra).await;
    assert!(out.contains("error TS2307"), "{out}");

    scene.write("src/other.ts", "export const other = 1\n");
    let (_, out) = compare("the file added, of another type", &extra).await;
    assert!(
        out.contains("error TS2322") && !out.contains("TS2307"),
        "{out}"
    );

    scene.write("src/other.ts", "export const other = 'one'\n");
    assert_eq!(
        compare("the added file fixed", &extra).await,
        (0, String::new())
    );

    std::fs::remove_file(root.join("src/other.ts")).unwrap();
    assert!(compare("the file removed again", &extra).await.0 != 0);

    scene.write(
        "src/index.ts",
        &format!("{source}\nexport function f(a) {{ return a }}\n"),
    );
    let (_, out) = compare("an implicit any", &extra).await;
    assert!(out.contains("error TS7006"), "{out}");

    // tsconfig edits: a looser one, a deprecated option (reported, then
    // silenced by the flag a build adds), a broken one, the original.
    let loose = tsconfig.replacen(
        "\"compilerOptions\": {",
        "\"compilerOptions\": {\n    \"noImplicitAny\": false,",
        1,
    );
    assert_ne!(loose, tsconfig);
    scene.write("tsconfig.json", &loose);
    assert_eq!(
        compare("noImplicitAny off", &extra).await,
        (0, String::new())
    );

    let deprecated = tsconfig.replacen(
        "\"compilerOptions\": {",
        "\"compilerOptions\": {\n    \"baseUrl\": \".\",",
        1,
    );
    scene.write("tsconfig.json", &deprecated);
    let (_, out) = compare("baseUrl", &extra).await;
    assert!(out.contains("error TS5101"), "{out}");
    extra.extend(["--ignoreDeprecations", "6.0"]);
    let (_, out) = compare("baseUrl, deprecations ignored", &extra).await;
    assert!(
        !out.contains("TS5101") && out.contains("error TS7006"),
        "{out}"
    );
    scene.write("src/index.ts", &source);
    assert_eq!(
        compare("and its error fixed", &extra).await,
        (0, String::new())
    );
    extra.clear();

    scene.write("tsconfig.json", &tsconfig.replacen('{', "{ oops", 1));
    assert!(compare("a tsconfig that does not parse", &extra).await.0 != 0);

    scene.write("tsconfig.json", &tsconfig);
    assert_eq!(
        compare("the tsconfig restored", &extra).await,
        (0, String::new())
    );
}
