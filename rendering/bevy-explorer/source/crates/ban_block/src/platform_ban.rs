//! The platform ban: an account-level ban of the local user's wallet from all of Decentraland,
//! read from the comms gatekeeper's public `GET /users/{address}/bans`. The one fact this crate
//! owns rather than reads from another crate.

use alloy_core::primitives::Address;
use bevy::{
    prelude::*,
    tasks::{IoTaskPool, Task},
};
use bevy_console::ConsoleCommand;
use common::{
    base_domain::{self, Service},
    structs::CurrentRealm,
    util::{TaskCompat, TaskExt},
};
use console::DoAddConsoleCommand;
use serde::{Deserialize, Serialize};
use wallet::Wallet;

use crate::BanBlockEvent;

/// Seconds between two checks of a wallet that has not changed.
pub const RECHECK_SECS: f64 = 300.0;
const REQUEST_TIMEOUT_SECS: u64 = 10;

#[derive(Resource)]
pub struct PlatformBanStatus {
    state: PlatformBanState,
    source: Option<PlatformBanSource>,
    http: reqwest::Client,
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
}

/// How the platform ban became known.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlatformBanSource {
    BanStatusEndpoint,
    SceneAdmissionRefusal,
}

#[derive(Deserialize)]
struct BanStatusEnvelope {
    data: BanStatusBody,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BanStatusBody {
    is_banned: bool,
    #[serde(default)]
    ban: Option<PlatformBanInfo>,
}

impl PlatformBanInfo {
    /// Decodes `{"data":{"isBanned":bool,"ban":{..}?}}`: `Ok(None)` when not banned, whatever
    /// else the body carries.
    pub fn from_json(body: &[u8]) -> Result<Option<Self>, String> {
        let envelope: BanStatusEnvelope =
            serde_json::from_slice(body).map_err(|e| format!("bad ban status body: {e}"))?;
        match (envelope.data.is_banned, envelope.data.ban) {
            (false, _) => Ok(None),
            (true, Some(ban)) => Ok(Some(ban)),
            (true, None) => Err("banned without a ban record".to_owned()),
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
        }
    }
}

impl PlatformBanStatus {
    pub fn new() -> Self {
        Self {
            state: PlatformBanState::Unknown,
            source: None,
            http: platform::http_client_builder()
                .build()
                .expect("reqwest client"),
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

    /// Records an answer. The event is returned only when the answer differs from the last one;
    /// the same answer again only moves `checked_at`.
    pub fn apply_platform_ban(
        &mut self,
        ban: Option<PlatformBanInfo>,
        source: PlatformBanSource,
        checked_at: f64,
    ) -> Vec<BanBlockEvent> {
        let same = matches!(&self.state, PlatformBanState::Checked { ban: previous, .. } if *previous == ban);
        self.source = ban.is_some().then_some(source);
        self.state = PlatformBanState::Checked { ban, checked_at };
        if same {
            Vec::new()
        } else {
            vec![BanBlockEvent::PlatformBanChanged]
        }
    }

    /// Records a failed check. A ban learnt from a scene-admission refusal is kept: a failed poll
    /// is no evidence against it.
    pub fn platform_ban_check_failed(
        &mut self,
        error: String,
        checked_at: f64,
    ) -> Vec<BanBlockEvent> {
        if self.source() == Some(PlatformBanSource::SceneAdmissionRefusal) {
            warn!("[ban_block] platform ban check failed, keeping the refusal: {error}");
            return Vec::new();
        }
        let was_failed = matches!(self.state, PlatformBanState::Failed { .. });
        self.state = PlatformBanState::Failed { error, checked_at };
        self.source = None;
        if was_failed {
            Vec::new()
        } else {
            vec![BanBlockEvent::PlatformBanChanged]
        }
    }

    /// Forgets the answer.
    pub fn reset_platform_ban(&mut self) -> Vec<BanBlockEvent> {
        if self.state == PlatformBanState::Unknown {
            return Vec::new();
        }
        self.state = PlatformBanState::Unknown;
        self.source = None;
        vec![BanBlockEvent::PlatformBanChanged]
    }
}

impl Default for PlatformBanStatus {
    fn default() -> Self {
        Self::new()
    }
}

/// Whether a check is due: never checked, the server's retry delay passed, or the recheck period
/// passed.
pub fn due(now: f64, last_checked: Option<f64>, retry_after: Option<f64>) -> bool {
    match (last_checked, retry_after) {
        (None, _) => true,
        (Some(last), Some(wait)) => now - last >= wait,
        (Some(last), None) => now - last >= RECHECK_SECS,
    }
}

/// `/ban_block_refresh` asks for the platform ban to be checked again now.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/ban_block_refresh")]
struct BanBlockRefreshCommand;

#[derive(Resource, Default)]
struct RefreshRequested(bool);

fn ban_block_refresh(
    mut input: ConsoleCommand<BanBlockRefreshCommand>,
    wallet: Res<Wallet>,
    mut refresh: ResMut<RefreshRequested>,
) {
    if let Some(Ok(_)) = input.take() {
        if wallet.address().is_none() {
            input.reply_failed("not signed in");
            return;
        }
        refresh.0 = true;
        input.reply_ok("platform ban check requested");
    }
}

pub(crate) fn register(app: &mut App) {
    app.insert_resource(PlatformBanStatus::new())
        .init_resource::<RefreshRequested>()
        .add_console_command::<BanBlockRefreshCommand, _>(ban_block_refresh)
        .add_systems(Update, check_platform_ban);
}

struct Check {
    address: Address,
    task: Task<Result<Option<PlatformBanInfo>, String>>,
}

#[derive(Default)]
struct CheckState {
    address: Option<Address>,
    last_checked: Option<f64>,
    retry_after: Option<f64>,
    in_flight: Option<Check>,
}

/// Polls the one request in flight, dropping an answer for an address that is no longer the
/// wallet's, and starts the next one when a check is due.
fn check_platform_ban(
    mut status: ResMut<PlatformBanStatus>,
    wallet: Res<Wallet>,
    realm: Option<Res<CurrentRealm>>,
    time: Res<Time>,
    mut refresh: ResMut<RefreshRequested>,
    mut check: Local<CheckState>,
    mut events: EventWriter<BanBlockEvent>,
) {
    let now = time.elapsed_secs_f64();
    let address = wallet.address();

    if address != check.address {
        check.address = address;
        check.last_checked = None;
        check.retry_after = None;
        check.in_flight = None;
        for event in status.reset_platform_ban() {
            events.write(event);
        }
    }
    let Some(address) = address else {
        return;
    };

    if let Some(in_flight) = check.in_flight.as_mut() {
        let Some(result) = in_flight.task.complete() else {
            return;
        };
        let for_address = in_flight.address;
        check.in_flight = None;
        if for_address != address {
            return;
        }
        let changed = match result {
            Ok(ban) => {
                info!(
                    "[ban_block] platform ban check: {}",
                    if ban.is_some() {
                        "banned"
                    } else {
                        "not banned"
                    }
                );
                status.apply_platform_ban(ban, PlatformBanSource::BanStatusEndpoint, now)
            }
            Err(error) => {
                check.retry_after = retry_after_secs(&error);
                warn!("[ban_block] platform ban check failed: {error}");
                status.platform_ban_check_failed(error, now)
            }
        };
        for event in changed {
            events.write(event);
        }
        return;
    }

    let realm_changed = realm.is_some_and(|realm| realm.is_changed() && realm.connected);
    let requested = std::mem::take(&mut refresh.0);
    if !(requested || realm_changed || due(now, check.last_checked, check.retry_after)) {
        return;
    }
    check.last_checked = Some(now);
    check.retry_after = None;
    let url = base_domain::url(
        Service::CommsGatekeeper,
        &format!("/users/{address:#x}/bans"),
    );
    let request = status
        .http
        .get(url)
        .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS));
    check.in_flight = Some(Check {
        address,
        task: IoTaskPool::get().spawn_compat(fetch_ban_status(request)),
    });
}

/// The decoded answer, or an error naming the status and, when the answer carries one, its
/// `Retry-After` in seconds.
async fn fetch_ban_status(
    request: reqwest::RequestBuilder,
) -> Result<Option<PlatformBanInfo>, String> {
    let response = platform::TracedJsonResponse::send(request)
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    let status = response.status();
    if status == reqwest::StatusCode::OK {
        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("bad ban status body: {e}"))?;
        return PlatformBanInfo::from_json(body.to_string().as_bytes());
    }
    let retry = response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| platform::http::parse_retry_after(value, std::time::SystemTime::now()))
        .map(|wait| wait.min(platform::http::MAX_RETRY_AFTER).as_secs());
    response.discard();
    match retry {
        Some(secs) => Err(format!("status {status}, retry after {secs}s")),
        None => Err(format!("status {status}")),
    }
}

