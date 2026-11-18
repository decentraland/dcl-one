//! Presentation sessions, in memory as upstream keeps them
//! (`src/logic/presentation-manager/component.ts`): create (connect first,
//! then render and publish), the data-channel commands, the state broadcast,
//! stop, and the idle reaper (every 60 s, a session with no remote
//! participant for 5 minutes stops).

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::mpsc;

use super::bot::{BotEvent, BotLauncher, BotLink, ConnectError};
use super::render::{Frame, PdfDocument};
use super::wire::{
    self, Command, DEFAULT_OVERLAY, FileType, Navigate, Overlay, PresentationInfo,
    PresentationState, SlideVideo, VideoState,
};

pub const IDLE_TIMEOUT: Duration = Duration::from_secs(5 * 60);
pub const IDLE_CHECK_INTERVAL: Duration = Duration::from_secs(60);
const OVERLAY_BROADCAST_INTERVAL: Duration = Duration::from_millis(250);
/// `MAX_CONCURRENT_PRESENTATIONS` when unset.
pub const DEFAULT_MAX_CONCURRENT: usize = 10;

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
    bot: Arc<dyn BotLink>,
    state: tokio::sync::Mutex<SessionState>,
    activity: Mutex<Activity>,
}

struct Activity {
    participants: usize,
    last_active: Instant,
}

/// The sessions of this node.
pub struct Presenter {
    launcher: Arc<dyn BotLauncher>,
    /// `None` is unlimited (`MAX_CONCURRENT_PRESENTATIONS=0`).
    max_concurrent: Option<usize>,
    sessions: Mutex<HashMap<String, Arc<Session>>>,
    in_flight: AtomicUsize,
}

struct InFlight<'a>(&'a AtomicUsize);

impl Drop for InFlight<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

async fn render(doc: &Arc<PdfDocument>, index: usize) -> Result<(Frame, Vec<SlideVideo>), String> {
    let doc = doc.clone();
    tokio::task::spawn_blocking(move || doc.render(index))
        .await
        .map_err(|e| format!("Failed to render slide {index}: {e}"))?
}

impl Presenter {
    /// `max_concurrent` as configured: 0 is unlimited.
    pub fn new(launcher: Arc<dyn BotLauncher>, max_concurrent: usize) -> Arc<Self> {
        Arc::new(Self {
            launcher,
            max_concurrent: (max_concurrent > 0).then_some(max_concurrent),
            sessions: Mutex::new(HashMap::new()),
            in_flight: AtomicUsize::new(0),
        })
    }

    pub fn active_sessions(&self) -> usize {
        self.sessions.lock().unwrap().len()
    }

    /// The JSON path's pre-flight: connect and disconnect, so a refused token
    /// costs no download.
    pub async fn validate_credentials(&self, url: &str, token: &str) -> Result<(), ConnectError> {
        let connected = self.launcher.connect(url, token).await?;
        connected.link.shutdown().await;
        Ok(())
    }

    /// Creates a session from a PDF: the concurrency limit, the LiveKit
    /// connect, the first slide published, the first state broadcast.
    pub async fn create(
        self: &Arc<Self>,
        file: Vec<u8>,
        livekit_url: &str,
        livekit_token: &str,
        file_name: &str,
    ) -> Result<PresentationInfo, CreateError> {
        {
            let sessions = self.sessions.lock().unwrap();
            if let Some(max) = self.max_concurrent
                && sessions.len() + self.in_flight.load(Ordering::SeqCst) >= max
            {
                return Err(CreateError::MaxConcurrent(max));
            }
            self.in_flight.fetch_add(1, Ordering::SeqCst);
        }
        let _in_flight = InFlight(&self.in_flight);
        let id = uuid::Uuid::new_v4().to_string();
        let connected = self
            .launcher
            .connect(livekit_url, livekit_token)
            .await
            .map_err(|e| CreateError::Internal(e.to_string()))?;
        let bot = connected.link.clone();
        match self
            .start(
                &id,
                connected.ready.room_metadata.as_str(),
                file,
                file_name,
                &bot,
            )
            .await
        {
            Ok(session) => {
                {
                    let mut activity = session.activity.lock().unwrap();
                    activity.participants = connected.ready.participant_count;
                }
                self.sessions
                    .lock()
                    .unwrap()
                    .insert(id.clone(), session.clone());
                tracing::info!(presentation = %id, slides = session.slide_count, "presentation created");
                session.broadcast_state().await;
                tokio::spawn(self.clone().run(session.clone(), connected.events));
                Ok(PresentationInfo {
                    id,
                    file_name: session.file_name.clone(),
                    slide_count: session.slide_count,
                    current_slide: 0,
                    file_type: session.file_type,
                })
            }
            Err(e) => {
                bot.shutdown().await;
                Err(e)
            }
        }
    }

