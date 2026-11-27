//! Request and file checks, as upstream does them: the file name
//! (`src/logic/file-validator.ts`), the LiveKit URL (plus this node's own
//! check) and the presenter
//! identity (`src/controllers/handlers/create-presentation-handler.ts:14-36`)
//! and the sharing-link rewrite (`src/adapters/file-provider/file-url-providers.ts`).

use std::sync::LazyLock;

use regex::Regex;

use super::session::LiveKitTarget;
use super::wire::FileType;

/// Largest presentation file, uploaded or downloaded (100 MiB).
pub const MAX_FILE_SIZE: usize = 100 * 1024 * 1024;
/// Largest JSON request body (1 MiB): it carries only a URL and the token.
pub const MAX_JSON_BODY_SIZE: usize = 1024 * 1024;

const PDF_MAGIC: &[u8] = b"%PDF";
const ZIP_MAGIC: &[u8] = &[0x50, 0x4b, 0x03, 0x04];

/// Whether the file starts with its type's magic bytes: `%PDF` for a PDF,
/// the ZIP header for a PPTX.
pub fn magic_bytes_match(bytes: &[u8], file_type: FileType) -> bool {
    let magic = match file_type {
        FileType::Pdf => PDF_MAGIC,
        FileType::Pptx => ZIP_MAGIC,
    };
    bytes.len() >= 4 && bytes.starts_with(magic)
}

/// Node's `path.posix.basename`: the last component, trailing slashes ignored.
fn basename(path: &str) -> &str {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        // basename('/') and basename('') are both ''.
        return "";
    }
    trimmed.rsplit('/').next().unwrap_or(trimmed)
}

/// `sanitizeFilename`: the basename with every UTF-16 code unit outside
/// `[a-zA-Z0-9._-]` replaced by `_` (a character outside the BMP is two).
pub fn sanitize_filename(name: &str) -> String {
    basename(name)
        .chars()
        .flat_map(|c| {
            let keep = c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-');
            let units = if keep { 1 } else { c.len_utf16() };
            std::iter::repeat_n(if keep { c } else { '_' }, units)
        })
        .collect()
}

/// Node's `path.posix.extname` for a name without `/`.
fn extname(name: &str) -> &str {
    let bytes = name.as_bytes();
    let mut start_dot: Option<usize> = None;
    let mut end: Option<usize> = None;
    // 0: no character seen before the dot yet; 1: only dots; -1: something else.
    let mut pre_dot_state = 0i8;
    for i in (0..bytes.len()).rev() {
        if end.is_none() {
            end = Some(i + 1);
        }
        if bytes[i] == b'.' {
            if start_dot.is_none() {
                start_dot = Some(i);
            } else if pre_dot_state != 1 {
                pre_dot_state = 1;
            }
        } else if start_dot.is_some() {
            pre_dot_state = -1;
        }
    }
    match (start_dot, end) {
        (Some(dot), Some(end))
            if pre_dot_state != 0 && !(pre_dot_state == 1 && dot == end - 1 && dot == 1) =>
        {
            &name[dot..end]
        }
        _ => "",
    }
}

/// `getFileTypeFromName`: the extension, lowercased, `.pdf` or `.pptx`.
pub fn file_type_from_name(name: &str) -> Option<FileType> {
    match extname(name).to_ascii_lowercase().as_str() {
        ".pdf" => Some(FileType::Pdf),
        ".pptx" => Some(FileType::Pptx),
        _ => None,
    }
}

/// The presentation's display name: the file name without its last
/// extension, or `Presentation` when nothing is left.
pub fn presentation_name(file_name: &str) -> String {
    static EXT: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\.[^.]+$").expect("literal regex is valid"));
    let stem = EXT.replace(file_name, "");
    if stem.is_empty() {
        "Presentation".to_string()
    } else {
        stem.into_owned()
    }
}

