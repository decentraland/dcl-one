use std::io::Result;
use std::path::{Path, PathBuf};

#[path = "src/play_embed_select.rs"]
mod play_embed_select;

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=proto");
    let proto_files = [
        "proto/decentraland/sdk/development/local_development.proto",
        "proto/decentraland/kernel/comms/rfc5/ws_comms.proto",
    ];
    let proto_files: Vec<PathBuf> = proto_files.iter().map(PathBuf::from).collect();
    let fds = compile_protos(&proto_files, Path::new("proto"))?;
    prost_build::Config::new().compile_fds(fds)?;
    generate_abgen_embed()?;
    generate_livekit_embed()?;
    generate_skills_embed()?;
    generate_play_embed()?;
    Ok(())
}

/// The packaged web explorer `start` serves at `/play/` (src/play_embed.rs).
/// Only a build with PLAY_EMBED_DIR set carries one: a cargo dev build points
/// it at engine/deploy/web, the nix export at its web-embedded
/// package; unset, the table is empty and `/play/` says so. The selection
/// rules are play_embed_select.rs: a precompressed `.br` file is stored as it
/// is and stands in for its uncompressed twin, the rest of the text-like files
/// are gzipped here (served as-is to a client that accepts gzip, inflated for
/// one that does not), images and fonts stay raw. Two served paths that
/// resolve to one file on disk (the content-addressed `v/<hash>/` boot
/// payloads are symlinks to the root twins) share one static, so the 13 MB
/// wasm rides once.
fn generate_play_embed() -> Result<()> {
    println!("cargo:rerun-if-env-changed=PLAY_EMBED_DIR");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let dest = out_dir.join("play_embed_data.rs");
    let configured = std::env::var("PLAY_EMBED_DIR").unwrap_or_default();
    let configured = configured.trim();
    if configured.is_empty() {
        return std::fs::write(
            &dest,
            format!(
                "pub static FILES: &[File] = &[];\npub const PROVENANCE: &str = \"\";\npub static INLINE_SCRIPT_HASHES: &[&str] = &[];\n{}",
                csp_policy_code(None)
            ),
        );
    }
    let root = play_embed_select::embed_root(configured).unwrap_or_else(|why| panic!("{why}"));
    println!("cargo:rerun-if-changed={}", root.display());
    let mut rels = Vec::new();
    collect_files(&root, "", &mut rels)?;
    let rels: Vec<String> = rels
        .into_iter()
        .map(|r| r.trim_start_matches('/').to_string())
        .collect();
    let index_html = std::fs::read_to_string(root.join("index.html"))?;
    let boot = play_embed_select::boot_prefixes(&index_html);
    let selected = play_embed_select::select(&rels, &boot);
    let blob_dir = out_dir.join("play-embed");
    std::fs::create_dir_all(&blob_dir)?;
    let mut statics = String::new();
    let mut table = String::from("pub static FILES: &[File] = &[\n");
    let mut blobs: std::collections::HashMap<PathBuf, (String, &str, usize)> =
        std::collections::HashMap::new();
    for (i, file) in selected.iter().enumerate() {
        let abs = root.join(&file.source);
        println!("cargo:rerun-if-changed={}", abs.display());
        if let Some(parent) = abs.parent() {
            println!("cargo:rerun-if-changed={}", parent.display());
        }
        let canonical = std::fs::canonicalize(&abs)?;
        let raw = std::fs::read(&canonical)?;
        let etag = {
            let mut h: u64 = 0xcbf29ce484222325;
            fnv1a64(&mut h, file.served.as_bytes());
            fnv1a64(&mut h, &raw);
            format!("{h:016x}")
        };
        let (blob, encoding, raw_len) = match blobs.get(&canonical) {
            Some(known) => known.clone(),
            None => {
                let (source, encoding) = if file.br {
                    (canonical.display().to_string(), "Br")
                } else if play_embed_select::compressible(&file.served) {
                    let packed = gzip(&raw);
                    if packed.len() * 10 < raw.len() * 9 {
                        let blob = blob_dir.join(format!("{i}.gz"));
                        std::fs::write(&blob, packed)?;
                        (blob.display().to_string(), "Gzip")
                    } else {
                        (canonical.display().to_string(), "Identity")
                    }
                } else {
                    (canonical.display().to_string(), "Identity")
                };
                let name = format!("BLOB_{i}");
                statics.push_str(&format!(
                    "static {name}: &[u8] = include_bytes!({source:?});\n"
                ));
                let entry = (name, encoding, raw.len());
                blobs.insert(canonical.clone(), entry.clone());
                entry
            }
        };
        table.push_str(&format!(
            "    File {{ path: {:?}, bytes: {blob}, encoding: Encoding::{encoding}, raw_len: {raw_len}, etag: {etag:?} }},\n",
            file.served,
        ));
    }
    table.push_str("];\n");
    let mut code = String::from("// @generated by build.rs from PLAY_EMBED_DIR\n");
    code.push_str(&statics);
    code.push_str(&table);
    let provenance = std::fs::read_to_string(root.join("provenance.json")).unwrap_or_default();
    code.push_str(&format!("pub const PROVENANCE: &str = {provenance:?};\n"));
    // The page's inline scripts, as CSP hash sources: the enforced policy
    // play_embed.rs sends names each one, so the page runs without
    // 'unsafe-inline' (play_embed::csp_for).
    code.push_str("pub static INLINE_SCRIPT_HASHES: &[&str] = &[\n");
    for body in play_embed_select::inline_script_bodies(&index_html) {
        code.push_str(&format!("    {:?},\n", play_embed_select::csp_sha256(body)));
    }
    code.push_str("];\n");
    code.push_str(&csp_policy_code(
        std::fs::read_to_string(root.join("csp.json"))
            .ok()
            .as_deref(),
    ));
    std::fs::write(&dest, code)
}

