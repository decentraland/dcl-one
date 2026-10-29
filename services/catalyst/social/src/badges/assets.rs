//! The badge art served at `/badges/assets/{path}`. The 23 PNGs the seed
//! rows advertise are embedded in the binary (5.3 MB: open_for_business and
//! decentraland_citizen are the real art mirrored once from the public
//! assets CDN, walkabout and emotionista solid-colour placeholders pending
//! the authoritative definitions), so a node serves them with no directory
//! to configure; `BADGES_ASSETS_DIR` overrides the set file by file for a
//! deployment that mirrors real art.

use std::path::{Path, PathBuf};

use axum::body::Body;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};

pub const EMBEDDED: &[(&str, &[u8])] = &[
    (
        "decentraland_citizen/2d/normal.png",
        include_bytes!("assets/decentraland_citizen/2d/normal.png"),
    ),
    (
        "decentraland_citizen/3d/baseColor.png",
        include_bytes!("assets/decentraland_citizen/3d/baseColor.png"),
    ),
    (
        "decentraland_citizen/3d/hrm.png",
        include_bytes!("assets/decentraland_citizen/3d/hrm.png"),
    ),
    (
        "decentraland_citizen/3d/normal.png",
        include_bytes!("assets/decentraland_citizen/3d/normal.png"),
    ),
    (
        "emotionista/2d/normal.png",
        include_bytes!("assets/emotionista/2d/normal.png"),
    ),
    (
        "emotionista/3d/baseColor.png",
        include_bytes!("assets/emotionista/3d/baseColor.png"),
    ),
    (
        "emotionista/3d/hrm.png",
        include_bytes!("assets/emotionista/3d/hrm.png"),
    ),
    (
        "emotionista/3d/normal.png",
        include_bytes!("assets/emotionista/3d/normal.png"),
    ),
    (
        "emotionista/bronze/2d/normal.png",
        include_bytes!("assets/emotionista/bronze/2d/normal.png"),
    ),
    (
        "emotionista/gold/2d/normal.png",
        include_bytes!("assets/emotionista/gold/2d/normal.png"),
    ),
    (
        "emotionista/silver/2d/normal.png",
        include_bytes!("assets/emotionista/silver/2d/normal.png"),
    ),
    (
        "open_for_business/2d/normal.png",
        include_bytes!("assets/open_for_business/2d/normal.png"),
    ),
    (
        "open_for_business/3d/baseColor.png",
        include_bytes!("assets/open_for_business/3d/baseColor.png"),
    ),
    (
        "open_for_business/3d/hrm.png",
        include_bytes!("assets/open_for_business/3d/hrm.png"),
    ),
    (
        "open_for_business/3d/normal.png",
        include_bytes!("assets/open_for_business/3d/normal.png"),
    ),
    (
        "walkabout/2d/normal.png",
        include_bytes!("assets/walkabout/2d/normal.png"),
    ),
    (
        "walkabout/3d/baseColor.png",
        include_bytes!("assets/walkabout/3d/baseColor.png"),
    ),
    (
        "walkabout/3d/hrm.png",
        include_bytes!("assets/walkabout/3d/hrm.png"),
    ),
    (
        "walkabout/3d/normal.png",
        include_bytes!("assets/walkabout/3d/normal.png"),
    ),
    (
        "walkabout/bronze/2d/normal.png",
        include_bytes!("assets/walkabout/bronze/2d/normal.png"),
    ),
    (
        "walkabout/gold/2d/normal.png",
        include_bytes!("assets/walkabout/gold/2d/normal.png"),
    ),
    (
        "walkabout/silver/2d/normal.png",
        include_bytes!("assets/walkabout/silver/2d/normal.png"),
    ),
    (
        "walkabout/starter/2d/normal.png",
        include_bytes!("assets/walkabout/starter/2d/normal.png"),
    ),
];

/// A relative path made of plain segments: no `..`, no absolute, no
/// backslashes, no NUL, so an override directory can never be escaped.
pub fn sanitize(path: &str) -> Option<&str> {
    let path = path.trim_start_matches('/');
    if path.is_empty() || path.contains('\\') || path.contains('\0') || path.contains("//") {
        return None;
    }
    if path
        .split('/')
        .any(|seg| seg.is_empty() || seg == "." || seg == ".." || seg.starts_with('.'))
    {
        return None;
    }
    if !path
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'_' | b'-' | b'.'))
    {
        return None;
    }
    Some(path)
}

pub fn embedded(path: &str) -> Option<&'static [u8]> {
    EMBEDDED
        .iter()
        .find(|(p, _)| *p == path)
        .map(|(_, bytes)| *bytes)
}

fn content_type(path: &str) -> &'static str {
    match Path::new(path).extension().and_then(|e| e.to_str()) {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("glb") => "model/gltf-binary",
        _ => "application/octet-stream",
    }
}

