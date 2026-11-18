//! The badge art served at `/badges/assets/{path}` from `BADGES_ASSETS_DIR`,
//! upstream's CDN layout (`<badge>[/<tier slot>]/{2d,3d}/<file>.png`) mirrored
//! there by the operator or uploaded through the admin `PUT` of the same
//! path, for a node whose `BADGES_PUBLIC_ASSET_BASE_URL` is
//! `<node>/badges/assets`. With the default base (upstream's CDN) nothing
//! links here. No art is embedded.

use std::path::{Path, PathBuf};

use axum::body::Body;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use tokio::io::AsyncWriteExt;

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
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
        ],
        Body::from(bytes),
    )
        .into_response()
}

/// The PNG signature every uploaded file starts with.
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

/// Whether `bytes` is a PNG by its signature.
pub fn is_png(bytes: &[u8]) -> bool {
    bytes.starts_with(PNG_SIGNATURE)
}

/// Writes `bytes` at the sanitized `path` under `dir`, whole or not at all,
/// across a crash too: a hidden temporary file (never served), synced, then
/// renamed over the target and the directory synced. A failure before the
/// rename removes the temporary file; once renamed the file is live, so a
/// directory sync failure (refused by some shared volumes) is only logged.
pub async fn store(dir: &Path, path: &str, bytes: &[u8]) -> std::io::Result<()> {
    let target = dir.join(path);
    let (Some(parent), Some(name)) = (target.parent(), target.file_name()) else {
        return Err(std::io::Error::other("asset path has no file name"));
    };
    tokio::fs::create_dir_all(parent).await?;
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        name.to_string_lossy(),
        uuid::Uuid::new_v4().simple()
    ));
    let written = async {
        let mut file = tokio::fs::File::create(&temporary).await?;
        file.write_all(bytes).await?;
        file.sync_all().await?;
        drop(file);
        tokio::fs::rename(&temporary, &target).await
    }
    .await;
    if written.is_err() {
        let _ = tokio::fs::remove_file(&temporary).await;
        return written;
    }
    let synced = async { tokio::fs::File::open(parent).await?.sync_all().await }.await;
    if let Err(error) = synced {
        tracing::warn!(path, %error, "badge asset directory sync failed; the file is in place");
    }
    Ok(())
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

    #[tokio::test]
    async fn store_writes_the_file_whole_and_leaves_no_temporary() {
        let dir = std::env::temp_dir().join(format!(
            "catalyst-badges-store-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let png = [PNG_SIGNATURE, b"art"].concat();
        assert!(is_png(&png) && !is_png(b"GIF89a"));
        store(&dir, "traveler/starter/2d/normal.png", &png)
            .await
            .unwrap();
        store(&dir, "traveler/starter/2d/normal.png", &png)
            .await
            .unwrap();
        let folder = dir.join("traveler/starter/2d");
        let names: Vec<_> = std::fs::read_dir(&folder)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["normal.png"]);
        assert_eq!(std::fs::read(folder.join("normal.png")).unwrap(), png);
        // a target that cannot be replaced (a directory) leaves no temporary
        std::fs::create_dir_all(folder.join("hrm.png/x")).unwrap();
        assert!(
            store(&dir, "traveler/starter/2d/hrm.png", &png)
                .await
                .is_err()
        );
        let mut names: Vec<_> = std::fs::read_dir(&folder)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        names.sort();
        assert_eq!(names, ["hrm.png", "normal.png"]);
        std::fs::remove_dir_all(&dir).unwrap();
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
        assert_eq!(resp.headers()[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
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