fn retry_after_secs(error: &str) -> Option<f64> {
    let (_, tail) = error.split_once("retry after ")?;
    tail.trim_end_matches('s').parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ban_status_body_decodes_as_served() {
        let banned = br#"{"data":{"isBanned":true,"ban":{"id":"b1","bannedAddress":"0xab","bannedBy":"0xcd","reason":"spam","customMessage":"go away","bannedAt":"2026-01-01T00:00:00.000Z","expiresAt":null,"liftedAt":null,"liftedBy":null,"createdAt":"2026-01-01T00:00:00.000Z"}}}"#;
        let ban = PlatformBanInfo::from_json(banned).unwrap().unwrap();
        assert_eq!(ban.reason, "spam");
        assert_eq!(ban.custom_message.as_deref(), Some("go away"));
        assert_eq!(ban.expires_at, None);
        assert_eq!(
            PlatformBanInfo::from_json(br#"{"data":{"isBanned":false}}"#).unwrap(),
            None
        );
        assert_eq!(
            PlatformBanInfo::from_json(br#"{"data":{"isBanned":false,"ban":{"reason":"stale"}}}"#)
                .unwrap(),
            None
        );
        assert!(PlatformBanInfo::from_json(br#"{"data":{"isBanned":true}}"#).is_err());
        assert!(PlatformBanInfo::from_json(b"not json").is_err());
    }

    #[test]
    fn an_answer_is_an_event_only_when_it_differs() {
        let mut status = PlatformBanStatus::new();
        assert_eq!(status.ban(), None);
        assert_eq!(
            status.apply_platform_ban(None, PlatformBanSource::BanStatusEndpoint, 1.0),
            vec![BanBlockEvent::PlatformBanChanged]
        );
        assert!(status
            .apply_platform_ban(None, PlatformBanSource::BanStatusEndpoint, 2.0)
            .is_empty());
        assert_eq!(status.ban(), None);
        assert_eq!(status.source(), None);
        let ban = PlatformBanInfo::from_refusal("Access denied, platform-banned user");
        assert_eq!(
            status.apply_platform_ban(
                Some(ban.clone()),
                PlatformBanSource::SceneAdmissionRefusal,
                3.0
            ),
            vec![BanBlockEvent::PlatformBanChanged]
        );
        assert_eq!(status.ban(), Some(&ban));
        assert_eq!(
            status.source(),
            Some(PlatformBanSource::SceneAdmissionRefusal)
        );
    }

    #[test]
    fn a_failed_check_fails_open_but_keeps_a_refusal() {
        let mut status = PlatformBanStatus::new();
        assert_eq!(
            status.platform_ban_check_failed("status 500".into(), 1.0),
            vec![BanBlockEvent::PlatformBanChanged]
        );
        assert!(status
            .platform_ban_check_failed("status 500".into(), 2.0)
            .is_empty());
        assert_eq!(status.ban(), None);
        assert!(matches!(status.state(), PlatformBanState::Failed { .. }));

        let ban = PlatformBanInfo::from_refusal("banned");
        status.apply_platform_ban(
            Some(ban.clone()),
            PlatformBanSource::SceneAdmissionRefusal,
            3.0,
        );
        assert!(status
            .platform_ban_check_failed("status 500".into(), 4.0)
            .is_empty());
        assert_eq!(status.ban(), Some(&ban));

        assert_eq!(
            status.reset_platform_ban(),
            vec![BanBlockEvent::PlatformBanChanged]
        );
        assert!(status.reset_platform_ban().is_empty());
    }

    #[test]
    fn a_check_is_due_at_first_then_on_the_period_or_the_retry_delay() {
        assert!(due(0.0, None, None));
        assert!(!due(10.0, Some(0.0), None));
        assert!(due(RECHECK_SECS, Some(0.0), None));
        assert!(!due(4.0, Some(0.0), Some(5.0)));
        assert!(due(5.0, Some(0.0), Some(5.0)));
        assert_eq!(retry_after_secs("status 503, retry after 7s"), Some(7.0));
        assert_eq!(retry_after_secs("status 500"), None);
    }
}