fn respond(path: &str, bytes: Vec<u8>) -> Response {
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type(path).to_string()),
            (header::CACHE_CONTROL, "public, max-age=86400".to_string()),
        ],
        Body::from(bytes),
    )
        .into_response()
}

/// The override directory first (a missing file there falls through, a
/// missing directory is not an error), then the embedded set, else 404.
pub async fn serve(assets_dir: Option<&PathBuf>, raw_path: &str) -> Response {
    let Some(path) = sanitize(raw_path) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if let Some(dir) = assets_dir
        && let Ok(bytes) = tokio::fs::read(dir.join(path)).await
    {
        return respond(path, bytes);
    }
    match embedded(path) {
        Some(bytes) => respond(path, bytes.to_vec()),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The inventory the seed rows advertise (migration 0002 + the 0005
    /// fix-up), set-equal both ways: a missing file breaks the API's
    /// promise, a stray file means the seed and this pin no longer describe
    /// the tree. The two 0002-advertised `open_for_business/2d/{hrm,baseColor}.png`
    /// URLs stay unbacked (upstream serves no 2d hrm/baseColor for any
    /// badge) and so are absent here.
    #[test]
    fn advertised_assets_are_backed_by_real_png_files() {
        const BACKED: &[&str] = &[
            "decentraland_citizen/2d/normal.png",
            "decentraland_citizen/3d/baseColor.png",
            "decentraland_citizen/3d/hrm.png",
            "decentraland_citizen/3d/normal.png",
            "emotionista/2d/normal.png",
            "emotionista/3d/baseColor.png",
            "emotionista/3d/hrm.png",
            "emotionista/3d/normal.png",
            "emotionista/bronze/2d/normal.png",
            "emotionista/gold/2d/normal.png",
            "emotionista/silver/2d/normal.png",
            "open_for_business/2d/normal.png",
            "open_for_business/3d/baseColor.png",
            "open_for_business/3d/hrm.png",
            "open_for_business/3d/normal.png",
            "walkabout/2d/normal.png",
            "walkabout/3d/baseColor.png",
            "walkabout/3d/hrm.png",
            "walkabout/3d/normal.png",
            "walkabout/bronze/2d/normal.png",
            "walkabout/gold/2d/normal.png",
            "walkabout/silver/2d/normal.png",
            "walkabout/starter/2d/normal.png",
        ];
        const MIN_BYTES: usize = 5 * 1024;
        const PNG_MAGIC: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];
        let mut found: Vec<&str> = EMBEDDED.iter().map(|(p, _)| *p).collect();
        found.sort();
        assert_eq!(
            found, BACKED,
            "the embedded set diverged from the advertised inventory"
        );
        for (rel, bytes) in EMBEDDED {
            assert!(
                bytes.starts_with(&PNG_MAGIC),
                "{rel} is not a PNG (bad magic)"
            );
            assert!(
                bytes.len() > MIN_BYTES,
                "{rel} is {} bytes: placeholder-sized, not real art",
                bytes.len()
            );
        }
    }

    #[test]
    fn sanitize_refuses_traversal_and_odd_paths() {
        assert_eq!(
            sanitize("walkabout/2d/normal.png"),
            Some("walkabout/2d/normal.png")
        );
        assert_eq!(
            sanitize("/walkabout/2d/normal.png"),
            Some("walkabout/2d/normal.png")
        );
        for bad in [
            "../x.png",
            "a/../b.png",
            "a//b.png",
            "",
            ".hidden/x.png",
            "a\\b.png",
            "a/./b",
            "a b.png",
            "a%2fb",
        ] {
            assert_eq!(sanitize(bad), None, "{bad}");
        }
    }

    #[tokio::test]
    async fn serve_prefers_the_override_directory_and_falls_back_to_the_embedded_set() {
        let resp = serve(None, "walkabout/2d/normal.png").await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(resp.headers()[header::CONTENT_TYPE], "image/png");
        assert_eq!(
            serve(None, "nope/2d/normal.png").await.status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            serve(None, "../../etc/passwd").await.status(),
            StatusCode::NOT_FOUND
        );

        let dir =
            std::env::temp_dir().join(format!("catalyst-badges-assets-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("walkabout/2d")).unwrap();
        std::fs::write(dir.join("walkabout/2d/normal.png"), b"override-bytes").unwrap();
        let resp = serve(Some(&dir), "walkabout/2d/normal.png").await;
        let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        assert_eq!(&bytes[..], b"override-bytes");
        let resp = serve(Some(&dir), "emotionista/2d/normal.png").await;
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "a file missing from the override falls back"
        );
        let missing = dir.join("does-not-exist");
        assert_eq!(
            serve(Some(&missing), "emotionista/2d/normal.png")
                .await
                .status(),
            StatusCode::OK
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
