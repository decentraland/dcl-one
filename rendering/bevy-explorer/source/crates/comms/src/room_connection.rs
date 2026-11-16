use bevy::{
    ecs::query::QueryData,
    prelude::*,
    tasks::{IoTaskPool, Task},
};
use common::util::{TaskCompat, TaskExt};
use ipfs::IpfsAssetServer;
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use wallet::{sign_service_request, Wallet};

use crate::{
    access_status::{AccessScope, AccessStatus},
    adapter::{parse_adapter, validate_url, AdapterKind},
    recovery::RetryState,
    AdapterManager,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureKind {
    Transient,
    Denied,
    Invalid,
    IdentityConflict,
    CredentialsRequired,
}

#[derive(Clone, Debug)]
pub struct AdmissionError {
    pub kind: FailureKind,
    pub retry_after: Option<f64>,
    status: Option<u16>,
    message: Option<String>,
}

impl std::fmt::Display for AdmissionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "room admission {:?}", self.kind)
    }
}
impl std::error::Error for AdmissionError {}

impl AdmissionError {
    pub fn new(kind: FailureKind) -> Self {
        Self {
            kind,
            retry_after: None,
            status: None,
            message: None,
        }
    }
    pub fn status(status: u16, retry_after: Option<f64>) -> Self {
        Self {
            kind: if matches!(status, 401 | 403) {
                FailureKind::Denied
            } else {
                FailureKind::Transient
            },
            retry_after,
            status: Some(status),
            message: None,
        }
    }
    pub fn terminal(&self) -> bool {
        self.kind != FailureKind::Transient
    }
}

#[derive(Component, Default, Clone, Debug)]
pub struct RoomRecovery {
    pub retry: RetryState,
    pub failure: Option<FailureKind>,
}

impl RoomRecovery {
    pub fn fail(&mut self, now: f64, error: AdmissionError) {
        self.failure = Some(error.kind);
        self.retry.failed(now, error.retry_after);
    }
    pub fn can_retry(&self, now: f64) -> bool {
        self.failure
            .is_none_or(|kind| kind == FailureKind::Transient)
            && self.retry.ready(now)
    }
    pub fn reset(&mut self) {
        self.failure = None;
        self.retry.reset();
    }
}

/// Request metadata is owned by the same entity as the connection it authorizes.
#[derive(Component, Clone)]
#[require(RoomRecovery)]
pub struct CredentialRequest {
    pub method: &'static str,
    pub url: String,
    pub metadata: String,
}

#[derive(Component)]
pub struct NeedsAdapter;

#[derive(Component)]
pub struct RoomContext(pub Entity);

/// Realm lifetime, including handshakes which have not produced a transport yet.
#[derive(Component)]
pub struct RealmRoom;

#[derive(Component)]
#[component(on_remove = Self::on_remove)]
pub struct RoomAccessScope(pub AccessScope);

impl RoomAccessScope {
    fn on_remove(
        mut world: bevy::ecs::world::DeferredWorld,
        context: bevy::ecs::component::HookContext,
    ) {
        let scope = world
            .get::<Self>(context.entity)
            .map(|scope| scope.0.clone());
        if let (Some(scope), Some(mut status)) = (scope, world.get_resource_mut::<AccessStatus>()) {
            status.remove_scope(&scope);
        }
    }
}

#[derive(Component)]
pub struct RealmAdapter;

#[derive(Component)]
#[component(on_remove = Self::on_remove)]
pub struct RoomSuspended;