/// Why a `livekitUrl` is refused (the 400 body's message): upstream's
/// checks, then this node's own (`own`): only the node's LiveKit is joined.
pub fn check_livekit_url(value: &str, own: &LiveKitTarget) -> Result<(), &'static str> {
    let Ok(parsed) = url::Url::parse(value) else {
        return Err("livekitUrl is not a valid URL");
    };
    let host = parsed.host_str().unwrap_or_default();
    let is_own = own.is_own(value);
    // Upstream accepts wss:// only. A node whose own LiveKit is a ws:// one
    // on loopback (the local stack's, `tooling/dev.py`) accepts that URL too.
    let loopback = matches!(host, "localhost" | "127.0.0.1" | "[::1]");
    if parsed.scheme() != "wss" && !(parsed.scheme() == "ws" && loopback && is_own) {
        return Err("livekitUrl must use wss:// protocol");
    }
    if host.is_empty() {
        return Err("livekitUrl must specify a host");
    }
    if !is_own {
        return Err("livekitUrl is not this node's LiveKit server");
    }
    Ok(())
}

/// `presenterIdentity`: `stream:` plus letters, digits and `:_.-`, or a `0x`
/// address; anything else is stored as none (and never refused).
pub fn presenter_identity(value: Option<&str>) -> Option<String> {
    static PATTERN: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^(stream:[A-Za-z0-9:_.-]{1,121}|0x[0-9a-fA-F]{40})$")
            .expect("literal regex is valid")
    });
    value.filter(|v| PATTERN.is_match(v)).map(str::to_string)
}

/// A sharing link turned into its download URL, and the file name the
/// rewrite knows (Google Slides exports as `<id>.pdf`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedFileUrl {
    pub url: String,
    pub filename: Option<String>,
}

/// `resolveFileUrl`: Google Slides to its PDF export, a Drive file to its
/// download URL, anything else unchanged.
pub fn resolve_file_url(url: &str) -> ResolvedFileUrl {
    static SLIDES: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^https://docs\.google\.com/presentation/d/([a-zA-Z0-9_-]+)")
            .expect("literal regex is valid")
    });
    static DRIVE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^https://drive\.google\.com/file/d/([a-zA-Z0-9_-]+)")
            .expect("literal regex is valid")
    });
    if let Some(id) = SLIDES.captures(url).map(|c| c[1].to_string()) {
        return ResolvedFileUrl {
            url: format!("https://docs.google.com/presentation/d/{id}/export?format=pdf"),
            filename: Some(format!("{id}.pdf")),
        };
    }
    if let Some(id) = DRIVE.captures(url).map(|c| c[1].to_string()) {
        return ResolvedFileUrl {
            url: format!(
                "https://drive.usercontent.google.com/download?id={id}&export=download&confirm=t"
            ),
            filename: None,
        };
    }
    ResolvedFileUrl {
        url: url.to_string(),
        filename: None,
    }
}

/// The downloaded file's name: `Content-Disposition`'s `filename` (percent
/// decoded when it decodes), else the final URL's last path segment as it
/// stands, else `presentation`.
pub fn download_filename(content_disposition: Option<&str>, final_url: &url::Url) -> String {
    static FILENAME: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"filename\*?=(?:UTF-8''|"?)([^";]+)"?"#).expect("literal regex is valid")
    });
    if let Some(raw) = content_disposition
        .and_then(|d| FILENAME.captures(d))
        .map(|c| c[1].to_string())
    {
        // decodeURIComponent; a malformed escape keeps the raw value.
        return match urlencoding::decode(&raw) {
            Ok(decoded) if !has_bad_escape(&raw) => decoded.into_owned(),
            _ => raw,
        };
    }
    let last = final_url.path().rsplit('/').next().unwrap_or_default();
    if last.is_empty() {
        "presentation".to_string()
    } else {
        last.to_string()
    }
}

/// Whether `decodeURIComponent` would throw: a `%` not followed by two hex digits.
fn has_bad_escape(raw: &str) -> bool {
    let bytes = raw.as_bytes();
    bytes.iter().enumerate().any(|(i, &b)| {
        b == b'%'
            && !(bytes.get(i + 1).is_some_and(u8::is_ascii_hexdigit)
                && bytes.get(i + 2).is_some_and(u8::is_ascii_hexdigit))
    })
}
