//! Checks the local user's platform ban against the comms gatekeeper's public
//! `GET /users/{address}/bans` and records the answer in `PlatformBanStatus`.

use alloy_core::primitives::Address;
use bevy::{
    app::{App, Plugin, PostUpdate, Update},
    ecs::{
        change_detection::DetectChanges,
        event::{Event, EventReader, EventWriter},
        schedule::{common_conditions::resource_changed, Condition, IntoScheduleConfigs},
        system::{Local, Res, ResMut},
    },
    log::{info, warn},
    tasks::{IoTaskPool, Task},
    time::Time,
};
use bevy_console::ConsoleCommand;
use common::{
    base_domain::{self, Service},
    platform_ban::{PlatformBanInfo, PlatformBanSource, PlatformBanStatus},
    structs::CurrentRealm,
    util::{TaskCompat, TaskExt},
};
use console::DoAddConsoleCommand;
use serde::Deserialize;
use wallet::Wallet;

use crate::{AdmissionView, BanScope, SceneAdmission};

/// Seconds between two checks of a wallet that has not changed.
pub const RECHECK_SECS: f64 = 300.0;
/// The least a server's `Retry-After` is honoured as.
pub const MIN_RETRY_SECS: f64 = 5.0;
const REQUEST_TIMEOUT_SECS: u64 = 10;

/// `GET /users/{address}/bans`: `{"data":{"isBanned":bool,"ban":{..}?,"matchedOn":"device"?}}`.
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
    #[serde(default)]
    matched_on: Option<String>,
}

/// Decodes the ban-status body: `Ok(None)` when not banned, whatever else it carries; a ban
/// without its record is a ban whose record was withheld.
#[cfg(test)]
fn ban_from_json(body: &[u8]) -> Result<Option<PlatformBanInfo>, String> {
    let envelope: BanStatusEnvelope =
        serde_json::from_slice(body).map_err(|e| format!("bad ban status body: {e}"))?;
    Ok(ban_from_body(envelope.data))
}

fn ban_from_body(body: BanStatusBody) -> Option<PlatformBanInfo> {
    if !body.is_banned {
        return None;
    }
    Some(body.ban.unwrap_or_else(|| {
        PlatformBanInfo::withheld(body.matched_on.as_deref().unwrap_or("another match"))
    }))
}

/// True while the known ban came from a scene-admission refusal and the admission still shows
/// that refusal.
fn refusal_outranks_not_banned(
    source: Option<PlatformBanSource>,
    admission: &SceneAdmission,
) -> bool {
    source == Some(PlatformBanSource::SceneAdmissionRefusal)
        && matches!(
            admission.view(),
            AdmissionView::Refused {
                scope: BanScope::Platform,
                ..
            }
        )
}

/// Records a platform-scoped admission refusal as the platform ban while none is known.
fn cross_feed_admission_refusal(
    admission: Res<SceneAdmission>,
    mut status: ResMut<PlatformBanStatus>,
    time: Res<Time>,
) {
    let AdmissionView::Refused {
        scope: BanScope::Platform,
        message,
        ..
    } = admission.view()
    else {
        return;
    };
    if status.ban().is_some() {
        return;
    }
    status.apply_platform_ban(
        Some(PlatformBanInfo::from_refusal(message)),
        PlatformBanSource::SceneAdmissionRefusal,
        time.elapsed_secs_f64(),
    );
}

/// `/ban_block_refresh` asks for the platform ban to be checked again now.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/ban_block_refresh")]
struct BanBlockRefreshCommand;

/// A check was asked for.
#[derive(Event)]
struct PlatformBanRefreshRequested;

fn ban_block_refresh(
    mut input: ConsoleCommand<BanBlockRefreshCommand>,
    wallet: Res<Wallet>,
    mut refresh: EventWriter<PlatformBanRefreshRequested>,
) {
    if let Some(Ok(_)) = input.take() {
        if wallet.address().is_none() {
            input.reply_failed("not signed in");
            return;
        }
        refresh.write(PlatformBanRefreshRequested);
        input.reply_ok("platform ban check requested");
    }
}

