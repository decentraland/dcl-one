//! Presentation sessions, in memory as upstream keeps them
//! (`src/logic/presentation-manager/component.ts`): create (connect first,
//! then render and publish), the data-channel commands, the state broadcast,
//! stop, and the idle reaper (every 60 s, a session with no remote
//! participant for 5 minutes stops).
//!
//! The bot token is checked here before anything else: signed by this
//! node's LiveKit key, unexpired, a `presentation-bot:` identity in the room
//! its grant names, allowed to publish a screen share
//! ([`LiveKitTarget::check_bot_token`]). Any other token, a player's scene
//! token included, is refused before a slot is taken or a bot started.
//!
//! Every bot this node runs holds a slot of `MAX_CONCURRENT_PRESENTATIONS`:
//! a session, a creation in progress, or the JSON path's credential check
//! and download. A creation runs in its own task, so a client that goes
//! away mid-request leaves either a running session or nothing. Multipart
//! uploads being read hold an upload permit, as many as there are slots.
//!
//! The PDF load and each render run on [`RENDER_THREADS`] blocking threads
//! at most. A render waits up to [`RENDER_WAIT`] for a thread; with none
//! free it is busy: a creation answers 503, a slide change is dropped and the
//! session stays on its slide. Once it holds a thread, the render has
//! [`RENDER_TIMEOUT`]. hayro cannot be interrupted, so a timed-out render
//! keeps its thread until it returns, and that thread stays one of the
//! [`RENDER_THREADS`]: the creation or that render's session fails at once
//! and releases its slot, while stuck renders take render capacity, never
//! more of tokio's shared blocking pool.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc, watch};

use super::bot::{BotLauncher, BotLink, ConnectError, RoomData, RoomState};
use super::render::{Frame, PdfDocument};
use super::wire::{
    self, Command, DEFAULT_OVERLAY, FileType, Navigate, Overlay, PresentationInfo,
    PresentationState, SlideVideo, VideoState,
};

pub const IDLE_TIMEOUT: Duration = Duration::from_secs(5 * 60);
pub const IDLE_CHECK_INTERVAL: Duration = Duration::from_secs(60);
const OVERLAY_BROADCAST_INTERVAL: Duration = Duration::from_millis(250);
/// The PDF load, and each slide render, from when it holds a thread.
pub const RENDER_TIMEOUT: Duration = Duration::from_secs(30);
/// How long a load or render waits for one of the [`RENDER_THREADS`].
pub const RENDER_WAIT: Duration = Duration::from_secs(10);
/// The blocking threads PDF loads and renders take at once, timed-out ones
/// included.
pub const RENDER_THREADS: usize = 4;
/// `MAX_CONCURRENT_PRESENTATIONS` when unset.
pub const DEFAULT_MAX_CONCURRENT: usize = 10;
/// The identity prefix of the gatekeeper's presentation-bot tokens
/// (`presentation-bot:<room>:<access id>:<uuid>`).
pub const BOT_IDENTITY_PREFIX: &str = "presentation-bot:";
/// The track source the bot publishes, as LiveKit names it in a grant.
const SCREEN_SHARE_SOURCE: &str = "screen_share";

/// Why a presentation was not created.
#[derive(Debug, thiserror::Error)]
pub enum CreateError {
    /// 429.
    #[error("Maximum concurrent presentations reached ({0})")]
    MaxConcurrent(usize),
    /// 422: a slide failed to render.
    #[error("{0}")]
    SlideRender(String),
    /// 500: the LiveKit connect failed (upstream does not tell a refused
    /// token from any other failure on this path), the PDF did not load,
    /// or publishing failed.
    #[error("{0}")]
    Internal(String),
    /// 401: the token is not a presentation-bot token of this node's
    /// LiveKit, or (the JSON path's credential check) LiveKit refused it.
    #[error("{0}")]
    InvalidCredentials(String),
    /// 503, the credential check only: the bot did not start or LiveKit did
    /// not answer.
    #[error("{0}")]
    Unavailable(String),
    /// 503: no render thread came free within [`RENDER_WAIT`].
    #[error("No render thread available")]
    Busy,
}