/// The page's Content-Security-Policy directives, from the docroot's
/// `csp.json` — what engine/web/build.mts writes out of
/// web/csp-policy.mts, the one source the nginx front reads too — or, for a
/// docroot from before that file and for a build without a player, the same
/// list as of 2026-09-23. play_embed.rs adds the inline-script hashes and
/// the site.
fn csp_policy_code(csp_json: Option<&str>) -> String {
    const DEFAULT: &[(&str, &str)] = &[
        ("default-src", "'self'"),
        ("script-src", "'self' 'wasm-unsafe-eval' blob:"),
        ("worker-src", "'self' blob:"),
        ("frame-src", "'self' {site}"),
        ("frame-ancestors", "'self'"),
        ("base-uri", "'none'"),
        ("object-src", "'none'"),
        ("img-src", "* data: blob:"),
        ("media-src", "* data: blob:"),
        ("font-src", "'self' data:"),
        ("style-src", "'self' 'unsafe-inline'"),
        ("connect-src", "* https: wss: data: blob:"),
    ];
    let mut directives: Vec<(String, String)> = DEFAULT
        .iter()
        .map(|(n, v)| (n.to_string(), v.to_string()))
        .collect();
    let mut scene_worker = "sandbox_worker.js".to_string();
    let mut scene_worker_script_src = "'unsafe-eval'".to_string();
    let mut from = "the built-in list (no csp.json in the embed)";
    if let Some(json) = csp_json {
        let policy: serde_json::Value = serde_json::from_str(json)
            .unwrap_or_else(|e| panic!("PLAY_EMBED_DIR/csp.json is not JSON: {e}"));
        let list = policy["directives"]
            .as_array()
            .unwrap_or_else(|| panic!("csp.json: directives must be a list of [name, value]"));
        directives = list
            .iter()
            .map(|pair| {
                let name = pair[0].as_str();
                let value = pair[1].as_str();
                match (name, value) {
                    (Some(n), Some(v)) if !n.is_empty() && !n.contains(';') && !v.contains(';') => {
                        (n.to_string(), v.to_string())
                    }
                    _ => panic!("csp.json: bad directive {pair}"),
                }
            })
            .collect();
        if let Some(w) = policy["sceneWorker"].as_str() {
            scene_worker = w.to_string();
        }
        if let Some(x) = policy["sceneWorkerScriptSrc"].as_str() {
            scene_worker_script_src = x.to_string();
        }
        from = "PLAY_EMBED_DIR/csp.json";
    }
    assert!(
        directives.iter().any(|(n, _)| n == "script-src"),
        "csp.json: a script-src directive is required"
    );
    let mut code = format!("/// From {from}.\npub static CSP_DIRECTIVES: &[(&str, &str)] = &[\n");
    for (n, v) in &directives {
        code.push_str(&format!("    ({n:?}, {v:?}),\n"));
    }
    code.push_str("];\n");
    code.push_str(&format!(
        "pub const SCENE_WORKER: &str = {scene_worker:?};\n"
    ));
    code.push_str(&format!(
        "pub const SCENE_WORKER_SCRIPT_SRC: &str = {scene_worker_script_src:?};\n"
    ));
    code
}

