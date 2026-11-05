//! The `livekit-video://` addresses a scene's `VideoPlayer` may name, after unity-explorer's
//! `LivekitAddress` (`Explorer/Assets/DCL/SDKComponents/MediaStream/LivekitAddress.cs`,
//! `LiveKitMediaExtensions.cs`): the sentinel for whatever the stream manager picks, or one
//! participant's track. Unity reads `parts[2]` and `parts[3]` unchecked; here a malformed path
//! is an error, so the player can report it instead of panicking.

use std::fmt;

pub const SCHEME: &str = "livekit-video://";
/// The contract's predefined value for the stream the manager picks.
pub const CURRENT_STREAM: &str = "livekit-video://current-stream";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LivekitAddress {
    CurrentStream,
    /// `livekit-video://{identity}/{sid}`: one participant's video track.
    User {
        identity: String,
        sid: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LivekitAddressError {
    MissingSid,
    EmptyIdentity,
    EmptySid,
    ExtraSegments,
}

impl fmt::Display for LivekitAddressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::MissingSid => "no track sid after the identity",
            Self::EmptyIdentity => "empty participant identity",
            Self::EmptySid => "empty track sid",
            Self::ExtraSegments => "more than an identity and a track sid",
        })
    }
}

impl LivekitAddress {
    pub fn is_livekit(url: &str) -> bool {
        url.starts_with(SCHEME)
    }

    /// `None` for a url that is not a livekit address at all; `Some(Err)` for one whose path
    /// is neither the sentinel nor exactly `{identity}/{sid}` with both non-empty.
    pub fn parse(url: &str) -> Option<Result<Self, LivekitAddressError>> {
        let rest = url.strip_prefix(SCHEME)?;
        if rest == "current-stream" {
            return Some(Ok(Self::CurrentStream));
        }
        let mut parts = rest.split('/');
        let identity = parts.next().unwrap_or_default();
        let Some(sid) = parts.next() else {
            return Some(Err(LivekitAddressError::MissingSid));
        };
        if parts.next().is_some() {
            return Some(Err(LivekitAddressError::ExtraSegments));
        }
        if identity.is_empty() {
            return Some(Err(LivekitAddressError::EmptyIdentity));
        }
        if sid.is_empty() {
            return Some(Err(LivekitAddressError::EmptySid));
        }
        Some(Ok(Self::User {
            identity: identity.to_owned(),
            sid: sid.to_owned(),
        }))
    }

    pub fn user(identity: impl Into<String>, sid: impl Into<String>) -> Self {
        Self::User {
            identity: identity.into(),
            sid: sid.into(),
        }
    }
}

impl fmt::Display for LivekitAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CurrentStream => f.write_str(CURRENT_STREAM),
            Self::User { identity, sid } => write!(f, "{SCHEME}{identity}/{sid}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sentinel_and_a_user_track_parse_and_print_back() {
        assert_eq!(
            LivekitAddress::parse(CURRENT_STREAM),
            Some(Ok(LivekitAddress::CurrentStream))
        );
        let user = LivekitAddress::parse("livekit-video://0xabc/TR_x")
            .unwrap()
            .unwrap();
        assert_eq!(user, LivekitAddress::user("0xabc", "TR_x"));
        assert_eq!(user.to_string(), "livekit-video://0xabc/TR_x");
        assert_eq!(LivekitAddress::CurrentStream.to_string(), CURRENT_STREAM);
        assert!(LivekitAddress::is_livekit(CURRENT_STREAM));
    }

    #[test]
    fn a_malformed_path_is_an_error_and_other_schemes_are_none() {
        use LivekitAddressError::*;
        for (url, error) in [
            ("livekit-video://0xabc", MissingSid),
            ("livekit-video:///TR_x", EmptyIdentity),
            ("livekit-video://0xabc/", EmptySid),
            ("livekit-video://a/b/c", ExtraSegments),
            ("livekit-video://", MissingSid),
        ] {
            assert_eq!(LivekitAddress::parse(url), Some(Err(error)), "{url}");
        }
        assert_eq!(LivekitAddress::parse("https://example.com/v.mp4"), None);
        assert_eq!(LivekitAddress::parse("livekit-audio://0xabc/TR_x"), None);
        assert!(!LivekitAddress::is_livekit("https://example.com"));
    }
}
