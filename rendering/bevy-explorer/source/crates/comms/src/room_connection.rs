use bevy::{
    prelude::*,
    tasks::{IoTaskPool, Task},
};
use common::access_status::{AccessScope, AccessStatus};
use common::util::{TaskCompat, TaskExt};
use ipfs::IpfsAssetServer;
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use wallet::{sign_service_request, Wallet};

use crate::{
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

#[derive(Clone, Copy, Debug)]
pub struct AdmissionError {
    pub kind: FailureKind,
    pub retry_after: Option<f64>,
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
pub struct RoomSuspended;

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
struct CredentialTask {
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
            status.set_allowed(&scope.0);
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
        return Err(AdmissionError::status(
            response.discard().as_u16(),
            retry_after,
        ));
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

fn finish_credentials(
    mut commands: Commands,
    mut manager: AdapterManager,
    mut requests: Query<(
        Entity,
        &mut CredentialTask,
        &mut RoomRecovery,
        Option<&RoomContext>,
        Has<RealmAdapter>,
        Has<RoomSuspended>,
    )>,
    time: Res<Time>,
    wallet: Res<Wallet>,
) {
    for (entity, mut task, mut recovery, context, realm, suspended) in &mut requests {
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
            if !realm && kind != AdapterKind::Livekit {
                return Err(AdmissionError::new(FailureKind::Invalid));
            }
            Ok(adapter)
        }) {
            Ok(adapter) => {
                // Keep failure count until the transport connects, so repeated expired tokens back off.
                recovery.failure = None;
                commands.entity(entity).try_remove::<NeedsAdapter>();
                manager.connect_existing(&adapter, context.map(|context| context.0), entity);
            }
            Err(error) => {
                recovery.fail(time.elapsed_secs_f64(), error);
                warn!(?entity, kind = ?error.kind, "room admission failed");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            .add_event::<crate::websocket_room::StartWsRoom>()
            .add_event::<crate::signed_login::StartSignedLogin>()
            .add_event::<crate::archipelago::StartArchipelago>()
            .add_systems(Update, finish_credentials);
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
            .set_allowed(&scene_scope);
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