pub(crate) struct PlatformBanPlugin;

impl Plugin for PlatformBanPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(PlatformBanStatus::new())
            .add_event::<PlatformBanRefreshRequested>()
            .add_console_command::<BanBlockRefreshCommand, _>(ban_block_refresh)
            .add_systems(
                Update,
                check_platform_ban.run_if(|| !common::structs::server_mode()),
            )
            .add_systems(
                PostUpdate,
                // before the realm change, so a refusal learnt this frame refuses it; also on a
                // status change, so a refusal still standing returns once another ban is lifted
                cross_feed_admission_refusal
                    .run_if(
                        resource_changed::<SceneAdmission>
                            .or(resource_changed::<PlatformBanStatus>),
                    )
                    .before(ipfs::change_realm),
            );
    }
}

/// Why a check gave no answer, with the delay the server asked for before the next one.
#[derive(Debug, PartialEq)]
pub struct CheckFailure {
    pub error: String,
    pub retry_after_secs: Option<f64>,
}

/// The client ban-status requests are sent with.
struct BanCheckClient(reqwest::Client);

impl Default for BanCheckClient {
    fn default() -> Self {
        Self(
            platform::http_client_builder()
                .build()
                .expect("reqwest client"),
        )
    }
}

/// One ban-status request on its way, for the address it was sent for.
struct InFlightBanCheck {
    address: Address,
    task: Task<Result<Option<PlatformBanInfo>, CheckFailure>>,
}

/// When the platform ban was last checked and when it may be checked next, for one address.
#[derive(Default)]
struct BanCheckSchedule {
    address: Option<Address>,
    last_checked: Option<f64>,
    retry_after: Option<f64>,
    /// A trigger that arrived while a request was in flight.
    retrigger: bool,
    /// The `expires_at` of the known ban that a check was already started for once it passed.
    expiry_checked: Option<String>,
    in_flight: Option<InFlightBanCheck>,
}

impl BanCheckSchedule {
    /// Takes the schedule to `address`, dropping everything about the previous one. True when
    /// the address changed.
    fn follow(&mut self, address: Option<Address>) -> bool {
        if self.address == address {
            return false;
        }
        *self = Self {
            address,
            ..Self::default()
        };
        true
    }

    /// Whether a check starts now: a trigger, the period or the server's delay elapsed, or
    /// `passed_expiry` (the known ban's passed `expires_at`) not yet checked. A kept trigger and a
    /// passed expiry wait out the server's delay; any check started after the expiry covers it.
    fn starts(&mut self, now: f64, triggered: bool, passed_expiry: Option<&str>) -> bool {
        let kept = self.retry_after.is_none() && self.retrigger;
        let expiry_due = passed_expiry.is_some()
            && self.retry_after.is_none()
            && self.expiry_checked.as_deref() != passed_expiry;
        let start =
            triggered || kept || expiry_due || due(now, self.last_checked, self.retry_after);
        if start {
            self.last_checked = Some(now);
            self.retry_after = None;
            self.retrigger = false;
            if let Some(expiry) = passed_expiry {
                self.expiry_checked = Some(expiry.to_owned());
            }
        }
        start
    }

    /// Records a failed check's delay, floored at `MIN_RETRY_SECS`.
    fn delay(&mut self, retry_after_secs: Option<f64>) {
        self.retry_after = retry_after_secs.map(|secs| secs.max(MIN_RETRY_SECS));
    }
}

