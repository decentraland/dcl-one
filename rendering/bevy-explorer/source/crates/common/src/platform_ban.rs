//! The signed-in wallet's platform ban: an account-level ban from all of Decentraland.

use bevy::{ecs::resource::Resource, log::warn};
use serde::{Deserialize, Serialize};

/// What a realm change refused for a platform ban answers.
pub const PLATFORM_BANNED: &str = "You are banned from Decentraland";

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

    /// Records a failed check. A ban learnt from a scene-admission refusal is kept: a failed poll
    /// is no evidence against it.
    pub fn platform_ban_check_failed(&mut self, error: String, checked_at: f64) {
        if self.source() == Some(PlatformBanSource::SceneAdmissionRefusal) {
            warn!("[ban_block] platform ban check failed, keeping the refusal: {error}");
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
