use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

// Only the unix-only provisioned-scene test reads the gate.
#[cfg(unix)]
mod common {
    pub mod env;
    pub mod testgate;
}

const BIN: &str = env!("CARGO_BIN_EXE_dcl-scene-cli");

struct Fixture(PathBuf);

impl Fixture {
    fn new(tag: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("dcl-scene-cli-initgate-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Fixture(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn dir_arg(&self) -> String {
        self.0.display().to_string()
    }

    fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.0.join(rel)).unwrap_or_else(|e| panic!("reading {rel}: {e}"))
    }

    fn json(&self, rel: &str) -> serde_json::Value {
        serde_json::from_str(&self.read(rel)).unwrap_or_else(|e| panic!("parsing {rel}: {e}"))
    }
}

impl Drop for Fixture {
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

fn stdout_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

const SCENE_FILES: &[&str] = &[
    "scene.json",
    "package.json",
    "tsconfig.json",
    "src/index.ts",
    ".gitignore",
    ".dclignore",
    "README.md",
    "images/scene-thumbnail.png",
];

#[test]
fn scene_scaffold_is_complete_and_loadable() {
    let f = Fixture::new("scene");
    let dir = f.dir_arg();
    let out = run(&["init", "--dir", &dir]);
    assert!(out.status.success(), "stderr: {}", stderr_of(&out));
    let stdout = stdout_of(&out);
    assert!(stdout.contains("Scaffolded a scene project"), "{stdout}");
    assert!(stdout.contains("default scene project"), "{stdout}");
    assert!(!stdout.contains("npm install"), "{stdout}");
    assert!(
        stdout.contains("Installed node_modules from the vendored SDK"),
        "{stdout}"
    );
    assert!(stdout.contains("dcl-scene-cli start"), "{stdout}");
    for rel in SCENE_FILES {
        assert!(f.path().join(rel).is_file(), "missing {rel}");
    }
    let scene = f.json("scene.json");
    assert_eq!(scene["runtimeVersion"], "7");
    assert_eq!(scene["main"], "bin/index.js");
    assert_eq!(scene["scene"]["parcels"][0], "0,0");
    let title = scene["display"]["title"].as_str().unwrap();
    assert!(title.starts_with("dcl-scene-cli-initgate-scene"), "{title}");
    let pkg = f.json("package.json");
    for (name, version) in pkg["devDependencies"].as_object().unwrap() {
        let v = version.as_str().unwrap();
        assert_ne!(v, "latest", "{name} is unpinned");
        assert!(
            v.chars().next().unwrap().is_ascii_digit(),
            "{name}={v} is not an exact pin"
        );
    }
    for script in ["start", "build", "deploy"] {
        assert!(
            pkg["scripts"][script].is_string(),
            "missing script {script}"
        );
    }
    assert!(
        f.path()
            .join("node_modules/@dcl/sdk/package.json")
            .is_file()
    );
    assert!(
        f.path()
            .join("node_modules/typescript/lib/tsc.js")
            .is_file()
    );
    assert!(f.read(".dclignore").lines().any(|l| l == "node_modules"));
    assert!(f.read(".gitignore").lines().any(|l| l == "bin/"));
    let png = std::fs::read(f.path().join("images/scene-thumbnail.png")).unwrap();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    let project = dcl_scene_cli::scene::Project::load(f.path()).expect("scaffold must load");
    assert_eq!(project.main_output().unwrap(), "bin/index.js");
    assert_eq!(project.parcels(), vec!["0,0".to_string()]);
    assert!(dcl_scene_cli::scene::min_cli_warning(f.path()).is_none());
}

#[test]
fn refuses_a_non_empty_directory_with_next_steps() {
    let f = Fixture::new("nonempty");
    std::fs::write(f.path().join("existing.txt"), "hi").unwrap();
    let dir = f.dir_arg();
    let out = run(&["init", "--dir", &dir]);
    assert!(!out.status.success());
    let err = stderr_of(&out);
    let error_lines: Vec<&str> = err.lines().filter(|l| l.starts_with("Error: ")).collect();
    assert_eq!(error_lines.len(), 1, "stderr: {err}");
    assert!(error_lines[0].contains("not empty"), "stderr: {err}");
    assert!(err.contains("\u{2192} try: "), "stderr: {err}");
    assert!(err.contains("--yes"), "stderr: {err}");
    assert!(!err.contains('\u{1b}'), "ANSI leaked: {err}");
    assert!(!f.path().join("scene.json").exists());
}

#[test]
fn yes_scaffolds_into_a_non_empty_directory() {
    let f = Fixture::new("yes");
    std::fs::write(f.path().join("existing.txt"), "hi").unwrap();
    let dir = f.dir_arg();
    let out = run(&["init", "--dir", &dir, "--yes"]);
    assert!(out.status.success(), "stderr: {}", stderr_of(&out));
    assert!(f.path().join("scene.json").is_file());
    assert_eq!(f.read("existing.txt"), "hi");
}

#[test]
fn smart_wearable_scaffold_has_wearable_skeleton() {
    let f = Fixture::new("sw");
    let dir = f.dir_arg();
    let out = run(&["init", "--dir", &dir, "--project", "smart-wearable"]);
    assert!(out.status.success(), "stderr: {}", stderr_of(&out));
    let stdout = stdout_of(&out);
    assert!(
        stdout.contains("Scaffolded a smart wearable project"),
        "{stdout}"
    );
    assert!(stdout.contains("thumbnail.png"), "{stdout}");
    let wearable = f.json("wearable.json");
    let id = wearable["id"].as_str().unwrap();
    assert_eq!(id.len(), 36);
    assert_eq!(id.as_bytes()[14], b'4');
    assert_eq!(wearable["data"]["category"], "eyewear");
    assert_eq!(wearable["rarity"], "mythic");
    let rep = &wearable["data"]["representations"][0];
    assert_eq!(rep["mainFile"], "model.glb");
    assert_eq!(rep["contents"][0], "model.glb");
    assert_eq!(rep["bodyShapes"].as_array().unwrap().len(), 2);
    let scene = f.json("scene.json");
    assert_eq!(scene["isPortableExperience"], true);
    assert_eq!(scene["runtimeVersion"], "7");
    assert_eq!(scene["scene"]["parcels"].as_array().unwrap().len(), 100);
    assert_eq!(scene["scene"]["base"], "0,0");
    let pkg = f.json("package.json");
    assert!(pkg["scripts"]["pack"].is_string());
    assert!(f.read("README.md").contains("model.glb"));
    let glb = std::fs::read(f.path().join("model.glb")).unwrap();
    assert_eq!(&glb[..4], b"glTF", "model.glb is a glTF binary");
    let png = std::fs::read(f.path().join("thumbnail.png")).unwrap();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(&png[16..24], &[0, 0, 1, 0, 0, 0, 1, 0], "256x256");
    let second = Fixture::new("sw2");
    let dir2 = second.dir_arg();
    let out2 = run(&["init", "--dir", &dir2, "--project", "smart-wearable"]);
    assert!(out2.status.success());
    assert_ne!(second.json("wearable.json")["id"], wearable["id"]);
}

#[test]
fn init_creates_a_missing_target_directory() {
    let f = Fixture::new("mkdir");
    let nested = f.path().join("brand/new/scene");
    let dir = nested.display().to_string();
    let out = run(&["init", "--dir", &dir]);
    assert!(out.status.success(), "stderr: {}", stderr_of(&out));
    assert!(nested.join("scene.json").is_file());
    let scene: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(nested.join("scene.json")).unwrap()).unwrap();
    assert_eq!(scene["display"]["title"], "scene");
}