    async fn start(
        &self,
        id: &str,
        room_metadata: &str,
        file: Vec<u8>,
        file_name: &str,
        bot: &Arc<dyn BotLink>,
    ) -> Result<Arc<Session>, CreateError> {
        bot.update_metadata(wire::bot_metadata(id, None))
            .await
            .map_err(|e| CreateError::Internal(e.to_string()))?;
        let doc = tokio::task::spawn_blocking(move || PdfDocument::load(file))
            .await
            .map_err(|e| CreateError::Internal(e.to_string()))?
            .map_err(CreateError::Internal)?;
        let doc = Arc::new(doc);
        let slide_count = doc.page_count();
        if slide_count == 0 {
            return Err(CreateError::Internal("PDF contains no pages".into()));
        }
        let (frame, slide_videos) = render(&doc, 0).await.map_err(CreateError::SlideRender)?;
        bot.publish_video(frame.width, frame.height)
            .await
            .map_err(|e| CreateError::Internal(e.to_string()))?;
        bot.push_frame(&frame)
            .await
            .map_err(|e| CreateError::Internal(e.to_string()))?;
        Ok(Arc::new(Session {
            id: id.to_string(),
            file_name: super::files::presentation_name(file_name),
            file_type: FileType::Pdf,
            slide_count,
            doc,
            bot: bot.clone(),
            state: tokio::sync::Mutex::new(SessionState {
                current_slide: 0,
                slide_videos,
                overlay: DEFAULT_OVERLAY,
                last_frame: frame,
                scene_id: String::new(),
                presenters: wire::presenters_from_room_metadata(room_metadata),
            }),
            activity: Mutex::new(Activity {
                participants: 0,
                last_active: Instant::now(),
            }),
        }))
    }

    /// One session's room events, handled one at a time.
    async fn run(self: Arc<Self>, session: Arc<Session>, mut events: mpsc::Receiver<BotEvent>) {
        let mut overlay_due: Option<tokio::time::Instant> = None;
        loop {
            let event = match overlay_due {
                Some(due) => tokio::select! {
                    event = events.recv() => event,
                    _ = tokio::time::sleep_until(due) => {
                        overlay_due = None;
                        session.broadcast_state().await;
                        continue;
                    }
                },
                None => events.recv().await,
            };
            let Some(event) = event else { break };
            if !self.is_live(&session.id) {
                break;
            }
            match event {
                BotEvent::RoomMetadata(metadata) => {
                    session.state.lock().await.presenters =
                        wire::presenters_from_room_metadata(&metadata);
                }
                BotEvent::ParticipantCount(count) => {
                    session.activity.lock().unwrap().participants = count;
                }
                BotEvent::Disconnected(reason) => {
                    tracing::warn!(presentation = %session.id, %reason, "bot disconnected");
                    self.stop(&session.id).await;
                    break;
                }
                BotEvent::Data { identity, payload } => {
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
                        other => session.handle(other).await,
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
                let mut activity = session.activity.lock().unwrap();
                if activity.participants > 0 {
                    activity.last_active = now;
                    false
                } else {
                    now.duration_since(activity.last_active) >= IDLE_TIMEOUT
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
        tokio::spawn(async move {
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

    async fn handle(&self, command: Command) {
        match command {
            Command::GetState => self.broadcast_state().await,
            Command::Navigate(navigate) => {
                if let Err(e) = self.navigate(navigate).await {
                    tracing::warn!(presentation = %self.id, "navigate failed: {e}");
                }
            }
            Command::Unsupported(kind) => {
                tracing::info!(presentation = %self.id, kind, "command not supported by this node");
            }
            // Handled by the session loop.
            Command::Stop | Command::Overlay(_) => {}
        }
    }

    async fn navigate(&self, navigate: Navigate) -> Result<(), String> {
        let current = self.state.lock().await.current_slide;
        let last = self.slide_count - 1;
        let target = match navigate {
            Navigate::Next => (current + 1).min(last),
            Navigate::Prev => current.saturating_sub(1),
            Navigate::Goto(Some(index)) if index < self.slide_count as f64 => index as usize,
            Navigate::Goto(index) => {
                return Err(format!(
                    "Invalid slide index: {}",
                    index.map_or("undefined".to_string(), crate::shared::js::number_to_string)
                ));
            }
        };
        if target == current {
            return Ok(());
        }
        match render(&self.doc, target).await {
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
            Err(e) => {
                let last_frame = self.state.lock().await.last_frame.clone();
                let _ = self.bot.push_frame(&last_frame).await;
                self.broadcast_state().await;
                Err(e)
            }
        }
    }
}
