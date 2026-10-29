//! Upstream endpoint checks shared by every mirror and reader.

/// A configured upstream must be a publicly routable http(s) URL that is not
/// one of this node's own hosts. The error names the variable, never the value.
pub fn check_upstream(var: &str, url: &str, self_hosts: &[String]) -> anyhow::Result<()> {
    if !crate::http::is_safe_http_url(url) {
        anyhow::bail!(
            "{var} is not a publicly routable http(s) URL; an upstream mirror source must be an \
             explicit external endpoint, never a loopback, private or single-label host"
        );
    }
    let host = url::Url::parse(url.trim())
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_ascii_lowercase()))
        .unwrap_or_default();
    if self_hosts.contains(&host) {
        anyhow::bail!(
            "{var} points at this node's own host {host}: a mirror cannot source from itself"
        );
    }
    Ok(())
}