impl RoomSuspended {
    fn on_remove(
        mut world: bevy::ecs::world::DeferredWorld,
        context: bevy::ecs::component::HookContext,
    ) {
        // A producer can enqueue after the last paused Update; discard before resuming.
        #[cfg(feature = "livekit")]
        if let Some(mut messages) =
            world.get_mut::<crate::livekit::LivekitNetworkMessage>(context.entity)
        {
            while messages.try_recv().is_ok() {}
        }
        #[cfg(not(feature = "livekit"))]
        let _ = (&mut world, context);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomState {
    Connecting,
    Connected,
    Reconnecting,
    Disconnected,
    Failed,
    Paused,
    Denied,
    Invalid,
    IdentityConflict,
    CredentialsRequired,
}

impl RoomState {
    pub fn name(self) -> &'static str {
        match self {
            Self::Connecting => "connecting",
            Self::Connected => "connected",
            Self::Reconnecting => "reconnecting",
            Self::Disconnected => "disconnected",
            Self::Failed => "failed",
            Self::Paused => "paused",
            Self::Denied => "denied",
            Self::Invalid => "invalid",
            Self::IdentityConflict => "identity_conflict",
            Self::CredentialsRequired => "credentials_required",
        }
    }
}

/// The same lifecycle reading for socket and LiveKit rooms, including admission failures.
#[derive(QueryData)]
pub struct RoomStatus {
    pub recovery: &'static RoomRecovery,
    pub suspended: Has<RoomSuspended>,
    pub socket: Option<&'static ConnectionHealth>,
    #[cfg(feature = "livekit")]
    pub connected: Has<crate::livekit::room::Connected>,
    #[cfg(feature = "livekit")]
    pub connecting: Has<crate::livekit::room::Connecting>,
    #[cfg(feature = "livekit")]
    pub reconnecting: Has<crate::livekit::room::Reconnecting>,
    #[cfg(feature = "livekit")]
    pub disconnected: Has<crate::livekit::room::Disconnected>,
}

impl RoomStatusItem<'_> {
    pub fn state(&self) -> RoomState {
        if self.suspended {
            return RoomState::Paused;
        }
        match self.recovery.failure {
            Some(FailureKind::Denied) => return RoomState::Denied,
            Some(FailureKind::Invalid) => return RoomState::Invalid,
            Some(FailureKind::IdentityConflict) => return RoomState::IdentityConflict,
            Some(FailureKind::CredentialsRequired) => return RoomState::CredentialsRequired,
            _ => {}
        }
        if self.socket.is_some_and(ConnectionHealth::is_connected) {
            return RoomState::Connected;
        }
        #[cfg(feature = "livekit")]
        {
            if self.connected {
                return RoomState::Connected;
            }
            if self.reconnecting {
                return RoomState::Reconnecting;
            }
            if self.disconnected {
                return RoomState::Disconnected;
            }
        }
        if self.recovery.failure.is_some() {
            RoomState::Failed
        } else if self.recovery.retry.attempts > 0 {
            RoomState::Reconnecting
        } else {
            RoomState::Connecting
        }
    }
}

/// Authenticated socket health shared with its async driver and the connectivity watchdog.
#[derive(Component, Clone, Default)]
pub struct ConnectionHealth(Arc<AtomicBool>);

impl ConnectionHealth {
    pub fn is_connected(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
    pub fn connected(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn disconnected(&self) {
        self.0.store(false, Ordering::Release);
    }
    pub fn reset_on_drop(&self) -> ConnectionHealthGuard {
        ConnectionHealthGuard(self.clone())
    }
}

/// Cancelling a socket future clears health just like an ordinary socket close.
pub struct ConnectionHealthGuard(ConnectionHealth);

impl Drop for ConnectionHealthGuard {
    fn drop(&mut self) {
        self.0.disconnected();
    }
}

#[derive(Component)]
pub(crate) struct CredentialTask {
    task: Task<Result<String, AdmissionError>>,
    identity: alloy_core::primitives::Address,
}

#[derive(Component)]
struct PublishedDenial;

pub struct RoomConnectionPlugin;

impl Plugin for RoomConnectionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AccessStatus>().add_systems(
            Update,
            (
                sync_access_identity,
                apply_access_changes,
                request_credentials,
                finish_credentials,
                publish_access,
            )
                .chain(),
        );
        app.add_systems(PostUpdate, observe_stable_connections);
    }
}

