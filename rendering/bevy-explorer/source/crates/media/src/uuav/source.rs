use ipfs::IpfsIo;

/// The url the helper streams for a scene's source string: a content-map file resolves to its
/// content-server url, anything else must already be an http(s) url. The helper runs sandboxed
/// without file access, so nothing is downloaded to disk first.
///
/// Blocking (`content_url` reads the ipfs context); call from where the html backend calls it,
/// the source observer.
pub fn resolve_source(ipfs: &IpfsIo, source: &str, hash: &str) -> Result<String, String> {
    let url = match ipfs.content_url(source, hash) {
        // content_url hands a url-shaped source that is not in the content map back normalised
        // (scheme lowercased, whitespace trimmed), so this branch takes any scheme it parses
        Some(url) if url != source => url,
        _ => source.to_owned(),
    };
    http_only(url, source)
}

/// `url` when it is http(s), whatever branch produced it. A literal prefix test rather than a
/// url parse, so the helper's demuxer cannot read the scheme differently: it takes far more than
/// http (its own protocols included), and the whitelist it runs with is the second line of
/// defence, not the first.
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