fn gzip(raw: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    e.write_all(raw).expect("gzip");
    e.finish().expect("gzip")
}

/// Walking skills/ here is what makes scripts/sync-sdk-skills.sh the whole
/// update: no file list is maintained by hand, and
/// `skills::tests::embedded_matches_the_source_tree` guards the walk against a
/// missing rerun-if-changed.
fn generate_skills_embed() -> Result<()> {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let root = manifest.join("skills");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut names: Vec<String> = std::fs::read_dir(&root)?
        .flatten()
        .filter(|e| e.path().join("SKILL.md").is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    let mut code = String::from(
        "// @generated by build.rs from skills/\npub const EMBEDDED: &[EmbeddedSkill] = &[\n",
    );
    for name in &names {
        let dir = root.join(name);
        let mut rels = Vec::new();
        collect_files(&dir, "", &mut rels)?;
        let mut rels: Vec<String> = rels
            .into_iter()
            .map(|r| r.trim_start_matches('/').to_string())
            .filter(|r| !r.split('/').any(|part| part.starts_with('.')))
            .filter(|r| r != "SKILL.md")
            .collect();
        rels.sort();
        rels.insert(0, "SKILL.md".to_string());
        code.push_str(&format!("    EmbeddedSkill {{ name: {name:?}, files: &[\n"));
        for rel in &rels {
            let abs = dir.join(rel);
            println!("cargo:rerun-if-changed={}", abs.display());
            if let Some(parent) = abs.parent() {
                println!("cargo:rerun-if-changed={}", parent.display());
            }
            code.push_str(&format!(
                "        ({rel:?}, include_str!({:?})),\n",
                abs.display().to_string()
            ));
        }
        code.push_str("    ] },\n");
    }
    code.push_str("];\n");
    std::fs::write(out_dir.join("skills_embedded.rs"), code)
}

/// The data-layer and comms protos compile without protoc; the component
/// schema table crdt_gen encodes against is dcl-scene-authoring's.
fn protox_error(e: protox::Error) -> std::io::Error {
    std::io::Error::other(format!("{e:?}"))
}

/// The vendored protos carry a UTF-8 BOM, which protoc skips and protox's
/// lexer refuses; this resolver drops it before parsing.
struct BomTolerant {
    include: PathBuf,
    inner: protox::file::IncludeFileResolver,
}

impl protox::file::FileResolver for BomTolerant {
    fn resolve_path(&self, path: &Path) -> Option<String> {
        self.inner.resolve_path(path)
    }

    fn open_file(&self, name: &str) -> std::result::Result<protox::file::File, protox::Error> {
        let path = self.include.join(name);
        if !path.is_file() {
            return Err(protox::Error::file_not_found(name));
        }
        let source = std::fs::read_to_string(&path)?;
        protox::file::File::from_source(name, source.strip_prefix('\u{feff}').unwrap_or(&source))
    }
}

/// What `protoc --include_imports -o` produced, without protoc.
fn compile_protos(files: &[PathBuf], include: &Path) -> Result<prost_types::FileDescriptorSet> {
    let mut resolver = protox::file::ChainFileResolver::new();
    resolver.add(BomTolerant {
        include: include.to_path_buf(),
        inner: protox::file::IncludeFileResolver::new(include.to_path_buf()),
    });
    resolver.add(protox::file::GoogleFileResolver::new());
    let mut compiler = protox::Compiler::with_file_resolver(resolver);
    compiler.include_imports(true);
    compiler.open_files(files).map_err(protox_error)?;
    Ok(compiler.file_descriptor_set())
}

/// Unconditional, so `start` never has to tell a user to install an
/// asset-bundle server. Bytes come from the release pinned in
/// abgen-release.lock (shared cache, sha256-verified) unless ABGEN_EMBED_BIN
/// names a different abgen — the escape hatch for offline builds and nix.
/// Stored deflate-compressed: macOS arm64 is 36 MB on disk, 13 MB compressed,
/// for a few ms of inflate on first `start`.
fn generate_abgen_embed() -> Result<()> {
    println!("cargo:rerun-if-env-changed=ABGEN_EMBED_BIN");
    println!("cargo:rerun-if-env-changed=ABGEN_EMBED_CACHE");
    println!("cargo:rerun-if-changed=abgen-release.lock");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let dest = out_dir.join("abgen_embed_data.rs");

    let (bin_path, version) = match std::env::var("ABGEN_EMBED_BIN").unwrap_or_default() {
        v if v.is_empty() => {
            let lock = AbgenLock::read()?;
            (fetch_pinned_abgen(&lock)?, lock.version)
        }
        v => {
            let p = PathBuf::from(&v);
            if !p.is_file() {
                panic!("ABGEN_EMBED_BIN={v} is not a file; it must point at an abgen executable");
            }
            (p, "custom".to_string())
        }
    };
    let bin_name = bin_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .expect("ABGEN_EMBED_BIN has no file name");
    let dir = bin_path.parent().map(Path::to_path_buf).unwrap_or_default();

    let mut rels = vec![bin_name.clone()];
    for sub in ["template", "shader", "bin", "lib"] {
        collect_files(&dir.join(sub), sub, &mut rels)?;
    }
    for entry in std::fs::read_dir(&dir)?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(".dll") && entry.path().is_file() {
            rels.push(name);
        }
    }
    rels.sort();
    rels.dedup();

    let blob_dir = out_dir.join("abgen-embed");
    std::fs::create_dir_all(&blob_dir)?;
    let mut hash: u64 = 0xcbf29ce484222325;
    let mut code = String::from(
        "/// (path relative to the extraction root, deflate stream, uncompressed length)\npub static FILES: &[(&str, &[u8], usize)] = &[\n",
    );
    for (i, rel) in rels.iter().enumerate() {
        let abs = dir.join(rel);
        println!("cargo:rerun-if-changed={}", abs.display());
        let raw = std::fs::read(&abs)?;
        fnv1a64(&mut hash, rel.as_bytes());
        fnv1a64(&mut hash, &raw);
        let blob = blob_dir.join(format!("{i}.z"));
        std::fs::write(&blob, deflate(&raw))?;
        code.push_str(&format!(
            "    ({rel:?}, include_bytes!({:?}), {}),\n",
            blob.display().to_string(),
            raw.len()
        ));
    }
    code.push_str("];\n");
    code.push_str(&format!("pub const BIN_NAME: &str = {bin_name:?};\n"));
    code.push_str(&format!("pub const VERSION: &str = {version:?};\n"));
    code.push_str(&format!("pub const TAG: &str = \"{hash:016x}\";\n"));
    std::fs::write(&dest, code)
}