/// `node` on PATH, or the gate's refusal: the vendored toolchain type-checks
/// through it.
#[cfg(unix)]
fn node_available() -> Option<()> {
    match dcl_scene_cli::build::find_node() {
        Some(_) => Some(()),
        None => common::testgate::unavailable("node", "no node binary on PATH"),
    }
}

#[cfg(unix)]
fn run_with_path(args: &[&str]) -> Output {
    let mut cmd = Command::new(BIN);
    cmd.args(args).stdin(Stdio::null()).env_remove("RUST_LOG");
    cmd.output().unwrap()
}

/// The scaffold builds with nothing but the vendored node_modules: SDK
/// chunk, scene chunk, loader stub and a passing type check.
#[cfg(unix)]
#[test]
fn scene_scaffold_builds_with_the_vendored_toolchain() {
    let Some(()) = node_available() else { return };
    let f = Fixture::new("vendored-build");
    let dir = f.dir_arg();
    let out = run(&["init", "--dir", &dir]);
    assert!(out.status.success(), "stderr: {}", stderr_of(&out));
    let out = run_with_path(&["build", "--dir", &dir]);
    let stdout = stdout_of(&out);
    assert!(
        out.status.success(),
        "build failed\nstdout: {stdout}\nstderr: {}",
        stderr_of(&out)
    );
    assert!(stdout.contains("SDK chunk installed"), "{stdout}");
    assert!(stdout.contains("Scene chunk saved"), "{stdout}");
    assert!(stdout.contains("Type check passed"), "{stdout}");
    for (rel, min) in [
        ("bin/sdk-runtime.js", 100_000usize),
        ("bin/index.js", 1_000),
        ("bin/scene.js", 100),
    ] {
        let bytes = std::fs::read(f.path().join(rel)).unwrap();
        assert!(
            bytes.len() > min,
            "{rel} suspiciously small: {} bytes",
            bytes.len()
        );
    }
}