/// The node's own LiveKit: the only server a presentation joins, and the
/// key its bot tokens must be signed with.
#[derive(Clone, PartialEq, Eq)]
pub struct LiveKitTarget {
    /// The client-facing signaling URL (`LIVEKIT_WS_URL`, else `LIVEKIT_HOST`),
    /// the one the gatekeeper's presentation-bot token answer names: a
    /// request's `livekitUrl` must be it.
    pub client_url: String,
    /// The URL the bot dials: the node's own route to LiveKit (`LIVEKIT_API_HOST`,
    /// else `LIVEKIT_HOST`), never the request's.
    pub connect_url: String,
    /// The key the gatekeeper mints bot tokens with.
    pub api_key: String,
    pub api_secret: String,
}

impl std::fmt::Debug for LiveKitTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiveKitTarget")
            .field("client_url", &self.client_url)
            .field("connect_url", &self.connect_url)
            .field("api_key", &self.api_key)
            .field("api_secret", &"<set>")
            .finish()
    }
}

impl LiveKitTarget {
    /// From the gatekeeper's LiveKit; `None` without one.
    pub fn from_livekit(livekit: &crate::shared::livekit::LiveKit) -> Option<Self> {
        if livekit.client_url.is_empty() {
            return None;
        }
        let connect_url = if livekit.api_url.is_empty() {
            livekit.client_url.clone()
        } else {
            crate::shared::livekit::client_ws_url(&livekit.api_url)
        };
        Some(Self {
            client_url: livekit.client_url.clone(),
            connect_url,
            api_key: livekit.api_key.clone(),
            api_secret: livekit.api_secret.clone(),
        })
    }

    /// Whether `token` is a presentation-bot token of this LiveKit at `now`
    /// (unix seconds), or why not: signed with its key and valid then (as
    /// LiveKit checks it), a `presentation-bot:<room>:` identity whose room
    /// is the one its grant joins, and allowed to publish a screen share.
    pub fn check_bot_token(&self, token: &str, now: u64) -> Result<(), &'static str> {
        let claims = crate::shared::livekit::verify_access_token(
            &self.api_key,
            &self.api_secret,
            token,
            now,
        )
        .ok_or("not a valid token of this node's LiveKit")?;
        let video = claims
            .get("video")
            .and_then(serde_json::Value::as_object)
            .ok_or("no video grant")?;
        let room = video
            .get("room")
            .and_then(serde_json::Value::as_str)
            .filter(|room| !room.is_empty())
            .ok_or("no room in the grant")?;
        let identity = claims
            .get("sub")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let in_room = identity
            .strip_prefix(BOT_IDENTITY_PREFIX)
            .and_then(|rest| rest.strip_prefix(room))
            .is_some_and(|rest| rest.starts_with(':'));
        if !in_room {
            return Err("not a presentation-bot identity of the granted room");
        }
        let flag = |name: &str| video.get(name).and_then(serde_json::Value::as_bool);
        if flag("roomJoin") != Some(true) {
            return Err("the grant does not join the room");
        }
        // LiveKit reads an absent `canPublish` as allowed, and an absent or
        // empty `canPublishSources` as every source.
        let sources = video
            .get("canPublishSources")
            .and_then(serde_json::Value::as_array);
        let screen_share = sources.is_none_or(|sources| {
            sources.is_empty()
                || sources
                    .iter()
                    .any(|source| source.as_str() == Some(SCREEN_SHARE_SOURCE))
        });
        if flag("canPublish") == Some(false) || !screen_share {
            return Err("the grant cannot publish a screen share");
        }
        Ok(())
    }

    /// Whether `livekit_url` names this node's LiveKit (trailing `/` aside).
    pub fn is_own(&self, livekit_url: &str) -> bool {
        let canonical = |raw: &str| {
            url::Url::parse(raw)
                .ok()
                .map(|u| u.as_str().trim_end_matches('/').to_string())
        };
        canonical(livekit_url).is_some_and(|url| Some(url) == canonical(&self.client_url))
    }
}

struct SessionState {
    current_slide: usize,
    slide_videos: Vec<SlideVideo>,
    overlay: Overlay,
    last_frame: Frame,
    /// The scene id of the last presenter command; packets the bot sends carry it.
    scene_id: String,
    presenters: HashSet<String>,
}