fn deflate(raw: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut e = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
    e.write_all(raw).expect("deflate");
    e.finish().expect("deflate")
}

fn collect_files(dir: &Path, prefix: &str, out: &mut Vec<String>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let rel = format!("{prefix}/{name}");
        if entry.path().is_dir() {
            collect_files(&entry.path(), &rel, out)?;
        } else if entry.path().is_file() {
            out.push(rel);
        }
    }
    Ok(())
}

/// A `*-release.lock`: `version`, a `url` template and one `<key> = <sha256>`
/// per downloadable, comments and blank lines skipped.
struct ReleaseLock {
    version: String,
    url: String,
    entries: std::collections::BTreeMap<String, String>,
}

impl ReleaseLock {
    fn read(path: &str) -> Result<Self> {
        let text = std::fs::read_to_string(path)?;
        let mut version = String::new();
        let mut url = String::new();
        let mut entries = std::collections::BTreeMap::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let (k, v) = (k.trim().to_string(), v.trim().to_string());
            match k.as_str() {
                "version" => version = v,
                "url" => url = v,
                _ => {
                    entries.insert(k, v);
                }
            }
        }
        assert!(
            !version.is_empty() && !url.is_empty() && !entries.is_empty(),
            "{path} is missing version, url or hashes"
        );
        Ok(ReleaseLock {
            version,
            url,
            entries,
        })
    }
}