/// A scene tsconfig from before the template moved to `bundler`:
/// `"moduleResolution": "node"`, which the vendored TypeScript 6 deprecates
/// as a value (TS5107, reported as `node10`). The build repeats the check
/// with `--ignoreDeprecations 6.0` and says which value needed it.
#[cfg(unix)]
#[test]
fn scene_with_a_deprecated_module_resolution_builds_with_the_vendored_toolchain() {
    let Some(()) = node_available() else { return };
    let f = Fixture::new("vendored-node10");
    let dir = f.dir_arg();
    let out = run(&["init", "--dir", &dir]);
    assert!(out.status.success(), "stderr: {}", stderr_of(&out));
    let tsconfig = f.read("tsconfig.json").replace(
        "\"moduleResolution\": \"bundler\"",
        "\"moduleResolution\": \"node\"",
    );
    assert!(
        tsconfig.contains("\"moduleResolution\": \"node\""),
        "{tsconfig}"
    );
    std::fs::write(f.path().join("tsconfig.json"), tsconfig).unwrap();
    let out = run_with_path(&["build", "--dir", &dir]);
    let (stdout, stderr) = (stdout_of(&out), stderr_of(&out));
    assert!(
        out.status.success(),
        "build failed\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(stdout.contains("Type check passed"), "{stdout}");
    assert!(
        stderr.contains("--ignoreDeprecations 6.0") && stderr.contains("moduleResolution=node10"),
        "{stderr}"
    );

    // The next check passes the flag at once, having learnt it is needed,
    // and says the same.
    let source = f.read("src/index.ts");
    std::fs::write(
        f.path().join("src/index.ts"),
        format!("{source}\n// edited\n"),
    )
    .unwrap();
    let out = run_with_path(&["build", "--dir", &dir]);
    let (stdout, stderr) = (stdout_of(&out), stderr_of(&out));
    assert!(out.status.success(), "stdout: {stdout}\nstderr: {stderr}");
    assert!(stdout.contains("Type check passed ("), "{stdout}");
    assert!(
        stderr.contains("--ignoreDeprecations 6.0") && stderr.contains("moduleResolution=node10"),
        "{stderr}"
    );
    // What it learnt still reports this scene's type errors.
    std::fs::write(
        f.path().join("src/index.ts"),
        format!("{source}\nexport const wrong: number = 'text'\n"),
    )
    .unwrap();
    let out = run_with_path(&["build", "--dir", &dir]);
    let stderr = stderr_of(&out);
    assert!(!out.status.success(), "{stderr}");
    assert!(
        stderr.contains("TS2322") && !stderr.contains("TS5107"),
        "{stderr}"
    );

    // A tsconfig that no longer needs the flag is checked without it.
    let fixed = f.read("tsconfig.json").replace(
        "\"moduleResolution\": \"node\"",
        "\"moduleResolution\": \"bundler\"",
    );
    std::fs::write(f.path().join("tsconfig.json"), fixed).unwrap();
    std::fs::write(f.path().join("src/index.ts"), source).unwrap();
    let out = run_with_path(&["build", "--dir", &dir]);
    let (stdout, stderr) = (stdout_of(&out), stderr_of(&out));
    assert!(out.status.success(), "stdout: {stdout}\nstderr: {stderr}");
    assert!(stdout.contains("Type check passed ("), "{stdout}");
    assert!(!stderr.contains("ignoreDeprecations"), "{stderr}");
}

/// The upstream template's tsconfig sets `baseUrl`, which the vendored
/// TypeScript 6 deprecates (TS5101): the build knows from the tsconfig and
/// says so, on the first check and on later ones.
#[cfg(unix)]
#[test]
fn scene_with_base_url_builds_with_the_vendored_toolchain() {
    let Some(()) = node_available() else { return };
    let f = Fixture::new("vendored-baseurl");
    let dir = f.dir_arg();
    let out = run(&["init", "--dir", &dir]);
    assert!(out.status.success(), "stderr: {}", stderr_of(&out));
    let tsconfig = f.read("tsconfig.json").replacen(
        "\"compilerOptions\": {",
        "\"compilerOptions\": {\n    \"baseUrl\": \".\",",
        1,
    );
    assert!(tsconfig.contains("\"baseUrl\""), "{tsconfig}");
    std::fs::write(f.path().join("tsconfig.json"), &tsconfig).unwrap();
    let source = f.read("src/index.ts");
    for edit in ["", "\n// edited\n"] {
        std::fs::write(f.path().join("src/index.ts"), format!("{source}{edit}")).unwrap();
        let out = run_with_path(&["build", "--dir", &dir]);
        let (stdout, stderr) = (stdout_of(&out), stderr_of(&out));
        assert!(out.status.success(), "stdout: {stdout}\nstderr: {stderr}");
        assert!(stdout.contains("Type check passed ("), "{stdout}");
        assert!(
            stderr.contains("tsconfig uses baseUrl, which TypeScript 6.")
                && stderr.contains("--ignoreDeprecations 6.0"),
            "{stderr}"
        );
    }
    std::fs::write(
        f.path().join("src/index.ts"),
        format!("{source}\nexport const wrong: number = 'text'\n"),
    )
    .unwrap();
    let out = run_with_path(&["build", "--dir", &dir]);
    let stderr = stderr_of(&out);
    assert!(!out.status.success(), "{stderr}");
    assert!(
        stderr.contains("TS2322") && !stderr.contains("TS5101"),
        "{stderr}"
    );

    std::fs::write(
        f.path().join("tsconfig.json"),
        tsconfig.replace("\"baseUrl\": \".\",", ""),
    )
    .unwrap();
    std::fs::write(f.path().join("src/index.ts"), source).unwrap();
    let out = run_with_path(&["build", "--dir", &dir]);
    let (stdout, stderr) = (stdout_of(&out), stderr_of(&out));
    assert!(out.status.success(), "stdout: {stdout}\nstderr: {stderr}");
    assert!(!stderr.contains("ignoreDeprecations"), "{stderr}");
}

/// init, then pack, with no edits in between: the zip the builder takes
/// carries the manifest, the placeholder assets and the production bundle.
#[cfg(unix)]
#[test]
fn smart_wearable_scaffold_packs_end_to_end() {
    let Some(()) = node_available() else { return };
    let f = Fixture::new("vendored-pack");
    let dir = f.dir_arg();
    let out = run(&["init", "--dir", &dir, "--project", "smart-wearable"]);
    assert!(out.status.success(), "stderr: {}", stderr_of(&out));
    let out = run_with_path(&["pack", "--dir", &dir]);
    let stdout = stdout_of(&out);
    assert!(
        out.status.success(),
        "pack failed\nstdout: {stdout}\nstderr: {}",
        stderr_of(&out)
    );
    let zip = std::fs::File::open(f.path().join("smart-wearable.zip")).unwrap();
    let mut archive = zip::ZipArchive::new(zip).unwrap();
    let mut names: Vec<String> = (0..archive.len())
        .map(|i| archive.by_index(i).unwrap().name().unwrap().to_string())
        .collect();
    names.sort();
    for required in [
        "bin/index.js",
        "bin/scene.js",
        "bin/sdk-runtime.js",
        "model.glb",
        "scene.json",
        "thumbnail.png",
        "wearable.json",
    ] {
        assert!(
            names.iter().any(|n| n == required),
            "{required} missing from {names:?}"
        );
    }
    assert!(
        !names
            .iter()
            .any(|n| n.starts_with("src/") || n.starts_with("node_modules/")),
        "{names:?}"
    );
    let total: u64 = (0..archive.len())
        .map(|i| archive.by_index(i).unwrap().size())
        .sum();
    assert!(
        total < 2_097_152,
        "the scaffold must pack under the 2 MiB limit, got {total}"
    );
    assert!(
        !f.path().join("bin/index.js").exists(),
        "pack builds into the release tree, not the working tree"
    );
}

// Unix only: the borrowed node_modules is linked in by symlink.
#[cfg(unix)]
#[test]
#[ignore = "needs DCL_SCENE_CLI_TEST_SCENE: an installed scene checkout to borrow node_modules from; see docs/testing.md"]
fn init_scene_is_immediately_buildable_with_provisioned_node_modules() {
    let Some(src) = common::env::require_env("DCL_SCENE_CLI_TEST_SCENE") else {
        return;
    };
    let src = PathBuf::from(src);
    assert!(
        src.join("node_modules/@dcl/sdk").is_dir(),
        "DCL_SCENE_CLI_TEST_SCENE has no installed @dcl/sdk"
    );
    let f = Fixture::new("buildable");
    let dir = f.dir_arg();
    let out = run(&["init", "--dir", &dir]);
    assert!(out.status.success(), "stderr: {}", stderr_of(&out));
    std::fs::remove_dir_all(f.path().join("node_modules")).unwrap();
    std::os::unix::fs::symlink(src.join("node_modules"), f.path().join("node_modules")).unwrap();
    let out = run(&["build", "--dir", &dir]);
    let stdout = stdout_of(&out);
    assert!(
        out.status.success(),
        "build failed\nstdout: {stdout}\nstderr: {}",
        stderr_of(&out)
    );
    assert!(
        stdout.contains("SDK chunk installed") || stdout.contains("SDK chunk saved"),
        "{stdout}"
    );
    assert!(stdout.contains("Scene chunk saved"), "{stdout}");
    assert!(stdout.contains("Type check passed"), "{stdout}");
    for (rel, min) in [
        ("bin/sdk-runtime.js", 100_000usize),
        ("bin/index.js", 1_000),
        ("bin/scene.js", 100),
    ] {
        let bytes = std::fs::read(f.path().join(rel)).unwrap();
        assert!(
            bytes.len() > min,
            "{rel} suspiciously small: {} bytes",
            bytes.len()
        );
    }
}

#[test]
fn px_template_scaffold_is_a_portable_experience() {
    let f = Fixture::new("px");
    let dir = f.dir_arg();
    let out = run(&[
        "init",
        "--dir",
        &dir,
        "--project",
        "px-template",
        "--skip-install",
    ]);
    assert!(out.status.success(), "stderr: {}", stderr_of(&out));
    let scene = f.json("scene.json");
    assert_eq!(scene["isPortableExperience"], true);
    assert_eq!(scene["requiredPermissions"], serde_json::json!([]));
    assert_eq!(scene["scene"]["parcels"], serde_json::json!(["0,0"]));
    assert!(f.path().join("src/index.ts").is_file());
    assert!(
        !f.path().join("images").exists(),
        "a portable experience has no thumbnail folder"
    );
    assert!(
        !f.path().join("node_modules").exists(),
        "--skip-install leaves node_modules out"
    );
    assert!(
        stdout_of(&out).contains("Skipped node_modules"),
        "{}",
        stdout_of(&out)
    );
}

#[test]
fn scene_template_is_an_alias_for_scene() {
    let f = Fixture::new("scene-alias");
    let dir = f.dir_arg();
    let out = run(&[
        "init",
        "--dir",
        &dir,
        "--project",
        "scene-template",
        "--skip-install",
    ]);
    assert!(out.status.success(), "stderr: {}", stderr_of(&out));
    assert_eq!(
        f.json("scene.json")["isPortableExperience"],
        serde_json::Value::Null
    );
    assert!(f.path().join("images/scene-thumbnail.png").is_file());
}

#[test]
fn library_scaffold_exports_an_engine_taking_init() {
    let f = Fixture::new("library");
    let dir = f.dir_arg();
    let out = run(&[
        "init",
        "--dir",
        &dir,
        "--project",
        "library",
        "--skip-install",
    ]);
    assert!(out.status.success(), "stderr: {}", stderr_of(&out));
    let index = f.read("src/index.ts");
    assert!(
        index.contains("export function initLibrary(scene: IEngine)"),
        "{index}"
    );
    assert!(
        index.contains("export function main()"),
        "previewable as a scene: {index}"
    );
    let package = f.json("package.json");
    assert_eq!(package["types"], "dist/index.d.ts");
    assert!(
        package["scripts"]["build:lib"]
            .as_str()
            .unwrap()
            .contains("--single src/index.ts --emitDeclaration")
    );
    let tsconfig = f.json("tsconfig.json");
    assert_eq!(tsconfig["compilerOptions"]["declarationDir"], "dist");
    assert!(stdout_of(&out).contains("build:lib"), "{}", stdout_of(&out));
}

/// `build --single` on the library scaffold: the type check writes the
/// declarations and each entry bundles beside its source.
#[cfg(unix)]
#[test]
fn library_scaffold_bundles_single_files_with_declarations() {
    let Some(()) = node_available() else { return };
    let f = Fixture::new("library-single");
    let dir = f.dir_arg();
    let out = run(&["init", "--dir", &dir, "--project", "library"]);
    assert!(out.status.success(), "stderr: {}", stderr_of(&out));
    let out = run_with_path(&[
        "build",
        "--built-in",
        "--dir",
        &dir,
        "--single",
        "src/**/*.ts",
        "--emitDeclaration",
    ]);
    let stdout = stdout_of(&out);
    assert!(
        out.status.success(),
        "build failed\nstdout: {stdout}\nstderr: {}",
        stderr_of(&out)
    );
    assert!(stdout.contains("declarations emitted"), "{stdout}");
    let bundle = f.read("src/index.js");
    assert!(bundle.contains("initLibrary"), "{bundle}");
    assert!(
        bundle.contains("require(\"@dcl/sdk/ecs\")") || bundle.contains("require('@dcl/sdk/ecs')"),
        "the SDK stays external: {}",
        &bundle[..bundle.len().min(600)]
    );
    let dts = f.read("dist/index.d.ts");
    assert!(dts.contains("export declare function initLibrary"), "{dts}");
    std::fs::remove_dir_all(f.path().join("dist")).unwrap();
    let out = run_with_path(&[
        "build",
        "--built-in",
        "--dir",
        &dir,
        "--single",
        "src/**/*.ts",
        "--emitDeclaration",
    ]);
    assert!(out.status.success(), "stderr: {}", stderr_of(&out));
    assert!(
        f.path().join("dist/index.d.ts").is_file(),
        "unchanged sources and no dist: the declarations are written again"
    );
    assert!(
        !f.path().join("bin/index.js").exists(),
        "--single does not build the scene"
    );

    // A pattern that matches nothing is an error, not an empty success.
    let out = run_with_path(&["build", "--built-in", "--dir", &dir, "--single", "lib/*.ts"]);
    assert!(!out.status.success());
    assert!(
        stderr_of(&out).contains("matches no .ts"),
        "{}",
        stderr_of(&out)
    );
}

fn template_zip(top: &str, files: &[(&str, &str)]) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "dcl-scene-cli-template-{top}-{}.zip",
        std::process::id()
    ));
    let file = std::fs::File::create(&path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default();
    zip.add_directory(format!("{top}/"), opts).unwrap();
    for (rel, body) in files {
        use std::io::Write;
        zip.start_file(format!("{top}/{rel}"), opts).unwrap();
        zip.write_all(body.as_bytes()).unwrap();
    }
    zip.finish().unwrap();
    path
}