pub(crate) fn observe_stable_connections(
    mut rooms: Query<(
        Entity,
        &mut RoomRecovery,
        Option<&ConnectionHealth>,
        Has<RoomSuspended>,
    )>,
    #[cfg(feature = "livekit")] connected_rooms: Query<(), With<crate::livekit::room::Connected>>,
    time: Res<Time>,
) {
    for (entity, mut recovery, health, suspended) in &mut rooms {
        let connected = health.is_some_and(ConnectionHealth::is_connected);
        #[cfg(feature = "livekit")]
        let connected = connected || connected_rooms.contains(entity);
        #[cfg(not(feature = "livekit"))]
        let _ = entity;
        let healthy = connected && !suspended && recovery.failure.is_none();
        recovery
            .retry
            .observe_connection(healthy, time.elapsed_secs_f64());
    }
}

fn sync_access_identity(wallet: Res<Wallet>, mut status: ResMut<AccessStatus>) {
    if wallet.is_changed() {
        status.set_identity(wallet.address());
    }
}

fn publish_access(
    mut commands: Commands,
    rooms: Query<(Entity, &RoomAccessScope, &RoomRecovery), Changed<RoomRecovery>>,
    mut status: ResMut<AccessStatus>,
) {
    for (entity, scope, recovery) in &rooms {
        if recovery.failure == Some(FailureKind::Denied) {
            status.set_denied(scope.0.clone());
            commands.entity(entity).try_insert(PublishedDenial);
        } else if recovery.failure.is_none() {
            status.remove_scope(&scope.0);
            commands.entity(entity).try_remove::<PublishedDenial>();
        }
    }
}

fn apply_access_changes(
    mut commands: Commands,
    status: Res<AccessStatus>,
    mut rooms: Query<(Entity, &RoomAccessScope, &mut RoomRecovery), With<PublishedDenial>>,
) {
    if !status.is_changed() {
        return;
    }
    for (entity, scope, mut recovery) in &mut rooms {
        if recovery.failure == Some(FailureKind::Denied) && !status.is_denied(&scope.0) {
            recovery.reset();
            commands.entity(entity).try_remove::<PublishedDenial>();
        }
    }
}

#[derive(Deserialize)]
struct AdapterResponse {
    #[serde(default)]
    adapter: Option<String>,
    #[serde(default, rename = "fixedAdapter")]
    fixed_adapter: Option<String>,
}

async fn request_adapter(
    request: CredentialRequest,
    wallet: Wallet,
    client: reqwest::Client,
) -> Result<String, AdmissionError> {
    let invalid = || AdmissionError::new(FailureKind::Invalid);
    let transient = |_| AdmissionError::new(FailureKind::Transient);
    let url = validate_url(&request.url).map_err(|_| invalid())?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(invalid());
    }
    let headers = sign_service_request(request.method, &request.url, &wallet, request.metadata)
        .await
        .map_err(transient)?;
    let mut builder = client
        .request(request.method.parse().map_err(|_| invalid())?, url)
        .timeout(std::time::Duration::from_secs(10))
        .header("Content-Type", "application/json");
    for (key, value) in headers {
        builder = builder.header(key, value);
    }
    let response = platform::TracedJsonResponse::send(builder)
        .await
        .map_err(|_| AdmissionError::new(FailureKind::Transient))?;
    if !response.status().is_success() {
        let retry_after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse().ok());
        let mut error = AdmissionError::status(response.status().as_u16(), retry_after);
        error.message = response
            .json::<serde_json::Value>()
            .await
            .ok()
            .and_then(|body| body.get("error")?.as_str().map(str::to_owned));
        return Err(error);
    }
    let response = response
        .json::<AdapterResponse>()
        .await
        .map_err(|_| invalid())?;
    response
        .adapter
        .filter(|value| !value.is_empty())
        .or(response.fixed_adapter)
        .ok_or_else(invalid)
}

fn request_credentials(
    mut commands: Commands,
    requests: Query<
        (Entity, &CredentialRequest, &RoomRecovery),
        (
            With<NeedsAdapter>,
            Without<CredentialTask>,
            Without<RoomSuspended>,
        ),
    >,
    wallet: Res<Wallet>,
    ipfs: IpfsAssetServer,
    time: Res<Time>,
) {
    let Some(identity) = wallet.address() else {
        return;
    };
    for (entity, request, recovery) in &requests {
        if recovery.can_retry(time.elapsed_secs_f64()) {
            commands.entity(entity).try_insert(CredentialTask {
                identity,
                task: IoTaskPool::get().spawn_compat(request_adapter(
                    request.clone(),
                    wallet.snapshot(),
                    ipfs.ipfs().client(),
                )),
            });
        }
    }
}

