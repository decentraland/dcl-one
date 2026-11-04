use ipfs::IpfsIo;

/// The url the helper streams for a scene's source string: a content-map file resolves to its
/// content-server url, anything else must already be an http(s) url. The helper runs sandboxed
/// without file access, so nothing is downloaded to disk first.
///
/// Blocking (`content_url` reads the ipfs context); call from where the html backend calls it,
/// the source observer.
pub fn resolve_source(ipfs: &IpfsIo, source: &str, hash: &str) -> Result<String, String> {
    if let Some(url) = ipfs.content_url(source, hash) {
        // content_url hands a url-shaped source back unchanged when it is not in the content map
        if url != source {
            return Ok(url);
        }
    }

    // A literal prefix test rather than a url parse, so the helper's demuxer cannot read the
    // scheme differently: it takes far more than http (its own protocols included), and the
    // whitelist it runs with is the second line of defence, not the first.
    let lower = source.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        Ok(source.to_owned())
    } else {
        Err(format!(
            "unsupported video source `{source}`: only http(s) urls and scene content files play"
        ))
    }
}