struct Session {
    id: String,
    file_name: String,
    file_type: FileType,
    slide_count: usize,
    doc: Arc<PdfDocument>,
    renders: Arc<RenderPool>,
    bot: Arc<dyn BotLink>,
    state: tokio::sync::Mutex<SessionState>,
    /// The room's metadata and participant count, as the bot reports them.
    room: watch::Receiver<RoomState>,
    /// When the reaper last saw a participant.
    last_active: Mutex<Instant>,
}

/// The sessions of this node.
pub struct Presenter {
    launcher: Arc<dyn BotLauncher>,
    livekit: LiveKitTarget,
    /// `None` is unlimited (`MAX_CONCURRENT_PRESENTATIONS=0`).
    max_concurrent: Option<usize>,
    sessions: Mutex<HashMap<String, Arc<Session>>>,
    /// Slots held by creations and credential checks.
    in_flight: AtomicUsize,
    /// Multipart uploads being read; `None` when unlimited.
    uploads: Option<Arc<Semaphore>>,
    renders: Arc<RenderPool>,
}

/// A slot of the concurrency limit, released on drop unless a session took it.
pub struct Slot {
    presenter: Arc<Presenter>,
    held: bool,
}

impl Slot {
    /// The session takes the slot: in the map and out of `in_flight` at once.
    fn admit(mut self, session: Arc<Session>) {
        let mut sessions = self.presenter.sessions.lock().unwrap();
        sessions.insert(session.id.clone(), session);
        self.presenter.in_flight.fetch_sub(1, Ordering::SeqCst);
        self.held = false;
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        if self.held {
            self.presenter.in_flight.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

/// Why a render did not give a slide.
pub(super) enum RenderError {
    Failed(String),
    /// The render held a thread for [`RENDER_TIMEOUT`] without a result.
    TimedOut,
    /// No thread came free within [`RENDER_WAIT`]; nothing ran.
    Busy,
}

impl RenderError {
    fn message(self, index: usize) -> String {
        match self {
            RenderError::Failed(message) => message,
            RenderError::TimedOut => format!(
                "Failed to render slide {index}: no result in {}s",
                RENDER_TIMEOUT.as_secs()
            ),
            RenderError::Busy => "No render thread available".to_string(),
        }
    }
}

/// The blocking threads PDF work runs on: at most `threads` at once, each
/// held until its work returns, whether or not anyone still waits for it.
pub(super) struct RenderPool {
    threads: Arc<Semaphore>,
    wait: Duration,
    timeout: Duration,
}

impl RenderPool {
    pub(super) fn new(threads: usize, wait: Duration, timeout: Duration) -> Self {
        Self {
            threads: Arc::new(Semaphore::new(threads)),
            wait,
            timeout,
        }
    }

    /// `work` on one of the pool's threads: [`RenderError::Busy`] when no
    /// thread came free within `wait`, [`RenderError::TimedOut`] when the
    /// work gave no result within `timeout` of starting.
    pub(super) async fn run<T: Send + 'static>(
        &self,
        work: impl FnOnce() -> T + Send + 'static,
    ) -> Result<T, RenderError> {
        let Ok(thread) =
            tokio::time::timeout(self.wait, self.threads.clone().acquire_owned()).await
        else {
            return Err(RenderError::Busy);
        };
        let thread = thread.expect("never closed");
        let running = tokio::task::spawn_blocking(move || {
            let _thread = thread;
            work()
        });
        match tokio::time::timeout(self.timeout, running).await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(e)) => Err(RenderError::Failed(e.to_string())),
            Err(_) => Err(RenderError::TimedOut),
        }
    }

    async fn render(
        &self,
        doc: &Arc<PdfDocument>,
        index: usize,
    ) -> Result<(Frame, Vec<SlideVideo>), RenderError> {
        let doc = doc.clone();
        match self.run(move || doc.render(index)).await {
            Ok(result) => result.map_err(RenderError::Failed),
            Err(RenderError::Failed(e)) => Err(RenderError::Failed(format!(
                "Failed to render slide {index}: {e}"
            ))),
            Err(e) => Err(e),
        }
    }