pub(crate) fn finish_credentials(
    mut commands: Commands,
    mut manager: AdapterManager,
    mut requests: Query<(
        Entity,
        &mut CredentialTask,
        &mut RoomRecovery,
        Option<&RoomContext>,
        Option<&crate::SceneRoomTarget>,
        Has<RealmAdapter>,
        Has<RoomSuspended>,
    )>,
    time: Res<Time>,
    wallet: Res<Wallet>,
    mut admission: ResMut<crate::SceneAdmission>,
) {
    for (entity, mut task, mut recovery, context, scene, realm, suspended) in &mut requests {
        if suspended || wallet.address() != Some(task.identity) {
            commands.entity(entity).try_remove::<CredentialTask>();
            continue;
        }
        let Some(result) = task.task.complete() else {
            continue;
        };
        commands.entity(entity).try_remove::<CredentialTask>();
        match result.and_then(|adapter| {
            let (kind, _) =
                parse_adapter(&adapter).map_err(|_| AdmissionError::new(FailureKind::Invalid))?;
            if kind == AdapterKind::SignedLogin || (!realm && kind != AdapterKind::Livekit) {
                return Err(AdmissionError::new(FailureKind::Invalid));
            }
            Ok(adapter)
        }) {
            Ok(adapter) => {
                if let Some(scene) = scene {
                    admission.set_if_neq(crate::SceneAdmission::allowed(&scene.scene.scene_id));
                }
                // Admission alone does not establish a stable transport connection.
                recovery.failure = None;
                commands.entity(entity).try_remove::<NeedsAdapter>();
                manager.connect_existing(&adapter, context.map(|context| context.0), entity);
            }
            Err(error) => {
                if let Some(scene) = scene {
                    admission.set_if_neq(error.status.map_or_else(
                        crate::SceneAdmission::unknown,
                        |status| {
                            crate::SceneAdmission::from_refusal(
                                status,
                                error.message.as_deref(),
                                scene.is_world,
                                &scene.scene.scene_id,
                            )
                        },
                    ));
                }
                warn!(?entity, kind = ?error.kind, "room admission failed");
                recovery.fail(time.elapsed_secs_f64(), error);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn gatekeeper_http_refusal_retains_ban_evidence_and_retry_after() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let mut chunk = [0; 1024];
            while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                let count = socket.read(&mut chunk).unwrap();
                assert!(count > 0);
                request.extend_from_slice(&chunk[..count]);
            }
            assert!(request.starts_with(b"POST /adapter HTTP/1.1\r\n"));
            let body = r#"{"error":"User is banned from this scene"}"#;
            write!(socket, "HTTP/1.1 403 Forbidden\r\nContent-Type: application/json\r\nRetry-After: 30\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        });
        let mut wallet = Wallet::default();
        wallet.finalize_as_guest_with_seed([7; 32]);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let error = runtime
            .block_on(request_adapter(
                CredentialRequest {
                    method: "POST",
                    url: format!("http://{address}/adapter"),
                    metadata: "{}".into(),
                },
                wallet,
                reqwest::Client::new(),
            ))
            .unwrap_err();
        server.join().unwrap();
        assert_eq!(error.kind, FailureKind::Denied);
        assert_eq!(error.status, Some(403));
        assert_eq!(
            error.message.as_deref(),
            Some("User is banned from this scene")
        );
        assert_eq!(error.retry_after, Some(30.0));
    }

    #[test]
    fn only_continuous_healthy_rooms_reset_their_retry_budget() {
        let mut app = App::new();
        app.init_resource::<Time>()
            .add_systems(PostUpdate, observe_stable_connections);
        let health = ConnectionHealth::default();
        health.connected();
        let mut recovery = RoomRecovery::default();
        recovery.fail(0.0, AdmissionError::new(FailureKind::Transient));
        recovery.failure = None;
        let room = app.world_mut().spawn((recovery, health.clone())).id();
        app.update();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(29));
        app.update();
        assert_eq!(
            app.world()
                .get::<RoomRecovery>(room)
                .unwrap()
                .retry
                .attempts,
            1
        );
        health.disconnected();
        app.update();
        health.connected();
        app.update();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(29));
        app.update();
        assert_eq!(
            app.world()
                .get::<RoomRecovery>(room)
                .unwrap()
                .retry
                .attempts,
            1
        );
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(1));
        app.update();
        assert_eq!(
            app.world()
                .get::<RoomRecovery>(room)
                .unwrap()
                .retry
                .attempts,
            0
        );
    }

    #[test]
    fn connection_health_never_resets_denied_or_suspended_rooms() {
        let mut app = App::new();
        app.init_resource::<Time>()
            .add_systems(PostUpdate, observe_stable_connections);
        let health = ConnectionHealth::default();
        health.connected();
        let mut denied = RoomRecovery::default();
        denied.fail(0.0, AdmissionError::new(FailureKind::Denied));
        let room = app.world_mut().spawn((denied, health.clone())).id();
        let mut suspended = RoomRecovery::default();
        suspended.fail(0.0, AdmissionError::new(FailureKind::Transient));
        suspended.failure = None;
        let paused = app
            .world_mut()
            .spawn((suspended, health, RoomSuspended))
            .id();
        app.update();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(31));
        app.update();
        let denied = app.world().get::<RoomRecovery>(room).unwrap();
        assert_eq!(denied.failure, Some(FailureKind::Denied));
        assert!(!denied.can_retry(100.0));
        assert_eq!(denied.retry.attempts, 1);
        assert_eq!(
            app.world()
                .get::<RoomRecovery>(paused)
                .unwrap()
                .retry
                .attempts,
            1
        );
    }

    #[test]
    fn cancelled_socket_lifetime_cannot_leave_healthy_state_behind() {
        let health = ConnectionHealth::default();
        let lifetime = health.reset_on_drop();
        health.connected();
        assert!(health.is_connected());
        drop(lifetime);
        assert!(!health.is_connected());
    }

    #[cfg(feature = "livekit")]
    fn completion_app() -> App {
        let mut app = App::new();
        let mut wallet = Wallet::default();
        wallet.finalize_as_guest_with_seed([7; 32]);
        app.insert_resource(wallet)
            .init_resource::<Time>()
            .init_resource::<AccessStatus>()
            .init_resource::<crate::SceneAdmission>()
            .add_event::<crate::websocket_room::StartWsRoom>()
            .add_event::<crate::signed_login::StartSignedLogin>()
            .add_event::<crate::archipelago::StartArchipelago>()
            .add_systems(Update, (finish_credentials, publish_access).chain());
        #[cfg(feature = "livekit")]
        app.add_event::<crate::livekit::StartLivekit>();
        app
    }

    #[cfg(feature = "livekit")]
    fn completed(app: &App, result: Result<String, AdmissionError>) -> CredentialTask {
        let task = IoTaskPool::get_or_init(bevy::tasks::TaskPool::new).spawn(async move { result });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !task.is_finished() && std::time::Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert!(task.is_finished());
        CredentialTask {
            task,
            identity: app.world().resource::<Wallet>().address().unwrap(),
        }
    }

    #[cfg(feature = "livekit")]
    #[test]
    fn failed_mint_then_success_keeps_the_entity_and_resets_only_after_connection() {
        let mut app = completion_app();
        let task = completed(&app, Err(AdmissionError::status(503, Some(15.0))));
        let entity = app
            .world_mut()
            .spawn((RoomRecovery::default(), NeedsAdapter, task))
            .id();
        app.update();
        let recovery = app.world().get::<RoomRecovery>(entity).unwrap();
        assert_eq!(recovery.failure, Some(FailureKind::Transient));
        assert_eq!(recovery.retry.due, 15.0);
        assert!(app.world().get::<NeedsAdapter>(entity).is_some());
        let task = completed(
            &app,
            Ok("livekit:wss://rooms.example/?access_token=fresh".to_owned()),
        );
        app.world_mut().entity_mut(entity).insert(task);
        app.update();
        assert!(app.world().get::<NeedsAdapter>(entity).is_none());
        let recovery = app.world().get::<RoomRecovery>(entity).unwrap();
        assert_eq!(recovery.failure, None);
        assert_eq!(recovery.retry.attempts, 1);
        let events = app
            .world()
            .resource::<Events<crate::livekit::StartLivekit>>();
        let event = events.iter_current_update_events().next().unwrap();
        assert_eq!(event.entity, entity);
        assert_eq!(event.context, None);
    }

    #[cfg(feature = "livekit")]
    #[test]
    fn scene_credential_results_share_recovery_and_moderation_evidence() {
        for (status, message, world, scope) in [
            (
                403,
                "Access denied, platform-banned user",
                false,
                Some(crate::BanScope::Platform),
            ),
            (
                403,
                "User is banned from this scene",
                false,
                Some(crate::BanScope::Scene),
            ),
            (
                401,
                "You are banned from world example",
                true,
                Some(crate::BanScope::World),
            ),
            (401, "You are banned from world example", false, None),
            (403, "Forbidden", false, None),
            (503, "Access denied, platform-banned user", false, None),
        ] {
            let mut app = completion_app();
            let mut error = AdmissionError::status(status, Some(15.0));
            error.message = Some(message.to_owned());
            let task = completed(&app, Err(error));
            let entity = app
                .world_mut()
                .spawn((
                    RoomRecovery::default(),
                    NeedsAdapter,
                    task,
                    crate::SceneRoomTarget {
                        scene: crate::SetCurrentScene {
                            realm_name: "realm".into(),
                            scene_id: "scene".into(),
                        },
                        is_world: world,
                    },
                    RoomAccessScope(AccessScope::Scene("scene".into())),
                ))
                .id();
            app.update();
            let admission = app.world().resource::<crate::SceneAdmission>();
            match scope {
                Some(scope) => assert!(
                    matches!(admission.view(), crate::AdmissionView::Refused { scope: actual, scene_id: "scene", .. } if actual == scope)
                ),
                None => assert_eq!(admission.view(), crate::AdmissionView::Unknown),
            }
            let recovery = app.world().get::<RoomRecovery>(entity).unwrap();
            assert_eq!(
                recovery.failure,
                Some(if status == 503 {
                    FailureKind::Transient
                } else {
                    FailureKind::Denied
                })
            );
            assert_eq!(
                app.world()
                    .resource::<AccessStatus>()
                    .is_denied(&AccessScope::Scene("scene".into())),
                status != 503
            );
            if status != 503 {
                assert!(!recovery.can_retry(100.0));
            }

            let task = completed(
                &app,
                Ok("livekit:wss://rooms.example/?access_token=fresh".into()),
            );
            app.world_mut().entity_mut(entity).insert(task);
            app.update();
            assert_eq!(
                app.world().resource::<crate::SceneAdmission>().view(),
                crate::AdmissionView::Allowed { scene_id: "scene" }
            );
            assert!(!app
                .world()
                .resource::<AccessStatus>()
                .is_denied(&AccessScope::Scene("scene".into())));
        }
    }

    #[cfg(feature = "livekit")]
    #[test]
    fn unrelated_and_cancelled_credentials_cannot_publish_scene_admission() {
        for cancellation in ["other_room", "paused", "identity", "despawned"] {
            let mut app = completion_app();
            app.insert_resource(crate::SceneAdmission::allowed("current"));
            let mut error = AdmissionError::status(403, None);
            error.message = Some("Access denied, platform-banned user".into());
            let task = completed(&app, Err(error));
            let entity = app
                .world_mut()
                .spawn((RoomRecovery::default(), NeedsAdapter, task))
                .id();
            if cancellation != "other_room" {
                app.world_mut()
                    .entity_mut(entity)
                    .insert(crate::SceneRoomTarget {
                        scene: crate::SetCurrentScene {
                            realm_name: "realm".into(),
                            scene_id: "old".into(),
                        },
                        is_world: false,
                    });
            }
            match cancellation {
                "paused" => {
                    app.world_mut().entity_mut(entity).insert(RoomSuspended);
                }
                "identity" => {
                    app.world_mut()
                        .resource_mut::<Wallet>()
                        .finalize_as_guest_with_seed([9; 32]);
                }
                "despawned" => {
                    app.world_mut().despawn(entity);
                }
                _ => {}
            }
            app.update();
            assert_eq!(
                app.world().resource::<crate::SceneAdmission>().view(),
                crate::AdmissionView::Allowed {
                    scene_id: "current"
                },
                "{cancellation}"
            );
        }
    }

    #[cfg(feature = "livekit")]
    #[test]
    fn replacing_scene_ownership_clears_admission_before_old_results_finish() {
        for replacement in [
            "wallet",
            "same_wallet",
            "owner_gone",
            "realm",
            "scene",
            "leave",
        ] {
            let mut app = completion_app();
            app.init_resource::<common::structs::AppConfig>()
                .init_resource::<common::structs::CurrentRealm>()
                .init_resource::<common::structs::PreviewMode>()
                .init_resource::<ipfs::world_access::WorldCommsSecret>()
                .init_resource::<crate::SceneRoomConnection>()
                .init_resource::<crate::SceneRoomAdapterOverride>()
                .init_resource::<crate::DisableSceneRoomGatekeeper>()
                .add_event::<crate::SetCurrentScene>()
                .add_systems(Update, crate::connect_scene_room.before(finish_credentials));
            app.world_mut()
                .spawn(crate::global_crdt::GlobalCrdtState::new(None));
            app.world_mut().send_event(crate::SetCurrentScene {
                realm_name: "realm".into(),
                scene_id: "old".into(),
            });
            app.update();
            let old = app
                .world_mut()
                .query_filtered::<Entity, With<crate::SceneRoomTarget>>()
                .single(app.world())
                .unwrap();
            let mut error = AdmissionError::status(403, None);
            error.message = Some("Access denied, platform-banned user".into());
            let task = completed(&app, Err(error));
            app.world_mut().entity_mut(old).insert(task);
            app.insert_resource(crate::SceneAdmission::from_refusal(
                403,
                Some("Access denied, platform-banned user"),
                false,
                "old",
            ));
            match replacement {
                "owner_gone" => {
                    app.world_mut().despawn(old);
                    app.world_mut()
                        .resource_mut::<Wallet>()
                        .finalize_as_guest_with_seed([9; 32]);
                }
                "wallet" => app
                    .world_mut()
                    .resource_mut::<Wallet>()
                    .finalize_as_guest_with_seed([9; 32]),
                "same_wallet" => app
                    .world_mut()
                    .resource_mut::<Wallet>()
                    .finalize_as_guest_with_seed([7; 32]),
                "realm" => {
                    app.world_mut()
                        .resource_mut::<common::structs::CurrentRealm>()
                        .about_url = "https://new.example/about".into()
                }
                _ => {
                    app.world_mut().send_event(crate::SetCurrentScene {
                        realm_name: "realm".into(),
                        scene_id: if replacement == "leave" { "" } else { "new" }.into(),
                    });
                }
            }
            app.update();
            assert!(app.world().get_entity(old).is_err(), "{replacement}");
            assert_eq!(
                app.world().resource::<crate::SceneAdmission>().view(),
                crate::AdmissionView::Unknown,
                "{replacement}"
            );
            assert!(app
                .world()
                .resource::<Events<crate::livekit::StartLivekit>>()
                .is_empty());
        }
    }

    #[cfg(feature = "livekit")]
    #[test]
    fn signed_login_result_cannot_start_another_signed_login() {
        let mut app = completion_app();
        let context = app.world_mut().spawn_empty().id();
        let task = completed(
            &app,
            Ok("signed-login:https://rooms.example/login".to_owned()),
        );
        let entity = app
            .world_mut()
            .spawn((
                RealmAdapter,
                RoomContext(context),
                RoomRecovery::default(),
                NeedsAdapter,
                task,
            ))
            .id();
        app.update();

        let recovery = app.world().get::<RoomRecovery>(entity).unwrap();
        assert_eq!(recovery.failure, Some(FailureKind::Invalid));
        assert!(!recovery.can_retry(60.0));
        assert!(app
            .world()
            .resource::<Events<crate::signed_login::StartSignedLogin>>()
            .is_empty());
    }

    #[cfg(feature = "livekit")]
    #[test]
    fn paused_old_identity_and_despawned_completions_cannot_create_rooms() {
        for cancellation in ["paused", "identity", "despawned"] {
            let mut app = completion_app();
            let task = completed(
                &app,
                Ok("livekit:wss://rooms.example/?access_token=old".to_owned()),
            );
            let entity = app
                .world_mut()
                .spawn((RoomRecovery::default(), NeedsAdapter, task))
                .id();
            match cancellation {
                "paused" => {
                    app.world_mut().entity_mut(entity).insert(RoomSuspended);
                }
                "identity" => {
                    app.world_mut()
                        .resource_mut::<Wallet>()
                        .finalize_as_guest_with_seed([9; 32]);
                }
                _ => {
                    app.world_mut().despawn(entity);
                }
            }
            app.update();
            assert!(
                app.world()
                    .resource::<Events<crate::livekit::StartLivekit>>()
                    .is_empty(),
                "{cancellation}"
            );
            assert!(app.world().get::<CredentialTask>(entity).is_none());
        }
    }

    #[test]
    fn denial_and_explicit_access_change_affect_only_the_named_room() {
        let mut app = App::new();
        app.init_resource::<AccessStatus>()
            .add_systems(Update, (apply_access_changes, publish_access).chain());
        let scene_scope = AccessScope::Scene("scene".into());
        let chat_scope = AccessScope::PrivateChat;
        let denial = RoomRecovery {
            failure: Some(FailureKind::Denied),
            ..Default::default()
        };
        let scene = app
            .world_mut()
            .spawn((RoomAccessScope(scene_scope.clone()), denial.clone()))
            .id();
        let chat = app
            .world_mut()
            .spawn((RoomAccessScope(chat_scope.clone()), denial))
            .id();
        app.update();
        assert!(app
            .world()
            .resource::<AccessStatus>()
            .is_denied(&scene_scope));
        app.world_mut()
            .resource_mut::<AccessStatus>()
            .remove_scope(&scene_scope);
        app.update();
        assert_eq!(
            app.world().get::<RoomRecovery>(scene).unwrap().failure,
            None
        );
        assert_eq!(
            app.world().get::<RoomRecovery>(chat).unwrap().failure,
            Some(FailureKind::Denied)
        );
        app.world_mut().despawn(chat);
        assert!(!app
            .world()
            .resource::<AccessStatus>()
            .is_denied(&chat_scope));
    }

    #[test]
    fn denial_is_terminal_but_server_errors_retry() {
        assert!(AdmissionError::status(401, None).terminal());
        assert!(AdmissionError::status(403, None).terminal());
        assert!(!AdmissionError::status(503, Some(15.0)).terminal());
        assert_eq!(
            AdmissionError::status(429, Some(15.0)).retry_after,
            Some(15.0)
        );
    }

    #[test]
    fn a_failure_changes_only_its_room_and_respects_retry_after() {
        let mut failed = RoomRecovery::default();
        let healthy = RoomRecovery::default();
        failed.fail(10.0, AdmissionError::status(503, Some(15.0)));
        assert!(!failed.retry.ready(24.0));
        assert!(failed.retry.ready(25.0));
        assert_eq!(healthy.failure, None);
        failed.fail(25.0, AdmissionError::status(403, None));
        assert_eq!(failed.failure, Some(FailureKind::Denied));
        assert!(!failed.can_retry(1000.0));
        failed.reset();
        assert!(failed.can_retry(1000.0));
    }
}