struct AbgenLock {
    version: String,
    url: String,
    /// abgen release target -> sha256 of its archive
    targets: std::collections::BTreeMap<String, String>,
}

impl AbgenLock {
    fn read() -> Result<Self> {
        let lock = ReleaseLock::read("abgen-release.lock")?;
        Ok(AbgenLock {
            version: lock.version,
            url: lock.url,
            targets: lock.entries,
        })
    }

    /// abgen ships a standalone executable, so *-msvc maps to the -gnu
    /// archive: nothing links against it.
    fn release_target(&self) -> String {
        let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
        let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
        match (os.as_str(), arch.as_str()) {
            ("macos", "aarch64") => "aarch64-apple-darwin",
            ("macos", "x86_64") => "x86_64-apple-darwin",
            ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
            ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
            ("windows", "aarch64") => "aarch64-pc-windows-gnullvm",
            ("windows", "x86_64") => "x86_64-pc-windows-gnu",
            _ => panic!(
                "abgen publishes no release for {os}/{arch}; build an abgen for it and point ABGEN_EMBED_BIN at the executable"
            ),
        }
        .to_string()
    }
}

fn fetch_pinned_abgen(lock: &AbgenLock) -> Result<PathBuf> {
    let target = lock.release_target();
    let sha = lock.targets.get(&target).unwrap_or_else(|| {
        panic!("abgen-release.lock has no sha256 for target {target}");
    });
    let cache = abgen_cache_dir().join(&lock.version).join(&target);
    let exe = if target.contains("windows") {
        "abgen.exe"
    } else {
        "abgen"
    };
    let unpacked = cache.join("dist").join(exe);
    if unpacked.is_file() {
        return Ok(unpacked);
    }
    std::fs::create_dir_all(&cache)?;

    let url = lock
        .url
        .replace("{version}", &lock.version)
        .replace("{target}", &target);
    let archive = cache.join(format!("archive.tar.gz.{}", std::process::id()));
    println!(
        "cargo:warning=downloaded pinned abgen {} for {target} into {}",
        lock.version,
        cache.display()
    );
    let status = std::process::Command::new("curl")
        .args(["-fsSL", "--retry", "3", "--retry-delay", "2", "-o"])
        .arg(&archive)
        .arg(&url)
        .status();
    match status {
        Ok(s) if s.success() => {}
        other => {
            let _ = std::fs::remove_file(&archive);
            panic!(
                "could not download the pinned abgen from {url} ({other:?}).\n\
                 dcl-scene-cli always embeds abgen; to build without network access, unpack that \
                 archive yourself and set ABGEN_EMBED_BIN=<dir>/{exe}"
            );
        }
    }

    let got = sha256_hex(&std::fs::read(&archive)?);
    if &got != sha {
        let _ = std::fs::remove_file(&archive);
        panic!("{url} hashed to {got}, but abgen-release.lock pins {sha}");
    }

    let stage = cache.join(format!("stage.{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&stage);
    std::fs::create_dir_all(&stage)?;
    let status = std::process::Command::new("tar")
        .arg("-xzf")
        .arg(&archive)
        .arg("-C")
        .arg(&stage)
        .status()?;
    assert!(
        status.success(),
        "tar failed to unpack {}",
        archive.display()
    );
    let mut found = Vec::new();
    collect_files(&stage, "", &mut found)?;
    let rel = found
        .iter()
        .find(|r| r.rsplit('/').next() == Some(exe))
        .unwrap_or_else(|| panic!("no {exe} inside {url}"));
    let staged = stage.join(rel.trim_start_matches('/'));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755))?;
    }
    let dist_src = staged
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| stage.clone());
    if std::fs::rename(&dist_src, cache.join("dist")).is_err() && !unpacked.is_file() {
        panic!(
            "could not publish the unpacked abgen into {}",
            cache.display()
        );
    }
    let _ = std::fs::remove_dir_all(&stage);
    let _ = std::fs::remove_file(&archive);
    Ok(unpacked)
}