    async fn load(&self, file: Vec<u8>) -> Result<PdfDocument, CreateError> {
        match self.run(move || PdfDocument::load(file)).await {
            Ok(result) => result.map_err(CreateError::Internal),
            Err(RenderError::Failed(e)) => Err(CreateError::Internal(e)),
            Err(RenderError::TimedOut) => Err(CreateError::Internal(format!(
                "PDF did not load in {}s",
                self.timeout.as_secs()
            ))),
            Err(RenderError::Busy) => Err(CreateError::Busy),
        }
    }
}

impl Presenter {
    /// `max_concurrent` as configured: 0 is unlimited.
    pub fn new(
        launcher: Arc<dyn BotLauncher>,
        livekit: LiveKitTarget,
        max_concurrent: usize,
    ) -> Arc<Self> {
        let max_concurrent = (max_concurrent > 0).then_some(max_concurrent);
        Arc::new(Self {
            launcher,
            livekit,
            max_concurrent,
            sessions: Mutex::new(HashMap::new()),
            in_flight: AtomicUsize::new(0),
            uploads: max_concurrent.map(|max| Arc::new(Semaphore::new(max))),
            renders: Arc::new(RenderPool::new(RENDER_THREADS, RENDER_WAIT, RENDER_TIMEOUT)),
        })
    }

    /// Every render thread, held until the permit drops.
    #[cfg(test)]
    pub(super) async fn hold_render_threads(&self) -> OwnedSemaphorePermit {
        self.renders
            .threads
            .clone()
            .acquire_many_owned(RENDER_THREADS as u32)
            .await
            .expect("never closed")
    }

    pub fn livekit(&self) -> &LiveKitTarget {
        &self.livekit
    }

    pub fn active_sessions(&self) -> usize {
        self.sessions.lock().unwrap().len()
    }

    /// Whether every slot is taken now (a hint: [`Self::create`] decides).
    pub fn at_capacity(&self) -> bool {
        let sessions = self.sessions.lock().unwrap();
        self.full(sessions.len())
    }

    /// The 429.
    pub fn capacity_error(&self) -> CreateError {
        CreateError::MaxConcurrent(self.max_concurrent.unwrap_or(0))
    }

    fn full(&self, sessions: usize) -> bool {
        self.max_concurrent
            .is_some_and(|max| sessions + self.in_flight.load(Ordering::SeqCst) >= max)
    }

    /// A permit to read a multipart upload, or 429 when as many uploads as
    /// there are slots are being read. Unlimited when the slots are.
    pub fn upload_permit(&self) -> Result<Option<OwnedSemaphorePermit>, CreateError> {
        match &self.uploads {
            None => Ok(None),
            Some(uploads) => uploads
                .clone()
                .try_acquire_owned()
                .map(Some)
                .map_err(|_| self.capacity_error()),
        }
    }

    /// 401 unless `token` is a presentation-bot token of this node's LiveKit
    /// ([`LiveKitTarget::check_bot_token`]).
    pub fn verify_token(&self, token: &str) -> Result<(), CreateError> {
        let now =
            crate::shared::livekit::unix_now().map_err(|e| CreateError::Internal(e.to_string()))?;
        self.livekit
            .check_bot_token(token, now)
            .map_err(|why| CreateError::InvalidCredentials(why.to_string()))
    }

    /// A slot, or 429 when every one is taken.
    pub fn reserve(self: &Arc<Self>) -> Result<Slot, CreateError> {
        let sessions = self.sessions.lock().unwrap();
        if self.full(sessions.len()) {
            return Err(self.capacity_error());
        }
        self.in_flight.fetch_add(1, Ordering::SeqCst);
        Ok(Slot {
            presenter: self.clone(),
            held: true,
        })
    }

    /// The JSON path's pre-flight, under the caller's slot: the token
    /// checked here, then a connect and disconnect, so a token LiveKit
    /// refuses costs no download.
    pub async fn validate_credentials(&self, _slot: &Slot, token: &str) -> Result<(), CreateError> {
        self.verify_token(token)?;
        match self
            .launcher
            .connect(&self.livekit.connect_url, token)
            .await
        {
            Ok(connected) => {
                connected.link.shutdown().await;
                Ok(())
            }
            Err(ConnectError::Refused(why)) => Err(CreateError::InvalidCredentials(why)),
            Err(ConnectError::Unavailable(why)) => Err(CreateError::Unavailable(why)),
        }
    }

