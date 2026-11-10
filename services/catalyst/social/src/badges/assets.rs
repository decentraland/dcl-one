//! The badge art served at `/badges/assets/{path}` from `BADGES_ASSETS_DIR`,
//! upstream's CDN layout (`<badge>[/<tier slot>]/{2d,3d}/<file>.png`) mirrored
//! there by the operator, for a node whose `BADGES_PUBLIC_ASSET_BASE_URL`
//! is `<node>/badges/assets`. With the default base (upstream's CDN) nothing
//! links here. No art is embedded.

use std::path::{Path, PathBuf};

use axum::body::Body;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};

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

/// The file under `assets_dir`, else 404 (no directory configured, a
/// missing file or a refused path).
pub async fn serve(assets_dir: Option<&PathBuf>, raw_path: &str) -> Response {
    let (Some(path), Some(dir)) = (sanitize(raw_path), assets_dir) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match tokio::fs::read(dir.join(path)).await {
        Ok(bytes) => respond(path, bytes),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    async fn serve_reads_the_mirror_directory_only() {
        assert_eq!(
            serve(None, "traveler/starter/2d/normal.png").await.status(),
            StatusCode::NOT_FOUND
        );
        let dir =
            std::env::temp_dir().join(format!("catalyst-badges-assets-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("traveler/starter/3d")).unwrap();
        std::fs::write(dir.join("traveler/starter/3d/basecolor.png"), b"mirrored").unwrap();
        let resp = serve(Some(&dir), "traveler/starter/3d/basecolor.png").await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(resp.headers()[header::CONTENT_TYPE], "image/png");
        let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        assert_eq!(&bytes[..], b"mirrored");
        for missing in [
            "traveler/starter/2d/normal.png",
            "nope/2d/normal.png",
            "../../etc/passwd",
        ] {
            assert_eq!(
                serve(Some(&dir), missing).await.status(),
                StatusCode::NOT_FOUND,
                "{missing}"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
