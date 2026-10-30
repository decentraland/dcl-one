//! Which files of a packaged web explorer the binary carries, decided once
//! here for build.rs (which walks the tree) and `play_embed` (whose tests
//! pin the rules). Pure functions over relative paths, so both can `#[path]`
//! this file without sharing a crate — plus [`embed_root`], the one look at
//! the disk both need.

/// `PLAY_EMBED_DIR` as build.rs may use it: canonical, so the
/// `include_bytes!` paths written into OUT_DIR resolve (a relative one would
/// pass the index.html check from the crate dir and then fail on every
/// file), and holding an `index.html`. The error names the variable.
pub fn embed_root(configured: &str) -> Result<std::path::PathBuf, String> {
    let root = std::fs::canonicalize(configured).map_err(|e| {
        format!(
            "PLAY_EMBED_DIR={configured} is not a readable directory ({e}); it must point at a packaged web explorer (engine/deploy/web)"
        )
    })?;
    if !root.join("index.html").is_file() {
        return Err(format!(
            "PLAY_EMBED_DIR={configured} has no index.html; it must point at a packaged web explorer (engine/deploy/web)"
        ));
    }
    Ok(root)
}

/// The subtrees the page loads from; everything else at the root is a
/// leftover of a previous build (`*.prev-*`) or a dev-server file. `v/` is
/// apart: it holds content-addressed boot payloads, of which only the ones
/// the page names are current ([`boot_prefixes`]).
const KEPT_DIRS: &[&str] = &[
    "assets",
    "fonts",
    "modules",
    "pkg",
    "vendor",
    "draco",
    "favicon",
    "ui3-overlay",
    "renderer-artifacts",
];

/// Root-level files the page needs by name, besides `index.html`, every
/// `*.js` and every precompressed `*.br`.
const KEPT_ROOT_FILES: &[&str] = &["provenance.json", "shader_compiling_transparent2.png"];

/// The `v/<hash>/` prefixes the page boots from. `index.html` names its
/// wasm and asset bundle under a content hash (`window.__DCL_BOOT__ =
/// {"wasm":"v/<hash>/pkg/webgpu_build_bg.wasm", ...}`) and the engine
/// fetches exactly those paths, with no fallback to the root twins, so
/// those directories are current and every other `v/<hash>/` is a leftover.
pub fn boot_prefixes(index_html: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (at, _) in index_html.match_indices("\"v/") {
        let rest = &index_html[at + 3..];
        let hash: String = rest.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
        if hash.is_empty() || !rest[hash.len()..].starts_with('/') {
            continue;
        }
        let prefix = format!("v/{hash}/");
        if !out.contains(&prefix) {
            out.push(prefix);
        }
    }
    out
}

/// Whether one file of the tree (a `/`-separated path relative to its root)
/// belongs in the embed, before twin resolution. `boot` is
/// [`boot_prefixes`] of the tree's `index.html`.
pub fn wanted(rel: &str, boot: &[String]) -> bool {
    if rel.is_empty() || rel.contains(".prev-") || rel.ends_with(".d.ts") {
        return false;
    }
    if rel.split('/').any(|part| part.starts_with('.')) {
        return false;
    }
    match rel.split_once('/') {
        Some(("v", _)) => boot.iter().any(|prefix| rel.starts_with(prefix.as_str())),
        Some((dir, _)) => KEPT_DIRS.contains(&dir),
        None => {
            rel == "index.html"
                || rel.ends_with(".js")
                || rel.ends_with(".br")
                || KEPT_ROOT_FILES.contains(&rel)
        }
    }
}

/// One embedded file: the path it is served under, the path it is read from
/// (the same, or its `.br` twin) and whether the bytes are brotli.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selected {
    pub served: String,
    pub source: String,
    pub br: bool,
}

/// The files to embed out of a tree listing: unwanted files dropped, a
/// `.br` file served under its uncompressed name, and an uncompressed file
/// skipped when its `.br` twin exists. Sorted by served path. `boot` as for
/// [`wanted`].
pub fn select(rels: &[String], boot: &[String]) -> Vec<Selected> {
    let wanted: Vec<&str> = rels
        .iter()
        .map(String::as_str)
        .filter(|rel| self::wanted(rel, boot))
        .collect();
    let mut out: Vec<Selected> = wanted
        .iter()
        .filter_map(|rel| match rel.strip_suffix(".br") {
            Some(served) => Some(Selected {
                served: served.to_string(),
                source: rel.to_string(),
                br: true,
            }),
            None if wanted.contains(&format!("{rel}.br").as_str()) => None,
            None => Some(Selected {
                served: rel.to_string(),
                source: rel.to_string(),
                br: false,
            }),
        })
        .collect();
    out.sort_by(|a, b| a.served.cmp(&b.served));
    out.dedup_by(|a, b| a.served == b.served);
    out
}

/// Whether an uncompressed file is worth gzipping at build time: images,
/// fonts and media already are, and the `.br` files are brotli.
pub fn compressible(served: &str) -> bool {
    let ext = served
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default();
    !matches!(
        ext.as_str(),
        "png"
            | "jpg"
            | "jpeg"
            | "webp"
            | "gif"
            | "avif"
            | "ktx2"
            | "woff"
            | "woff2"
            | "mp3"
            | "ogg"
            | "mp4"
            | "webm"
            | "zip"
            | "gz"
            | "br"
    )
}

/// The bodies of the page's inline scripts, exactly as written between
/// `<script ...>` and `</script>`: what a CSP `'sha256-…'` source hashes.
/// A tag with a `src` loads a file and is not one; a tag's attributes are
/// not part of its body.
pub fn inline_script_bodies(html: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(at) = rest.find("<script") {
        let tag = &rest[at..];
        let Some(open_end) = tag.find('>') else { break };
        let attrs = &tag[7..open_end];
        // `<scripts>` or `<scripty …>` is another element: step past its name.
        if !(attrs.is_empty() || attrs.starts_with(|c: char| c.is_whitespace())) {
            rest = &tag[7..];
            continue;
        }
        let after = &tag[open_end + 1..];
        let Some(close) = after.find("</script>") else {
            break;
        };
        if !attrs.contains(" src=") {
            out.push(&after[..close]);
        }
        rest = &after[close + 9..];
    }
    out
}

/// One inline script as a CSP source: `sha256-<base64 of the SHA-256 of the
/// body>`, the form `script-src` takes.
pub fn csp_sha256(body: &str) -> String {
    use base64::Engine as _;
    use sha2::Digest as _;
    let digest = sha2::Sha256::digest(body.as_bytes());
    format!(
        "sha256-{}",
        base64::engine::general_purpose::STANDARD.encode(digest)
    )
}