    /// Creates a session from a PDF: the token, the concurrency limit, the
    /// LiveKit connect, the first slide published, the first state
    /// broadcast. The work runs in its own task: dropping this future does
    /// not stop it.
    pub async fn create(
        self: &Arc<Self>,
        file: Vec<u8>,
        livekit_token: &str,
        file_name: &str,
    ) -> Result<PresentationInfo, CreateError> {
        self.verify_token(livekit_token)?;
        let slot = self.reserve()?;
        self.create_reserved(slot, file, livekit_token, file_name)
            .await
    }

    /// [`Self::create`] in a slot the caller holds.
    pub async fn create_reserved(
        self: &Arc<Self>,
        slot: Slot,
        file: Vec<u8>,
        livekit_token: &str,
        file_name: &str,
    ) -> Result<PresentationInfo, CreateError> {
        self.verify_token(livekit_token)?;
        let presenter = self.clone();
        let (token, file_name) = (livekit_token.to_string(), file_name.to_string());
        crate::shared::task::spawn_with(async move {
            presenter.create_in(slot, file, &token, &file_name).await
        })
        .await
        .map_err(|e| CreateError::Internal(e.to_string()))?
    }

    async fn create_in(
        self: Arc<Self>,
        slot: Slot,
        file: Vec<u8>,
        livekit_token: &str,
        file_name: &str,
    ) -> Result<PresentationInfo, CreateError> {
        let id = uuid::Uuid::new_v4().to_string();
        let connected = self
            .launcher
            .connect(&self.livekit.connect_url, livekit_token)
            .await
            .map_err(|e| CreateError::Internal(e.to_string()))?;
        let bot = connected.link.clone();
        let session = match self.start(&id, connected.room, file, file_name, &bot).await {
            Ok(session) => session,
            Err(e) => {
                bot.shutdown().await;
                return Err(e);
            }
        };
        slot.admit(session.clone());
        tracing::info!(presentation = %id, slides = session.slide_count, "presentation created");
        session.broadcast_state().await;
        crate::shared::task::spawn(self.clone().run(session.clone(), connected.data));
        Ok(PresentationInfo {
            id,
            file_name: session.file_name.clone(),
            slide_count: session.slide_count,
            current_slide: 0,
            file_type: session.file_type,
        })
    }

    async fn start(
        &self,
        id: &str,
        room: watch::Receiver<RoomState>,
        file: Vec<u8>,
        file_name: &str,
        bot: &Arc<dyn BotLink>,
    ) -> Result<Arc<Session>, CreateError> {
        bot.update_metadata(wire::bot_metadata(id, None))
            .await
            .map_err(|e| CreateError::Internal(e.to_string()))?;
        let doc = Arc::new(self.renders.load(file).await?);
        let slide_count = doc.page_count();
        if slide_count == 0 {
            return Err(CreateError::Internal("PDF contains no pages".into()));
        }
        let (frame, slide_videos) = self.renders.render(&doc, 0).await.map_err(|e| match e {
            RenderError::Busy => CreateError::Busy,
            e => CreateError::SlideRender(e.message(0)),
        })?;
        bot.publish_video(frame.width, frame.height)
            .await
            .map_err(|e| CreateError::Internal(e.to_string()))?;
        bot.push_frame(&frame)
            .await
            .map_err(|e| CreateError::Internal(e.to_string()))?;
        let presenters = wire::presenters_from_room_metadata(&room.borrow().metadata);
        Ok(Arc::new(Session {
            id: id.to_string(),
            file_name: super::files::presentation_name(file_name),
            file_type: FileType::Pdf,
            slide_count,
            doc,
            renders: self.renders.clone(),
            bot: bot.clone(),
            state: tokio::sync::Mutex::new(SessionState {
                current_slide: 0,
                slide_videos,
                overlay: DEFAULT_OVERLAY,
                last_frame: frame,
                scene_id: String::new(),
                presenters,
            }),
            room,
            last_active: Mutex::new(Instant::now()),
        }))
    }

