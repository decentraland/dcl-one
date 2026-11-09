use reqwest::Url;
use std::{fmt, net::IpAddr};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdapterKind {
    Livekit,
    Websocket,
    Archipelago,
    SignedLogin,
    Offline,
}

/// Contains no remote input: safe to include in diagnostics and logs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidAdapter;

impl fmt::Display for InvalidAdapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid or insecure room adapter")
    }
}

impl std::error::Error for InvalidAdapter {}

pub fn validate_url(address: &str) -> Result<Url, InvalidAdapter> {
    let url = Url::parse(address).map_err(|_| InvalidAdapter)?;
    let host = url.host_str().ok_or(InvalidAdapter)?;
    if !url.username().is_empty() || url.password().is_some() || url.fragment().is_some() {
        return Err(InvalidAdapter);
    }
    let loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .trim_matches(['[', ']'])
            .parse::<IpAddr>()
            .is_ok_and(|ip| ip.is_loopback());
    match url.scheme() {
        "https" | "wss" => Ok(url),
        "http" | "ws" if loopback => Ok(url),
        _ => Err(InvalidAdapter),
    }
}

pub fn livekit_credentials(address: &str) -> Result<(String, String), InvalidAdapter> {
    let mut url = validate_url(address)?;
    if !matches!(url.scheme(), "ws" | "wss") {
        return Err(InvalidAdapter);
    }
    let tokens: Vec<_> = url
        .query_pairs()
        .filter(|(key, _)| key == "access_token")
        .collect();
    let token = match tokens.as_slice() {
        [(_, value)] if !value.is_empty() => value.to_string(),
        _ => return Err(InvalidAdapter),
    };
    url.set_query(None);
    Ok((url.to_string(), token))
}

pub fn parse_adapter(mut adapter: &str) -> Result<(AdapterKind, String), InvalidAdapter> {
    for _ in 0..8 {
        let (protocol, address) = adapter.split_once(':').ok_or(InvalidAdapter)?;
        let kind = match protocol {
            "fixed-adapter" => {
                adapter = address;
                continue;
            }
            "offline" => return Ok((AdapterKind::Offline, String::new())),
            "livekit" => AdapterKind::Livekit,
            "ws-room" => AdapterKind::Websocket,
            "signed-login" => AdapterKind::SignedLogin,
            "archipelago" => AdapterKind::Archipelago,
            _ => return Err(InvalidAdapter),
        };
        let address = if matches!(kind, AdapterKind::Archipelago | AdapterKind::Websocket)
            && !address.contains("://")
        {
            format!("wss://{address}")
        } else {
            address.to_owned()
        };
        let url = validate_url(&address)?;
        let valid_scheme = if kind == AdapterKind::SignedLogin {
            matches!(url.scheme(), "http" | "https")
        } else {
            matches!(url.scheme(), "ws" | "wss")
        };
        if !valid_scheme {
            return Err(InvalidAdapter);
        }
        if kind == AdapterKind::Livekit {
            livekit_credentials(&address)?;
        }
        return Ok((kind, address));
    }
    Err(InvalidAdapter)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleartext_is_limited_to_loopback() {
        for host in ["localhost", "127.0.0.1", "127.0.0.42", "[::1]"] {
            assert!(validate_url(&format!("ws://{host}:7880/")).is_ok());
        }
        for host in [
            "example.org",
            "localhost.example.org",
            "192.168.0.1",
            "[::2]",
        ] {
            assert!(validate_url(&format!("ws://{host}:7880/")).is_err());
        }
        assert!(validate_url("wss://example.org:7880/").is_ok());
    }

    #[test]
    fn livekit_credentials_preserve_ports_and_decode_tokens() {
        let (url, token) =
            livekit_credentials("wss://example.org:7880/path?access_token=a%2Bb%3Dc").unwrap();
        assert_eq!(url, "wss://example.org:7880/path");
        assert_eq!(token, "a+b=c");
    }

    #[test]
    fn invalid_credentials_are_safe_errors() {
        for input in [
            "not a url",
            "wss://example.org",
            "wss://example.org?access_token=",
            "file:///secret",
            "wss://user:secret@example.org?access_token=secret",
        ] {
            let error = livekit_credentials(input).unwrap_err();
            assert!(!error.to_string().contains("secret"));
        }
    }

    #[test]
    fn adapter_wrappers_are_bounded_and_unknown_protocols_rejected() {
        assert!(
            parse_adapter("fixed-adapter:livekit:wss://example.org?access_token=token").is_ok()
        );
        assert!(parse_adapter(&format!("{}offline:", "fixed-adapter:".repeat(40))).is_err());
        assert!(parse_adapter("unknown:wss://example.org").is_err());
    }

    #[test]
    fn legacy_bare_socket_authorities_use_secure_websockets() {
        for protocol in ["ws-room", "archipelago"] {
            let (_, address) =
                parse_adapter(&format!("{protocol}:rooms.example.org/room")).unwrap();
            assert_eq!(address, "wss://rooms.example.org/room");
        }
    }
}