/// `--template` with a local zip: the one top-level folder's contents become
/// the project, and dependencies the vendored SDK lacks are named.
#[test]
fn template_zip_unpacks_its_top_level_folder() {
    let f = Fixture::new("template-zip");
    let dir = f.dir_arg();
    let zip = template_zip(
        "my-template-main",
        &[
            (
                "scene.json",
                r#"{"main":"bin/index.js","scene":{"parcels":["0,0"],"base":"0,0"}}"#,
            ),
            ("src/index.ts", "export function main() {}\n"),
            ("src/nested/util.ts", "export const x = 1\n"),
            (
                "package.json",
                r#"{"name":"t","dependencies":{"@dcl/sdk":"7","@dcl-sdk/utils":"1"}}"#,
            ),
        ],
    );
    let out = run(&["init", "--dir", &dir, "--template", zip.to_str().unwrap()]);
    let _ = std::fs::remove_file(&zip);
    let stdout = stdout_of(&out);
    assert!(
        out.status.success(),
        "stdout: {stdout}\nstderr: {}",
        stderr_of(&out)
    );
    assert_eq!(f.read("src/nested/util.ts"), "export const x = 1\n");
    assert!(f.path().join("scene.json").is_file());
    assert!(
        !f.path().join("my-template-main").exists(),
        "the top folder is stripped"
    );
    assert!(
        f.path()
            .join("node_modules/@dcl/sdk/package.json")
            .is_file()
    );
    assert!(stdout.contains("Unpacked 4 files"), "{stdout}");
    assert!(
        stdout.contains("@dcl-sdk/utils"),
        "missing dependency named: {stdout}"
    );
    assert!(
        !stdout.contains("@dcl/sdk,"),
        "vendored packages are not missing: {stdout}"
    );
}