/// Shared across every checkout and target dir on the machine — a 13 MB
/// download per abgen release, not per `cargo clean`.
fn abgen_cache_dir() -> PathBuf {
    embed_cache_dir("ABGEN_EMBED_CACHE", "dcl-scene-cli-abgen")
}

fn livekit_cache_dir() -> PathBuf {
    embed_cache_dir("LIVEKIT_EMBED_CACHE", "dcl-scene-cli-livekit")
}

fn embed_cache_dir(env: &str, name: &str) -> PathBuf {
    if let Ok(v) = std::env::var(env)
        && !v.is_empty()
    {
        return PathBuf::from(v);
    }
    if let Ok(v) = std::env::var("CARGO_HOME")
        && !v.is_empty()
    {
        return PathBuf::from(v).join(name);
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".cache").join(name)
}

/// livekit-release.lock (shared cache, sha256-verified), LIVEKIT_EMBED_BIN, or
/// nothing: LiveKit publishes no macOS release, so a cargo build there embeds
/// no server unless one is named (brew's, nix's) and `start` looks on PATH.
/// Stored deflate-compressed like abgen: 47 MB of static Go on disk, 18 MB in
/// the binary.
fn generate_livekit_embed() -> Result<()> {
    println!("cargo:rerun-if-env-changed=LIVEKIT_EMBED_BIN");
    println!("cargo:rerun-if-env-changed=LIVEKIT_EMBED_CACHE");
    println!("cargo:rerun-if-changed=livekit-release.lock");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let dest = out_dir.join("livekit_embed_data.rs");
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let bin_name = if target_os == "windows" {
        "livekit-server.exe"
    } else {
        "livekit-server"
    };

    let named = std::env::var("LIVEKIT_EMBED_BIN").unwrap_or_default();
    let (bin_path, version): (Option<PathBuf>, String) = if named.eq_ignore_ascii_case("none")
        || named.eq_ignore_ascii_case("off")
    {
        (None, "none".to_string())
    } else if !named.is_empty() {
        let p = PathBuf::from(&named);
        if !p.is_file() {
            panic!(
                "LIVEKIT_EMBED_BIN={named} is not a file; it must point at a livekit-server executable, or be `none`"
            );
        }
        (Some(p), "custom".to_string())
    } else {
        let lock = ReleaseLock::read("livekit-release.lock")?;
        match livekit_release_asset(&target_os, &target_arch) {
            Some(asset) => (
                Some(fetch_pinned_livekit(&lock, asset, bin_name)?),
                lock.version.clone(),
            ),
            None => {
                println!(
                    "cargo:warning=no livekit-server embedded for {target_os}/{target_arch}: LiveKit publishes no release for it. \
                         `start` will run a livekit-server from PATH (brew install livekit) or LIVEKIT_SERVER_BIN, else comms stay on \
                         the built-in ws-room without voice; set LIVEKIT_EMBED_BIN to embed one"
                );
                (None, "none".to_string())
            }
        }
    };

    let mut code = String::new();
    match bin_path {
        Some(p) => {
            println!("cargo:rerun-if-changed={}", p.display());
            let raw = std::fs::read(&p)?;
            let mut hash: u64 = 0xcbf29ce484222325;
            fnv1a64(&mut hash, &raw);
            let blob = out_dir.join("livekit-server.z");
            std::fs::write(&blob, deflate(&raw))?;
            code.push_str(&format!(
                "/// The deflated livekit-server executable, empty when this build embeds none.\npub static PACKED: &[u8] = include_bytes!({:?});\n",
                blob.display().to_string()
            ));
            code.push_str(&format!("pub const RAW_LEN: usize = {};\n", raw.len()));
            code.push_str(&format!("pub const TAG: &str = \"{hash:016x}\";\n"));
        }
        None => {
            code.push_str("/// The deflated livekit-server executable, empty when this build embeds none.\npub static PACKED: &[u8] = &[];\n");
            code.push_str("pub const RAW_LEN: usize = 0;\n");
            code.push_str("pub const TAG: &str = \"none\";\n");
        }
    }
    code.push_str(&format!("pub const BIN_NAME: &str = {bin_name:?};\n"));
    code.push_str(&format!("pub const VERSION: &str = {version:?};\n"));
    std::fs::write(&dest, code)
}