/// Whether the known ban's `expires_at` has passed at `now`.
fn ban_expired(status: &PlatformBanStatus, now: web_time::SystemTime) -> bool {
    let Some(expires_at) = status.ban().and_then(|ban| ban.expires_at.as_deref()) else {
        return false;
    };
    let Ok(expires_at) = chrono::DateTime::parse_from_rfc3339(expires_at) else {
        return false;
    };
    let Ok(now) = now.duration_since(web_time::UNIX_EPOCH) else {
        return false;
    };
    expires_at.timestamp_millis() <= now.as_millis() as i64
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

/// Polls the one request in flight, dropping an answer for an address that is no longer the
/// wallet's, and starts the next one when a check is due or a known ban's expiry has passed. A
/// trigger that lands while a request is in flight starts another as soon as it completes.
#[allow(clippy::too_many_arguments)]
fn check_platform_ban(
    mut status: ResMut<PlatformBanStatus>,
    wallet: Res<Wallet>,
    realm: Option<Res<CurrentRealm>>,
    admission: Res<SceneAdmission>,
    time: Res<Time>,
    mut refresh: EventReader<PlatformBanRefreshRequested>,
    mut schedule: Local<BanCheckSchedule>,
    client: Local<BanCheckClient>,
) {
    let now = time.elapsed_secs_f64();
    if schedule.follow(wallet.address()) {
        status.reset_platform_ban();
    }
    let Some(address) = wallet.address() else {
        return;
    };
    let refresh_requested = refresh.read().count() > 0;
    let triggered =
        refresh_requested || realm.is_some_and(|realm| realm.is_changed() && realm.connected);

    if let Some(in_flight) = schedule.in_flight.as_mut() {
        let Some(result) = in_flight.task.complete() else {
            schedule.retrigger |= triggered;
            return;
        };
        let for_address = in_flight.address;
        schedule.in_flight = None;
        if for_address == address {
            match result {
                Ok(None) if refusal_outranks_not_banned(status.source(), &admission) => {
                    info!("[ban_block] platform ban check: not banned, keeping the refusal");
                }
                Ok(ban) => {
                    info!(
                        "[ban_block] platform ban check: {}",
                        if ban.is_some() {
                            "banned"
                        } else {
                            "not banned"
                        }
                    );
                    status.apply_platform_ban(ban, PlatformBanSource::BanStatusEndpoint, now);
                }
                Err(failure) => {
                    schedule.delay(failure.retry_after_secs);
                    warn!("[ban_block] platform ban check failed: {}", failure.error);
                    status.platform_ban_check_failed(failure.error, now);
                }
            }
        }
    }

    // read after the answer above: a lifted ban has no expiry to check
    let passed_expiry = status
        .ban()
        .and_then(|ban| ban.expires_at.clone())
        .filter(|_| ban_expired(&status, web_time::SystemTime::now()));
    if !schedule.starts(now, triggered, passed_expiry.as_deref()) {
        return;
    }
    let url = base_domain::url(
        Service::CommsGatekeeper,
        &format!("/users/{address:#x}/bans"),
    );
    let request = client
        .0
        .get(url)
        .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS));
    schedule.in_flight = Some(InFlightBanCheck {
        address,
        task: IoTaskPool::get().spawn_compat(fetch_ban_status(request)),
    });
}

/// The decoded answer, or the failure with the server's `Retry-After` in seconds when it sent
/// one.
async fn fetch_ban_status(
    request: reqwest::RequestBuilder,
) -> Result<Option<PlatformBanInfo>, CheckFailure> {
    let failed = |error: String| CheckFailure {
        error,
        retry_after_secs: None,
    };
    let response = platform::TracedJsonResponse::send(request)
        .await
        .map_err(|e| failed(format!("request failed: {e}")))?;
    let status = response.status();
    if status == reqwest::StatusCode::OK {
        return response
            .json::<BanStatusEnvelope>()
            .await
            .map(|envelope| ban_from_body(envelope.data))
            .map_err(|e| failed(format!("bad ban status body: {e}")));
    }
    let retry_after_secs = response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(retry_after_secs);
    response.discard();
    Err(CheckFailure {
        error: format!("status {status}"),
        retry_after_secs,
    })
}