#[test]
fn template_zip_must_hold_exactly_one_top_level_folder() {
    let f = Fixture::new("template-flat");
    let dir = f.dir_arg();
    let path = std::env::temp_dir().join(format!("dcl-scene-cli-flat-{}.zip", std::process::id()));
    {
        use std::io::Write;
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        let opts = zip::write::SimpleFileOptions::default();
        zip.start_file("scene.json", opts).unwrap();
        zip.write_all(b"{}").unwrap();
        zip.start_file("src/index.ts", opts).unwrap();
        zip.write_all(b"").unwrap();
        zip.finish().unwrap();
    }
    let out = run(&["init", "--dir", &dir, "--template", path.to_str().unwrap()]);
    let _ = std::fs::remove_file(&path);
    assert!(!out.status.success());
    assert!(
        stderr_of(&out).contains("exactly one top-level folder"),
        "{}",
        stderr_of(&out)
    );

    let out = run(&[
        "init",
        "--dir",
        &dir,
        "--template",
        "/nonexistent/template.zip",
    ]);
    assert!(!out.status.success());
    assert!(
        stderr_of(&out).contains("neither a file nor an http(s) URL"),
        "{}",
        stderr_of(&out)
    );
}

#[test]
fn github_repo_url_must_name_a_repository() {
    let f = Fixture::new("github-bad");
    let dir = f.dir_arg();
    for url in [
        "https://gitlab.com/a/b",
        "https://github.com/onlyowner",
        "https://github.com/a/b/blob/main/x",
    ] {
        let out = run(&["init", "--dir", &dir, "--github-repo", url]);
        assert!(!out.status.success(), "{url} accepted");
        assert!(
            stderr_of(&out).contains("not a GitHub repository URL"),
            "{url}: {}",
            stderr_of(&out)
        );
    }
}
