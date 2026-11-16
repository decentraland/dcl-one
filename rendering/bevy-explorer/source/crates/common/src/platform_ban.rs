//! The signed-in wallet's platform ban: an account-level ban from all of Decentraland.

use bevy::{ecs::resource::Resource, log::warn};
use serde::{Deserialize, Serialize};
use system_api_types::PlatformBanNotice;

/// What a realm change refused for a platform ban answers.
pub const REALM_CHANGE_BANNED: &str = "You are banned from Decentraland";

/// The signed-in wallet's platform ban: unknown, checked (banned or not) or failed.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct PlatformBanStatus {
    state: PlatformBanState,
    source: Option<PlatformBanSource>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlatformBanState {
    Unknown,
    Checked {
        ban: Option<PlatformBanInfo>,
        checked_at: f64,
    },
    Failed {
        error: String,
        checked_at: f64,
    },
}

/// The gatekeeper's `ban` object, as served; dates stay ISO strings.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformBanInfo {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub custom_message: Option<String>,
    #[serde(default)]
    pub banned_at: String,
    #[serde(default)]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub banned_by: String,
    /// What a withheld record matched (`device`); `None` for a record as served.
    #[serde(default)]
    pub matched_on: Option<String>,
}

/// How the platform ban became known.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlatformBanSource {
    BanStatusEndpoint,
    SceneAdmissionRefusal,
}

impl PlatformBanInfo {
    /// A ban with no record; `matched_on` is what it matched.
    pub fn withheld(matched_on: &str) -> Self {
        Self {
            id: String::new(),
            reason: String::new(),
            custom_message: None,
            banned_at: String::new(),
            expires_at: None,
            banned_by: String::new(),
            matched_on: Some(matched_on.to_owned()),
        }
    }

    /// A ban known only from a scene-admission refusal: the message is all there is.
    pub fn from_refusal(message: &str) -> Self {
        Self {
            id: String::new(),
            reason: message.to_owned(),
            custom_message: None,
            banned_at: String::new(),
            expires_at: None,
            banned_by: String::new(),
            matched_on: None,
        }
    }
}

impl PlatformBanStatus {
    pub fn new() -> Self {
        Self {
            state: PlatformBanState::Unknown,
            source: None,
        }
    }

    pub fn state(&self) -> &PlatformBanState {
        &self.state
    }

    /// `Some` only while a ban is known.
    pub fn source(&self) -> Option<PlatformBanSource> {
        match &self.state {
            PlatformBanState::Checked { ban: Some(_), .. } => self.source,
            _ => None,
        }
    }

    /// `Some` only for a checked ban; unknown and failed are `None`.
    pub fn ban(&self) -> Option<&PlatformBanInfo> {
        match &self.state {
            PlatformBanState::Checked { ban, .. } => ban.as_ref(),
            _ => None,
        }
    }

    /// Records an answer; the same answer again only moves `checked_at`.
    pub fn apply_platform_ban(
        &mut self,
        ban: Option<PlatformBanInfo>,
        source: PlatformBanSource,
        checked_at: f64,
    ) {
        self.source = ban.is_some().then_some(source);
        self.state = PlatformBanState::Checked { ban, checked_at };
    }

    /// Records a failed check. A known ban is kept: a failed poll is no evidence against it.
    pub fn platform_ban_check_failed(&mut self, error: String, checked_at: f64) {
        if self.ban().is_some() {
            warn!("[ban_block] platform ban check failed, keeping the known ban: {error}");
            return;
        }
        self.state = PlatformBanState::Failed { error, checked_at };
        self.source = None;
    }

    /// Forgets the answer.
    pub fn reset_platform_ban(&mut self) {
        self.state = PlatformBanState::Unknown;
        self.source = None;
    }
}

impl Default for PlatformBanStatus {
    fn default() -> Self {
        Self::new()
    }
}

/// Not banned unless a ban is known. A refusal-learnt ban has no reason: its text is the
/// gatekeeper's refusal, not a ban record.
impl From<&PlatformBanStatus> for PlatformBanNotice {
    fn from(status: &PlatformBanStatus) -> Self {
        match status.ban() {
            None => Self::default(),
            Some(ban) => Self {
                banned: true,
                reason: match status.source() {
                    Some(PlatformBanSource::SceneAdmissionRefusal) => String::new(),
                    _ => ban.reason.clone(),
                },
                custom_message: ban.custom_message.clone(),
                expires_at: ban.expires_at.clone(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn endpoint_ban() -> PlatformBanInfo {
        PlatformBanInfo {
            reason: "spam".to_owned(),
            ..PlatformBanInfo::withheld("")
        }
    }

    #[test]
    fn a_failed_check_keeps_a_ban_the_endpoint_confirmed() {
        let mut status = PlatformBanStatus::new();
        status.apply_platform_ban(
            Some(endpoint_ban()),
            PlatformBanSource::BanStatusEndpoint,
            1.0,
        );
        status.platform_ban_check_failed("status 503".to_owned(), 2.0);
        assert_eq!(status.ban(), Some(&endpoint_ban()));
        assert_eq!(status.source(), Some(PlatformBanSource::BanStatusEndpoint));
    }

    #[test]
    fn a_failed_check_without_a_known_ban_fails_open() {
        let mut status = PlatformBanStatus::new();
        status.platform_ban_check_failed("status 503".to_owned(), 1.0);
        assert_eq!(status.ban(), None);
        status.apply_platform_ban(None, PlatformBanSource::BanStatusEndpoint, 2.0);
        status.platform_ban_check_failed("status 503".to_owned(), 3.0);
        assert_eq!(status.ban(), None);
    }

    #[test]
    fn the_notice_carries_a_record_reason_and_no_refusal_text() {
        let mut status = PlatformBanStatus::new();
        assert_eq!(
            PlatformBanNotice::from(&status),
            PlatformBanNotice::default()
        );
        status.apply_platform_ban(
            Some(endpoint_ban()),
            PlatformBanSource::BanStatusEndpoint,
            1.0,
        );
        assert_eq!(PlatformBanNotice::from(&status).reason, "spam");
        status.reset_platform_ban();
        status.apply_platform_ban(
            Some(PlatformBanInfo::from_refusal(
                "Access denied, platform-banned user",
            )),
            PlatformBanSource::SceneAdmissionRefusal,
            2.0,
        );
        let notice = PlatformBanNotice::from(&status);
        assert!(notice.banned);
        assert_eq!(notice.reason, "");
    }

    #[test]
    fn the_hud_matches_the_refusal_text() {
        let protocol = std::path::Path::new(env!("DCL_ONE_ROOT"))
            .join("library/explorer-core/engine/protocol.ts");
        let source = std::fs::read_to_string(&protocol).expect("protocol.ts");
        assert!(
            source.contains(&format!(
                "export const PLATFORM_BANNED_REFUSAL = '{REALM_CHANGE_BANNED}'"
            )),
            "PLATFORM_BANNED_REFUSAL in protocol.ts differs from REALM_CHANGE_BANNED"
        );
    }
}