/// A `Retry-After` value as seconds from now, capped at the platform's longest honoured delay.
fn retry_after_secs(value: &str) -> Option<f64> {
    let since_epoch = web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .ok()?;
    platform::http::parse_retry_after(value, std::time::UNIX_EPOCH + since_epoch)
        .map(|wait| wait.min(platform::http::MAX_RETRY_AFTER).as_secs_f64())
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::platform_ban::PlatformBanState;

    fn address(n: u8) -> Address {
        Address::from_slice(&[n; 20])
    }

    #[test]
    fn the_ban_status_body_decodes_as_served() {
        let banned = br#"{"data":{"isBanned":true,"ban":{"id":"b1","bannedAddress":"0xab","bannedBy":"0xcd","reason":"spam","customMessage":"go away","bannedAt":"2026-01-01T00:00:00.000Z","expiresAt":null,"liftedAt":null,"liftedBy":null,"createdAt":"2026-01-01T00:00:00.000Z"}}}"#;
        let ban = ban_from_json(banned).unwrap().unwrap();
        assert_eq!(ban.reason, "spam");
        assert_eq!(ban.matched_on, None);
        assert_eq!(ban.custom_message.as_deref(), Some("go away"));
        assert_eq!(ban.expires_at, None);
        assert_eq!(
            ban_from_json(br#"{"data":{"isBanned":false}}"#).unwrap(),
            None
        );
        assert_eq!(
            ban_from_json(br#"{"data":{"isBanned":false,"ban":{"reason":"stale"}}}"#).unwrap(),
            None
        );
        assert!(ban_from_json(b"not json").is_err());
    }

    #[test]
    fn a_device_matched_ban_is_a_ban_with_its_record_withheld() {
        let withheld = ban_from_json(br#"{"data":{"isBanned":true,"matchedOn":"device"}}"#)
            .unwrap()
            .unwrap();
        assert_eq!(withheld, PlatformBanInfo::withheld("device"));
        assert_eq!(withheld.matched_on.as_deref(), Some("device"));
        assert_eq!(withheld.reason, "");
        let unexplained = ban_from_json(br#"{"data":{"isBanned":true}}"#)
            .unwrap()
            .unwrap();
        assert_eq!(unexplained, PlatformBanInfo::withheld("another match"));
    }

    const PLATFORM_BANNED: &str = "Access denied, platform-banned user";

    #[test]
    fn an_answer_is_recorded_with_its_source() {
        let mut status = PlatformBanStatus::new();
        assert_eq!(status.ban(), None);
        status.apply_platform_ban(None, PlatformBanSource::BanStatusEndpoint, 1.0);
        assert_eq!(status.ban(), None);
        assert_eq!(status.source(), None);
        assert!(matches!(
            status.state(),
            PlatformBanState::Checked { ban: None, .. }
        ));
        let ban = PlatformBanInfo::from_refusal(PLATFORM_BANNED);
        status.apply_platform_ban(
            Some(ban.clone()),
            PlatformBanSource::SceneAdmissionRefusal,
            3.0,
        );
        assert_eq!(status.ban(), Some(&ban));
        assert_eq!(
            status.source(),
            Some(PlatformBanSource::SceneAdmissionRefusal)
        );
        status.reset_platform_ban();
        assert_eq!(status.state(), &PlatformBanState::Unknown);
        assert_eq!(status.source(), None);
    }

    #[test]
    fn a_passed_expiry_is_checked_once_after_the_server_delay() {
        const EXPIRY: Option<&str> = Some("2026-12-01T00:00:00.000Z");
        let mut schedule = BanCheckSchedule::default();
        schedule.follow(Some(address(1)));
        assert!(schedule.starts(0.0, false, None));

        // a failed check asks for 30 s; the expiry passes during the wait
        schedule.delay(Some(30.0));
        assert!(
            !schedule.starts(10.0, false, EXPIRY),
            "the server delay holds"
        );
        // the delay's own check covers the expiry
        assert!(schedule.starts(30.0, false, EXPIRY));
        assert!(!schedule.starts(31.0, false, EXPIRY), "no second check");
        assert!(!schedule.starts(200.0, false, EXPIRY));

        // a passed expiry with no delay starts one check at once
        let mut schedule = BanCheckSchedule::default();
        schedule.follow(Some(address(2)));
        assert!(schedule.starts(0.0, false, None));
        assert!(schedule.starts(1.0, false, EXPIRY));
        assert!(!schedule.starts(2.0, false, EXPIRY));
    }

    #[test]
    fn a_ban_is_expired_once_its_expiry_passes() {
        let at = |secs: u64| web_time::UNIX_EPOCH + std::time::Duration::from_secs(secs);
        // 2026-12-01T00:00:00Z
        let expiry = 1_796_083_200;
        let mut status = PlatformBanStatus::new();
        assert!(!ban_expired(&status, at(expiry)), "no ban");
        let ban = |expires_at: Option<&str>| PlatformBanInfo {
            expires_at: expires_at.map(str::to_owned),
            ..PlatformBanInfo::from_refusal("")
        };
        status.apply_platform_ban(
            Some(ban(Some("2026-12-01T00:00:00.000Z"))),
            PlatformBanSource::BanStatusEndpoint,
            1.0,
        );
        assert!(!ban_expired(&status, at(expiry - 1)));
        assert!(ban_expired(&status, at(expiry)));
        status.apply_platform_ban(Some(ban(None)), PlatformBanSource::BanStatusEndpoint, 2.0);
        assert!(!ban_expired(&status, at(expiry)), "a permanent ban");
        status.apply_platform_ban(
            Some(ban(Some("soon"))),
            PlatformBanSource::BanStatusEndpoint,
            3.0,
        );
        assert!(!ban_expired(&status, at(expiry)), "an unreadable expiry");
    }

    #[test]
    fn a_failed_check_fails_open_but_keeps_a_refusal() {
        let mut status = PlatformBanStatus::new();
        status.platform_ban_check_failed("status 500".into(), 1.0);
        assert_eq!(status.ban(), None);
        assert!(matches!(status.state(), PlatformBanState::Failed { .. }));

        let ban = PlatformBanInfo::from_refusal("banned");
        status.apply_platform_ban(
            Some(ban.clone()),
            PlatformBanSource::SceneAdmissionRefusal,
            3.0,
        );
        status.platform_ban_check_failed("status 500".into(), 4.0);
        assert_eq!(status.ban(), Some(&ban));
    }

    #[test]
    fn a_not_banned_is_dropped_only_while_the_refusal_is_on_screen() {
        let refusal = SceneAdmission::from_refusal(403, Some(PLATFORM_BANNED), false, "s");
        let scene_ban =
            SceneAdmission::from_refusal(403, Some("User is banned from this scene"), false, "s");
        let from_refusal = Some(PlatformBanSource::SceneAdmissionRefusal);
        let from_endpoint = Some(PlatformBanSource::BanStatusEndpoint);
        assert!(refusal_outranks_not_banned(from_refusal, &refusal));
        assert!(!refusal_outranks_not_banned(
            from_refusal,
            &SceneAdmission::unknown()
        ));
        assert!(!refusal_outranks_not_banned(
            from_refusal,
            &SceneAdmission::allowed("s")
        ));
        assert!(!refusal_outranks_not_banned(from_refusal, &scene_ban));
        assert!(!refusal_outranks_not_banned(from_endpoint, &refusal));
        assert!(!refusal_outranks_not_banned(None, &refusal));
    }

    /// An app with the admission, the status and the clock, running the cross-feed.
    fn cross_feed_app() -> bevy::app::App {
        let mut app = bevy::app::App::new();
        app.init_resource::<Time>()
            .init_resource::<SceneAdmission>()
            .insert_resource(PlatformBanStatus::new())
            .add_systems(
                Update,
                cross_feed_admission_refusal.run_if(resource_changed::<SceneAdmission>),
            );
        app
    }

    fn set_admission(app: &mut bevy::app::App, admission: SceneAdmission) {
        use bevy::ecs::change_detection::DetectChangesMut;
        app.world_mut()
            .resource_mut::<SceneAdmission>()
            .set_if_neq(admission);
        app.update();
    }

    #[test]
    fn a_platform_refusal_marks_the_ban_while_none_is_known() {
        let mut app = cross_feed_app();
        app.update();
        assert_eq!(
            app.world().resource::<PlatformBanStatus>().state(),
            &PlatformBanState::Unknown
        );
        set_admission(
            &mut app,
            SceneAdmission::from_refusal(403, Some(PLATFORM_BANNED), false, "s"),
        );
        let status = app.world().resource::<PlatformBanStatus>();
        assert_eq!(
            status.ban().map(|b| b.reason.as_str()),
            Some(PLATFORM_BANNED)
        );
        assert_eq!(
            status.source(),
            Some(PlatformBanSource::SceneAdmissionRefusal)
        );
        // a scene change makes the admission unknown: the ban stands
        set_admission(&mut app, SceneAdmission::unknown());
        assert!(app.world().resource::<PlatformBanStatus>().ban().is_some());
        // a scene ban is not a platform ban
        let mut app = cross_feed_app();
        set_admission(
            &mut app,
            SceneAdmission::from_refusal(403, Some("User is banned from this scene"), false, "s"),
        );
        assert_eq!(
            app.world().resource::<PlatformBanStatus>().state(),
            &PlatformBanState::Unknown
        );
    }

    #[test]
    fn a_ban_the_endpoint_reported_is_not_replaced_by_a_refusal() {
        let mut app = cross_feed_app();
        let record = PlatformBanInfo {
            reason: "spam".into(),
            ..PlatformBanInfo::from_refusal("")
        };
        app.world_mut()
            .resource_mut::<PlatformBanStatus>()
            .apply_platform_ban(
                Some(record.clone()),
                PlatformBanSource::BanStatusEndpoint,
                1.0,
            );
        set_admission(
            &mut app,
            SceneAdmission::from_refusal(403, Some(PLATFORM_BANNED), false, "s"),
        );
        let status = app.world().resource::<PlatformBanStatus>();
        assert_eq!(status.ban(), Some(&record));
        assert_eq!(status.source(), Some(PlatformBanSource::BanStatusEndpoint));
    }

    #[test]
    fn a_check_is_due_at_first_then_on_the_period_or_the_retry_delay() {
        assert!(due(0.0, None, None));
        assert!(!due(10.0, Some(0.0), None));
        assert!(due(RECHECK_SECS, Some(0.0), None));
        assert!(!due(4.0, Some(0.0), Some(5.0)));
        assert!(due(5.0, Some(0.0), Some(5.0)));
        assert_eq!(retry_after_secs("7"), Some(7.0));
        assert_eq!(retry_after_secs("3600"), Some(60.0));
        assert_eq!(retry_after_secs("soon"), None);
    }

    #[test]
    fn the_schedule_follows_the_wallet_and_honours_triggers_and_delays() {
        let mut schedule = BanCheckSchedule::default();
        assert!(schedule.follow(Some(address(1))));
        assert!(
            schedule.starts(0.0, false, None),
            "a new address is checked at once"
        );
        assert!(!schedule.starts(1.0, false, None));
        assert!(schedule.starts(1.0, true, None), "a trigger starts a check");

        // a trigger during a request is kept for when the request completes
        schedule.retrigger = true;
        assert!(schedule.starts(2.0, false, None));
        assert!(!schedule.retrigger);
        assert!(!schedule.starts(3.0, false, None));

        // a server delay is floored and then honoured, by a kept trigger too
        schedule.delay(Some(0.0));
        schedule.retrigger = true;
        assert_eq!(schedule.retry_after, Some(MIN_RETRY_SECS));
        assert!(!schedule.starts(2.0 + MIN_RETRY_SECS - 1.0, false, None));
        assert!(schedule.retrigger);
        assert!(schedule.starts(2.0 + MIN_RETRY_SECS, false, None));
        assert!(!schedule.retrigger);

        // the same address is no change; another drops the schedule
        assert!(!schedule.follow(Some(address(1))));
        assert!(schedule.last_checked.is_some());
        assert!(schedule.follow(Some(address(2))));
        assert_eq!(schedule.last_checked, None);
        assert!(schedule.follow(None));
        assert_eq!(schedule.address, None);
    }
}