    /// One session's room, handled one packet at a time; a room change is
    /// read before the packets that follow it.
    async fn run(self: Arc<Self>, session: Arc<Session>, mut data: mpsc::Receiver<RoomData>) {
        let mut room = session.room.clone();
        // The presenters as of now: the metadata may have changed since
        // `start` read it, and marking it seen must not lose that change.
        let mut metadata = room.borrow_and_update().metadata.clone();
        session.state.lock().await.presenters = wire::presenters_from_room_metadata(&metadata);
        let mut overlay_due: Option<tokio::time::Instant> = None;
        loop {
            let packet = tokio::select! {
                biased;
                changed = room.changed() => {
                    if changed.is_err() {
                        tracing::warn!(presentation = %session.id, "bot gone");
                        self.stop(&session.id).await;
                        break;
                    }
                    let current = room.borrow_and_update().metadata.clone();
                    if current != metadata {
                        session.state.lock().await.presenters =
                            wire::presenters_from_room_metadata(&current);
                        metadata = current;
                    }
                    continue;
                }
                _ = tokio::time::sleep_until(overlay_due.unwrap_or_else(tokio::time::Instant::now)),
                    if overlay_due.is_some() =>
                {
                    overlay_due = None;
                    if !self.is_live(&session.id) {
                        break;
                    }
                    session.broadcast_state().await;
                    continue;
                }
                packet = data.recv() => packet,
            };
            let Some(RoomData { identity, payload }) = packet else {
                tracing::warn!(presentation = %session.id, "bot gone");
                self.stop(&session.id).await;
                break;
            };
            if !self.is_live(&session.id) {
                break;
            }
            let Some(command) = session.authorize(&identity, &payload).await else {
                continue;
            };
            match command {
                Command::Stop => {
                    self.stop(&session.id).await;
                    break;
                }
                Command::Overlay(patch) => {
                    patch.apply(&mut session.state.lock().await.overlay);
                    overlay_due.get_or_insert_with(|| {
                        tokio::time::Instant::now() + OVERLAY_BROADCAST_INTERVAL
                    });
                }
                other => {
                    if !session.handle(other).await {
                        self.stop(&session.id).await;
                        break;
                    }
                }
            }
        }
    }

    fn is_live(&self, id: &str) -> bool {
        self.sessions.lock().unwrap().contains_key(id)
    }

    /// Stops a session: removed first, then `presentation:stopped`, then the
    /// bot disconnects. Stopping a stopped session does nothing.
    pub async fn stop(&self, id: &str) {
        let Some(session) = self.sessions.lock().unwrap().remove(id) else {
            return;
        };
        let scene_id = session.state.lock().await.scene_id.clone();
        let packet = wire::encode_packet(&wire::stopped_message(id), &scene_id);
        if let Err(e) = session.bot.publish_data(packet).await {
            tracing::warn!(presentation = %id, "stop broadcast failed: {e}");
        }
        session.bot.shutdown().await;
        tracing::info!(presentation = %id, "presentation stopped");
    }

    /// One reaper pass at `now`: a session with participants is active; one
    /// without any for [`IDLE_TIMEOUT`] stops.
    pub async fn reap_idle(&self, now: Instant) {
        let sessions: Vec<Arc<Session>> = self.sessions.lock().unwrap().values().cloned().collect();
        for session in sessions {
            let idle = {
                let mut last_active = session.last_active.lock().unwrap();
                if session.room.borrow().participants > 0 {
                    *last_active = now;
                    false
                } else {
                    now.duration_since(*last_active) >= IDLE_TIMEOUT
                }
            };
            if idle {
                tracing::info!(presentation = %session.id, "idle with no participants, stopping");
                self.stop(&session.id).await;
            }
        }
    }

    /// The reaper, every [`IDLE_CHECK_INTERVAL`], for the life of the process.
    pub fn spawn_reaper(self: &Arc<Self>) {
        let presenter = Arc::downgrade(self);
        crate::shared::task::spawn(async move {
            let mut tick = tokio::time::interval(IDLE_CHECK_INTERVAL);
            tick.tick().await;
            loop {
                tick.tick().await;
                let Some(presenter) = presenter.upgrade() else {
                    break;
                };
                presenter.reap_idle(Instant::now()).await;
            }
        });
    }
}

impl Session {
    fn snapshot(&self, state: &SessionState) -> PresentationState {
        PresentationState {
            id: self.id.clone(),
            file_name: self.file_name.clone(),
            slide_count: self.slide_count,
            current_slide: state.current_slide,
            file_type: self.file_type,
            slide_videos: state.slide_videos.clone(),
            video_state: VideoState::Idle,
            overlay: state.overlay,
        }
    }