/// The release asset suffix LiveKit publishes for the build target, none
/// for macOS and the rest.
fn livekit_release_asset(os: &str, arch: &str) -> Option<&'static str> {
    Some(match (os, arch) {
        ("linux", "x86_64") => "linux_amd64.tar.gz",
        ("linux", "aarch64") => "linux_arm64.tar.gz",
        ("linux", "arm") => "linux_armv7.tar.gz",
        ("windows", "x86_64") => "windows_amd64.zip",
        ("windows", "aarch64") => "windows_arm64.zip",
        _ => return None,
    })
}

fn fetch_pinned_livekit(lock: &ReleaseLock, asset: &str, bin_name: &str) -> Result<PathBuf> {
    let sha = lock.entries.get(asset).unwrap_or_else(|| {
        panic!("livekit-release.lock has no sha256 for asset {asset}");
    });
    let cache = livekit_cache_dir().join(&lock.version).join(asset);
    let unpacked = cache.join("dist").join(bin_name);
    if unpacked.is_file() {
        return Ok(unpacked);
    }
    std::fs::create_dir_all(&cache)?;

    let url = lock
        .url
        .replace("{version}", &lock.version)
        .replace("{semver}", lock.version.trim_start_matches('v'))
        .replace("{asset}", asset);
    let archive = cache.join(format!("archive.{}", std::process::id()));
    println!(
        "cargo:warning=downloaded pinned livekit-server {} ({asset}) into {}",
        lock.version,
        cache.display()
    );
    let status = std::process::Command::new("curl")
        .args(["-fsSL", "--retry", "3", "--retry-delay", "2", "-o"])
        .arg(&archive)
        .arg(&url)
        .status();
    match status {
        Ok(s) if s.success() => {}
        other => {
            let _ = std::fs::remove_file(&archive);
            panic!(
                "could not download the pinned livekit-server from {url} ({other:?}).\n\
                 to build without network access, unpack that archive yourself and set \
                 LIVEKIT_EMBED_BIN=<dir>/{bin_name}, or LIVEKIT_EMBED_BIN=none to embed no server"
            );
        }
    }
    let got = sha256_hex(&std::fs::read(&archive)?);
    if &got != sha {
        let _ = std::fs::remove_file(&archive);
        panic!("{url} hashed to {got}, but livekit-release.lock pins {sha}");
    }

    let stage = cache.join(format!("stage.{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&stage);
    std::fs::create_dir_all(&stage)?;
    let staged = stage.join(bin_name);
    if asset.ends_with(".zip") {
        unzip_member(&archive, bin_name, &staged)?;
    } else {
        let status = std::process::Command::new("tar")
            .arg("-xzf")
            .arg(&archive)
            .arg("-C")
            .arg(&stage)
            .status()?;
        assert!(
            status.success(),
            "tar failed to unpack {}",
            archive.display()
        );
        assert!(staged.is_file(), "no {bin_name} inside {url}");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755))?;
    }
    let dist = cache.join("dist");
    std::fs::create_dir_all(&dist)?;
    if std::fs::rename(&staged, &unpacked).is_err() && !unpacked.is_file() {
        panic!("could not place {bin_name} into {}", dist.display());
    }
    let _ = std::fs::remove_dir_all(&stage);
    let _ = std::fs::remove_file(&archive);
    Ok(unpacked)
}

/// The one member of a zip whose file name is `name`, written to `dest`.
fn unzip_member(archive: &Path, name: &str, dest: &Path) -> Result<()> {
    let file = std::fs::File::open(archive)?;
    let mut zip = zip::ZipArchive::new(file).map_err(std::io::Error::other)?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(std::io::Error::other)?;
        if entry
            .name()
            .map_err(std::io::Error::other)?
            .rsplit('/')
            .next()
            == Some(name)
        {
            let mut out = std::fs::File::create(dest)?;
            std::io::copy(&mut entry, &mut out)?;
            return Ok(());
        }
    }
    panic!("no {name} inside {}", archive.display());
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    let mut h = sha2::Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

fn fnv1a64(hash: &mut u64, bytes: &[u8]) {
    for &b in bytes {
        *hash ^= b as u64;
        *hash = hash.wrapping_mul(0x100000001b3);
    }
}
