use ipfs::IpfsIo;

/// Resolves a video source to an http(s) url: scene files map to their content-server url.
/// Blocks on the ipfs context.
pub fn resolve_source(ipfs: &IpfsIo, source: &str, hash: &str) -> Result<String, String> {
    let url = match ipfs.content_url(source, hash) {
        // content_url may return a normalised copy of a plain url, so this can be any scheme
        Some(url) if url != source => url,
        _ => source.to_owned(),
    };
    http_only(url, source)
}

/// Accepts only http(s). A prefix check, not a url parse, so the helper reads the same scheme.
fn http_only(url: String, source: &str) -> Result<String, String> {
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        Ok(url)
    } else {
        Err(format!(
            "unsupported video source `{source}`: only http(s) urls and scene content files play"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::http_only;

    #[test]
    fn http_and_https_pass_as_they_are() {
        assert_eq!(
            http_only("https://host/a.mp4".to_owned(), "https://host/a.mp4"),
            Ok("https://host/a.mp4".to_owned())
        );
        assert_eq!(
            http_only("HTTP://host/a.mp4".to_owned(), "HTTP://host/a.mp4"),
            Ok("HTTP://host/a.mp4".to_owned())
        );
    }

    #[test]
    fn any_other_scheme_is_refused_whatever_its_case_or_padding() {
        for (url, source) in [
            ("tcp://10.0.0.1:6379", "TCP://10.0.0.1:6379"),
            ("tls://host:443", "tls://host:443"),
            (" http://host/a.mp4", " http://host/a.mp4"),
            ("file:///etc/passwd", "file:///etc/passwd"),
            ("assets/video/a.mp4", "assets/video/a.mp4"),
        ] {
            let error = http_only(url.to_owned(), source).unwrap_err();
            assert!(error.contains(source), "{error}");
        }
    }
}