    /// `presentation:state` to the room, then the same state in the bot's metadata.
    async fn broadcast_state(&self) {
        let (snapshot, scene_id) = {
            let state = self.state.lock().await;
            (self.snapshot(&state), state.scene_id.clone())
        };
        let packet = wire::encode_packet(&wire::state_message(&snapshot), &scene_id);
        if let Err(e) = self.bot.publish_data(packet).await {
            tracing::warn!(presentation = %self.id, "state broadcast failed: {e}");
        }
        let metadata = wire::bot_metadata(&self.id, Some(&snapshot));
        if let Err(e) = self.bot.update_metadata(metadata).await {
            tracing::warn!(presentation = %self.id, "metadata update failed: {e}");
        }
    }

    /// The command in a packet, when its sender is a presenter. Only a
    /// presenter's packet moves the scene id the bot answers with.
    async fn authorize(&self, identity: &str, payload: &[u8]) -> Option<Command> {
        let incoming = wire::decode_packet(payload)?;
        let kind = incoming.message.get("type").and_then(|t| t.as_str())?;
        if !kind.starts_with(wire::COMMAND_PREFIX) {
            return None;
        }
        let mut state = self.state.lock().await;
        if !state.presenters.contains(&identity.to_lowercase()) {
            tracing::debug!(presentation = %self.id, identity, kind, "not a presenter, ignored");
            return None;
        }
        if let Some(scene_id) = incoming.scene_id {
            state.scene_id = scene_id;
        }
        drop(state);
        wire::parse_command(&incoming.message)
    }

    /// Handles a command; `false` when the session must stop (its own render
    /// timed out). A slide change with no render thread free is dropped: the
    /// session stays on its slide and broadcasts that state.
    async fn handle(&self, command: Command) -> bool {
        match command {
            Command::GetState => self.broadcast_state().await,
            Command::Navigate(navigate) => match self.navigate(navigate).await {
                Ok(()) => {}
                Err(RenderError::TimedOut) => {
                    tracing::error!(presentation = %self.id, "a slide did not render in time, stopping");
                    return false;
                }
                Err(RenderError::Failed(e)) => {
                    tracing::warn!(presentation = %self.id, "navigate failed: {e}");
                }
                Err(RenderError::Busy) => {
                    tracing::warn!(presentation = %self.id, "navigate dropped: no render thread free");
                }
            },
            Command::Unsupported(kind) => {
                tracing::info!(presentation = %self.id, kind, "command not supported by this node");
            }
            // Handled by the session loop.
            Command::Stop | Command::Overlay(_) => {}
        }
        true
    }

    async fn navigate(&self, navigate: Navigate) -> Result<(), RenderError> {
        let current = self.state.lock().await.current_slide;
        let last = self.slide_count - 1;
        let target = match navigate {
            Navigate::Next => (current + 1).min(last),
            Navigate::Prev => current.saturating_sub(1),
            Navigate::Goto(Some(index)) if index < self.slide_count as f64 => index as usize,
            Navigate::Goto(index) => {
                return Err(RenderError::Failed(format!(
                    "Invalid slide index: {}",
                    index.map_or("undefined".to_string(), crate::shared::js::number_to_string)
                )));
            }
        };
        if target == current {
            return Ok(());
        }
        match self.renders.render(&self.doc, target).await {
            Ok((frame, videos)) => {
                if let Err(e) = self.bot.push_frame(&frame).await {
                    tracing::warn!(presentation = %self.id, "frame push failed: {e}");
                }
                {
                    let mut state = self.state.lock().await;
                    state.current_slide = target;
                    state.slide_videos = videos;
                    state.last_frame = frame;
                }
                tracing::info!(presentation = %self.id, slide = target, "navigated");
                self.broadcast_state().await;
                Ok(())
            }
            Err(RenderError::TimedOut) => Err(RenderError::TimedOut),
            Err(e) => {
                let last_frame = self.state.lock().await.last_frame.clone();
                let _ = self.bot.push_frame(&last_frame).await;
                self.broadcast_state().await;
                Err(e)
            }
        }
    }
}
