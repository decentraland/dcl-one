//! One media source's playback and reopen state as a pure machine: [`update`] takes a
//! [`MediaSourceMsg`] (a fact that already happened) and returns the [`MediaSourceCmd`] to carry
//! out; [`view`] derives the source's reported state. Neither reads a clock or touches the element.

use std::time::Duration;

use dcl_component::proto_components::sdk::components::VideoState;

use crate::{MediaRetry, MediaRetryStatus, RetryStep};

/// Page-wide media element id.
pub type MediaId = u32;

/// A fact about the source.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MediaSourceMsg {
    ScenePlayRequested,
    ScenePauseRequested,
    SceneSeekRequested {
        position_seconds: f64,
    },
    SceneLoopingChanged {
        looping: bool,
    },
    SceneVolumeChanged {
        volume: f32,
    },
    ElementObserved {
        observation: ElementObservation,
        snapshot: ElementSnapshot,
    },
    /// The timer of the reopen scheduled after failure `failure_count` fired.
    ReopenTimerFired {
        failure_count: u32,
    },
}

/// What the media element reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementObservation {
    /// `canplay`
    BecamePlayable,
    /// `playing`
    StartedPlaying,
    /// `waiting`
    Stalled,
    /// `seeking`
    StartedSeeking,
    /// `pause`, not at the end
    Paused,
    /// `timeupdate`
    PositionAdvanced,
    /// `ended`
    ReachedEnd,
    /// `error`
    Failed(MediaFailureKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaFailureKind {
    Aborted,
    Network,
    Decode,
    SourceNotSupported,
    Unspecified,
    LiveStreamEnded,
}

impl MediaFailureKind {
    /// The kind from a `MediaError.code`.
    pub fn from_media_error_code(code: Option<u16>) -> Self {
        match code {
            Some(1) => Self::Aborted,
            Some(2) => Self::Network,
            Some(3) => Self::Decode,
            Some(4) => Self::SourceNotSupported,
            _ => Self::Unspecified,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Aborted => "aborted",
            Self::Network => "network error",
            Self::Decode => "decode error",
            Self::SourceNotSupported => "source not supported",
            Self::Unspecified => "media error",
            Self::LiveStreamEnded => "live stream ended",
        }
    }
}

/// The element, and the page's reopen schedule, when it reported.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ElementSnapshot {
    pub position_seconds: f64,
    /// Infinite for a live stream; NaN before metadata.
    pub duration_seconds: f64,
    /// The wait before a first reopen; later ones double from it.
    pub first_reopen_delay: Duration,
}

/// An intent for the page to carry out on the element `media_id`.
#[derive(Debug, Clone, PartialEq)]
pub enum MediaSourceCmd {
    None,
    /// Carried out in order.
    Multi(Vec<MediaSourceCmd>),
    AttachSource {
        media_id: MediaId,
    },
    DetachSource {
        media_id: MediaId,
    },
    StartPlayback {
        media_id: MediaId,
    },
    PausePlayback {
        media_id: MediaId,
    },
    SeekTo {
        media_id: MediaId,
        position_seconds: f64,
    },
    SetLooping {
        media_id: MediaId,
        looping: bool,
    },
    SetVolume {
        media_id: MediaId,
        volume: f32,
    },
    ClearScreen {
        media_id: MediaId,
    },
    /// A [`MediaSourceMsg::ReopenTimerFired`] with `failure_count`, due after `delay`.
    ScheduleReopen {
        media_id: MediaId,
        delay: Duration,
        failure_count: u32,
    },
}

impl MediaSourceCmd {
    fn batch(commands: Vec<MediaSourceCmd>) -> Self {
        let mut commands: Vec<_> = commands
            .into_iter()
            .filter(|command| *command != MediaSourceCmd::None)
            .collect();
        match commands.len() {
            0 => Self::None,
            1 => commands.remove(0),
            _ => Self::Multi(commands),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScenePlaybackIntent {
    Play,
    Pause,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaReopenPolicy {
    Reopen,
    /// No url to reopen: a placeholder or an adopted element.
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaStreamKind {
    OnDemand,
    /// Once the element reported an infinite duration; never reverts.
    Live,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReopenTimer {
    Pending,
    Elapsed,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MediaSourcePhase {
    /// The source is attached and cannot play yet; the element's states are not reported.
    Opening {
        resume_position_seconds: Option<f64>,
    },
    Open {
        element_state: VideoState,
        last_position_seconds: f64,
    },
    /// Detached; reopened once the timer elapsed and the scene wants play.
    Failed {
        reopen_delay: Duration,
        reopen_timer: ReopenTimer,
        resume_position_seconds: Option<f64>,
    },
    GaveUp,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MediaSourceState {
    media_id: MediaId,
    phase: MediaSourcePhase,
    scene_playback_intent: ScenePlaybackIntent,
    reopen_policy: MediaReopenPolicy,
    stream_kind: MediaStreamKind,
    retry: MediaRetry,
    failure_count: u32,
    last_failure: Option<MediaFailureKind>,
    duration_seconds: f64,
}

/// The source's reported state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MediaSourceView {
    pub video_state: VideoState,
    pub duration_seconds: f64,
    pub retry_status: MediaRetryStatus,
}

impl MediaSourceState {
    pub fn media_id(&self) -> MediaId {
        self.media_id
    }

    pub fn phase(&self) -> MediaSourcePhase {
        self.phase
    }

    /// The scene wants play and the element can take a `play()`.
    pub fn wants_element_playback(&self) -> bool {
        self.scene_playback_intent == ScenePlaybackIntent::Play
            && matches!(
                self.phase,
                MediaSourcePhase::Opening { .. } | MediaSourcePhase::Open { .. }
            )
    }
}

/// A source with a url attaches it and waits until it can play; one without (a placeholder, an
/// adopted element) is open as it is and never reopens.
pub fn init(
    media_id: MediaId,
    reopen_policy: MediaReopenPolicy,
) -> (MediaSourceState, MediaSourceCmd) {
    let (phase, cmd) = match reopen_policy {
        MediaReopenPolicy::Reopen => (
            MediaSourcePhase::Opening {
                resume_position_seconds: None,
            },
            MediaSourceCmd::AttachSource { media_id },
        ),
        MediaReopenPolicy::Never => (
            MediaSourcePhase::Open {
                element_state: VideoState::VsLoading,
                last_position_seconds: 0.0,
            },
            MediaSourceCmd::None,
        ),
    };
    let state = MediaSourceState {
        media_id,
        phase,
        scene_playback_intent: ScenePlaybackIntent::Pause,
        reopen_policy,
        stream_kind: MediaStreamKind::OnDemand,
        retry: MediaRetry::default(),
        failure_count: 0,
        last_failure: None,
        duration_seconds: f64::NAN,
    };
    (state, cmd)
}

pub fn update(state: &mut MediaSourceState, msg: MediaSourceMsg) -> MediaSourceCmd {
    let media_id = state.media_id;
    match msg {
        MediaSourceMsg::ScenePlayRequested => {
            state.scene_playback_intent = ScenePlaybackIntent::Play;
            match state.phase {
                MediaSourcePhase::Open { .. } => MediaSourceCmd::StartPlayback { media_id },
                MediaSourcePhase::Failed {
                    reopen_timer: ReopenTimer::Elapsed,
                    resume_position_seconds,
                    ..
                } => reopen(state, resume_position_seconds),
                _ => MediaSourceCmd::None,
            }
        }
        MediaSourceMsg::ScenePauseRequested => {
            state.scene_playback_intent = ScenePlaybackIntent::Pause;
            match state.phase {
                MediaSourcePhase::Opening { .. } | MediaSourcePhase::Open { .. } => {
                    MediaSourceCmd::PausePlayback { media_id }
                }
                _ => MediaSourceCmd::None,
            }
        }
        MediaSourceMsg::SceneSeekRequested { position_seconds } => {
            let on_demand = state.stream_kind == MediaStreamKind::OnDemand;
            match &mut state.phase {
                MediaSourcePhase::Open { .. } => MediaSourceCmd::SeekTo {
                    media_id,
                    position_seconds,
                },
                MediaSourcePhase::Opening {
                    resume_position_seconds,
                }
                | MediaSourcePhase::Failed {
                    resume_position_seconds,
                    ..
                } if on_demand => {
                    *resume_position_seconds = Some(position_seconds);
                    MediaSourceCmd::None
                }
                _ => MediaSourceCmd::None,
            }
        }
        MediaSourceMsg::SceneLoopingChanged { looping } => {
            MediaSourceCmd::SetLooping { media_id, looping }
        }
        MediaSourceMsg::SceneVolumeChanged { volume } => {
            MediaSourceCmd::SetVolume { media_id, volume }
        }
        MediaSourceMsg::ElementObserved {
            observation,
            snapshot,
        } => element_observed(state, observation, snapshot),
        MediaSourceMsg::ReopenTimerFired { failure_count } => match state.phase {
            MediaSourcePhase::Failed {
                resume_position_seconds,
                ..
            } if failure_count == state.failure_count => {
                if state.scene_playback_intent == ScenePlaybackIntent::Play {
                    reopen(state, resume_position_seconds)
                } else {
                    if let MediaSourcePhase::Failed { reopen_timer, .. } = &mut state.phase {
                        *reopen_timer = ReopenTimer::Elapsed;
                    }
                    MediaSourceCmd::None
                }
            }
            _ => MediaSourceCmd::None,
        },
    }
}

fn element_observed(
    state: &mut MediaSourceState,
    observation: ElementObservation,
    snapshot: ElementSnapshot,
) -> MediaSourceCmd {
    let media_id = state.media_id;
    if matches!(
        state.phase,
        MediaSourcePhase::Failed { .. } | MediaSourcePhase::GaveUp
    ) {
        return MediaSourceCmd::None;
    }
    state.duration_seconds = snapshot.duration_seconds;
    if snapshot.duration_seconds == f64::INFINITY {
        state.stream_kind = MediaStreamKind::Live;
    }
    let live = state.stream_kind == MediaStreamKind::Live;
    match (observation, state.phase) {
        (ElementObservation::Failed(kind), _) => fail(state, kind, snapshot),
        (ElementObservation::ReachedEnd, _) if live => {
            fail(state, MediaFailureKind::LiveStreamEnded, snapshot)
        }
        (
            ElementObservation::BecamePlayable,
            MediaSourcePhase::Opening {
                resume_position_seconds,
            },
        ) => {
            state.phase = MediaSourcePhase::Open {
                element_state: VideoState::VsReady,
                last_position_seconds: resume_position_seconds.unwrap_or(snapshot.position_seconds),
            };
            let seek = match resume_position_seconds {
                Some(position_seconds) => MediaSourceCmd::SeekTo {
                    media_id,
                    position_seconds,
                },
                None => MediaSourceCmd::None,
            };
            let play = match state.scene_playback_intent {
                ScenePlaybackIntent::Play => MediaSourceCmd::StartPlayback { media_id },
                ScenePlaybackIntent::Pause => MediaSourceCmd::None,
            };
            MediaSourceCmd::batch(vec![seek, play])
        }
        (_, MediaSourcePhase::Opening { .. }) => MediaSourceCmd::None,
        (
            ElementObservation::PositionAdvanced,
            MediaSourcePhase::Open {
                last_position_seconds,
                ..
            },
        ) => {
            let step = snapshot.position_seconds - last_position_seconds;
            if let MediaSourcePhase::Open {
                last_position_seconds,
                ..
            } = &mut state.phase
            {
                *last_position_seconds = snapshot.position_seconds;
            }
            // a seek or a loop is no playback
            if (0.0..1.0).contains(&step) {
                state.retry.played(Duration::from_secs_f64(step));
            }
            MediaSourceCmd::None
        }
        (ElementObservation::ReachedEnd, MediaSourcePhase::Open { .. }) => {
            set_element_state(state, VideoState::VsNone);
            // an end clears the intent to play
            state.scene_playback_intent = ScenePlaybackIntent::Pause;
            MediaSourceCmd::ClearScreen { media_id }
        }
        (observation, MediaSourcePhase::Open { .. }) => {
            let element_state = match observation {
                ElementObservation::BecamePlayable => VideoState::VsReady,
                ElementObservation::StartedPlaying => VideoState::VsPlaying,
                ElementObservation::Stalled => VideoState::VsBuffering,
                ElementObservation::StartedSeeking => VideoState::VsSeeking,
                _ => VideoState::VsPaused,
            };
            set_element_state(state, element_state);
            MediaSourceCmd::None
        }
        (_, MediaSourcePhase::Failed { .. } | MediaSourcePhase::GaveUp) => MediaSourceCmd::None,
    }
}

fn set_element_state(state: &mut MediaSourceState, new_state: VideoState) {
    if let MediaSourcePhase::Open { element_state, .. } = &mut state.phase {
        *element_state = new_state;
    }
}

/// Counts the failure, then detaches the source until its reopen, or gives up.
fn fail(
    state: &mut MediaSourceState,
    kind: MediaFailureKind,
    snapshot: ElementSnapshot,
) -> MediaSourceCmd {
    let media_id = state.media_id;
    state.failure_count += 1;
    state.last_failure = Some(kind);
    if state.reopen_policy == MediaReopenPolicy::Never {
        state.phase = MediaSourcePhase::GaveUp;
        return MediaSourceCmd::None;
    }
    let pending_resume = match state.phase {
        MediaSourcePhase::Opening {
            resume_position_seconds,
        } => resume_position_seconds,
        _ => None,
    };
    let resume_position_seconds = match state.stream_kind {
        MediaStreamKind::Live => None,
        MediaStreamKind::OnDemand => pending_resume
            .or((snapshot.position_seconds > 0.0).then_some(snapshot.position_seconds)),
    };
    match state.retry.fail(snapshot.first_reopen_delay) {
        RetryStep::Reopen { delay, .. } => {
            state.phase = MediaSourcePhase::Failed {
                reopen_delay: delay,
                reopen_timer: ReopenTimer::Pending,
                resume_position_seconds,
            };
            MediaSourceCmd::Multi(vec![
                MediaSourceCmd::DetachSource { media_id },
                MediaSourceCmd::ScheduleReopen {
                    media_id,
                    delay,
                    failure_count: state.failure_count,
                },
            ])
        }
        RetryStep::GiveUp => {
            state.phase = MediaSourcePhase::GaveUp;
            MediaSourceCmd::Multi(vec![
                // stops the buffered media before the screen is cleared
                MediaSourceCmd::PausePlayback { media_id },
                MediaSourceCmd::DetachSource { media_id },
                MediaSourceCmd::ClearScreen { media_id },
            ])
        }
    }
}

fn reopen(state: &mut MediaSourceState, resume_position_seconds: Option<f64>) -> MediaSourceCmd {
    state.phase = MediaSourcePhase::Opening {
        resume_position_seconds,
    };
    MediaSourceCmd::AttachSource {
        media_id: state.media_id,
    }
}

pub fn view(state: &MediaSourceState) -> MediaSourceView {
    let wants_play = state.scene_playback_intent == ScenePlaybackIntent::Play;
    let (video_state, next_delay) = match state.phase {
        MediaSourcePhase::Opening { .. } => (VideoState::VsLoading, None),
        MediaSourcePhase::Open { element_state, .. } => (element_state, None),
        MediaSourcePhase::Failed { reopen_delay, .. } if wants_play => {
            (VideoState::VsLoading, Some(reopen_delay))
        }
        MediaSourcePhase::Failed { .. } | MediaSourcePhase::GaveUp => (VideoState::VsError, None),
    };
    MediaSourceView {
        video_state,
        duration_seconds: state.duration_seconds,
        retry_status: MediaRetryStatus {
            attempt: state.retry.attempts(),
            failures: state.failure_count,
            next_delay,
            gave_up: state.phase == MediaSourcePhase::GaveUp,
            last_error: state.last_failure.map(MediaFailureKind::label),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{HEALTHY_PLAYBACK, MAX_REOPENS};

    const ID: MediaId = 7;
    const FIRST: Duration = Duration::from_secs(2);

    fn snapshot(position_seconds: f64) -> ElementSnapshot {
        ElementSnapshot {
            position_seconds,
            duration_seconds: 60.0,
            first_reopen_delay: FIRST,
        }
    }

    fn live_snapshot() -> ElementSnapshot {
        ElementSnapshot {
            position_seconds: 3.0,
            duration_seconds: f64::INFINITY,
            first_reopen_delay: FIRST,
        }
    }

    fn observed(observation: ElementObservation, snapshot: ElementSnapshot) -> MediaSourceMsg {
        MediaSourceMsg::ElementObserved {
            observation,
            snapshot,
        }
    }

    fn failed(position_seconds: f64) -> MediaSourceMsg {
        observed(
            ElementObservation::Failed(MediaFailureKind::Network),
            snapshot(position_seconds),
        )
    }

    fn schedule(delay_seconds: u64, failure_count: u32) -> MediaSourceCmd {
        MediaSourceCmd::Multi(vec![
            MediaSourceCmd::DetachSource { media_id: ID },
            MediaSourceCmd::ScheduleReopen {
                media_id: ID,
                delay: Duration::from_secs(delay_seconds),
                failure_count,
            },
        ])
    }

    fn attach() -> MediaSourceCmd {
        MediaSourceCmd::AttachSource { media_id: ID }
    }

    fn video_state(state: &MediaSourceState) -> VideoState {
        view(state).video_state
    }

    /// A url source the scene plays, open.
    fn playing() -> MediaSourceState {
        let (mut state, _) = init(ID, MediaReopenPolicy::Reopen);
        update(&mut state, MediaSourceMsg::ScenePlayRequested);
        update(
            &mut state,
            observed(ElementObservation::BecamePlayable, snapshot(0.0)),
        );
        update(
            &mut state,
            observed(ElementObservation::StartedPlaying, snapshot(0.0)),
        );
        state
    }

    fn play_for(state: &mut MediaSourceState, from_seconds: f64, duration: Duration) {
        let mut position = from_seconds;
        let end = from_seconds + duration.as_secs_f64();
        while position < end - 1e-9 {
            position = (position + 0.25).min(end);
            update(
                state,
                observed(ElementObservation::PositionAdvanced, snapshot(position)),
            );
        }
    }

    #[test]
    fn init_attaches_a_url_source_and_reports_loading() {
        let (state, cmd) = init(ID, MediaReopenPolicy::Reopen);
        assert_eq!(cmd, attach());
        assert_eq!(video_state(&state), VideoState::VsLoading);
    }

    #[test]
    fn opening_hides_element_states_until_playable_then_plays() {
        let (mut state, _) = init(ID, MediaReopenPolicy::Reopen);
        assert_eq!(
            update(&mut state, MediaSourceMsg::ScenePlayRequested),
            MediaSourceCmd::None
        );
        update(
            &mut state,
            observed(ElementObservation::Stalled, snapshot(0.0)),
        );
        assert_eq!(video_state(&state), VideoState::VsLoading);
        let cmd = update(
            &mut state,
            observed(ElementObservation::BecamePlayable, snapshot(0.0)),
        );
        assert_eq!(cmd, MediaSourceCmd::StartPlayback { media_id: ID });
        assert_eq!(video_state(&state), VideoState::VsReady);
        update(
            &mut state,
            observed(ElementObservation::StartedPlaying, snapshot(0.0)),
        );
        assert_eq!(video_state(&state), VideoState::VsPlaying);
    }

    #[test]
    fn healthy_playback_restarts_the_count_and_jumps_do_not_count() {
        let mut state = playing();
        update(&mut state, failed(0.0));
        update(
            &mut state,
            MediaSourceMsg::ReopenTimerFired { failure_count: 1 },
        );
        update(
            &mut state,
            observed(ElementObservation::BecamePlayable, snapshot(0.0)),
        );
        assert_eq!(view(&state).retry_status.attempt, 1);
        // a seek forward is no playback
        update(
            &mut state,
            observed(ElementObservation::PositionAdvanced, snapshot(30.0)),
        );
        assert_eq!(view(&state).retry_status.attempt, 1);
        play_for(
            &mut state,
            30.0,
            HEALTHY_PLAYBACK - Duration::from_millis(250),
        );
        assert_eq!(view(&state).retry_status.attempt, 1);
        play_for(&mut state, 34.75, Duration::from_millis(250));
        assert_eq!(view(&state).retry_status.attempt, 0);
        assert_eq!(view(&state).retry_status.failures, 1);
    }

    #[test]
    fn a_failure_while_playing_detaches_and_reopens_when_the_timer_fires() {
        let mut state = playing();
        assert_eq!(update(&mut state, failed(0.0)), schedule(2, 1));
        let shown = view(&state);
        assert_eq!(shown.video_state, VideoState::VsLoading);
        assert_eq!(shown.retry_status.next_delay, Some(Duration::from_secs(2)));
        assert_eq!(shown.retry_status.last_error, Some("network error"));
        assert_eq!(
            update(
                &mut state,
                MediaSourceMsg::ReopenTimerFired { failure_count: 1 }
            ),
            attach()
        );
        assert_eq!(video_state(&state), VideoState::VsLoading);
        assert_eq!(view(&state).retry_status.next_delay, None);
    }

    #[test]
    fn a_stale_timer_is_ignored() {
        let mut state = playing();
        update(&mut state, failed(0.0));
        update(
            &mut state,
            MediaSourceMsg::ReopenTimerFired { failure_count: 1 },
        );
        update(&mut state, failed(0.0));
        assert_eq!(
            update(
                &mut state,
                MediaSourceMsg::ReopenTimerFired { failure_count: 1 }
            ),
            MediaSourceCmd::None
        );
        assert_eq!(
            update(
                &mut state,
                MediaSourceMsg::ReopenTimerFired { failure_count: 2 }
            ),
            attach()
        );
    }

    #[test]
    fn a_failure_while_paused_reports_error_and_waits_for_play() {
        let mut state = playing();
        update(&mut state, MediaSourceMsg::ScenePauseRequested);
        update(&mut state, failed(0.0));
        assert_eq!(video_state(&state), VideoState::VsError);
        assert_eq!(view(&state).retry_status.next_delay, None);
        // the timer elapses while paused: nothing reopens
        assert_eq!(
            update(
                &mut state,
                MediaSourceMsg::ReopenTimerFired { failure_count: 1 }
            ),
            MediaSourceCmd::None
        );
        assert_eq!(
            update(&mut state, MediaSourceMsg::ScenePlayRequested),
            attach()
        );
        assert_eq!(video_state(&state), VideoState::VsLoading);
    }

    #[test]
    fn play_before_the_delay_passed_reopens_when_the_timer_fires() {
        let mut state = playing();
        update(&mut state, failed(0.0));
        update(&mut state, MediaSourceMsg::ScenePauseRequested);
        assert_eq!(video_state(&state), VideoState::VsError);
        assert_eq!(
            update(&mut state, MediaSourceMsg::ScenePlayRequested),
            MediaSourceCmd::None
        );
        assert_eq!(video_state(&state), VideoState::VsLoading);
        assert_eq!(
            update(
                &mut state,
                MediaSourceMsg::ReopenTimerFired { failure_count: 1 }
            ),
            attach()
        );
    }

    #[test]
    fn a_pause_while_reopening_keeps_loading() {
        let mut state = playing();
        update(&mut state, failed(0.0));
        update(
            &mut state,
            MediaSourceMsg::ReopenTimerFired { failure_count: 1 },
        );
        assert_eq!(
            update(&mut state, MediaSourceMsg::ScenePauseRequested),
            MediaSourceCmd::PausePlayback { media_id: ID }
        );
        assert!(matches!(state.phase(), MediaSourcePhase::Opening { .. }));
        assert_eq!(
            update(
                &mut state,
                observed(ElementObservation::BecamePlayable, snapshot(0.0))
            ),
            MediaSourceCmd::None
        );
        assert_eq!(view(&state).retry_status.attempt, 1);
    }

    #[test]
    fn delays_double_then_the_sixth_failure_gives_up() {
        let mut state = playing();
        for (index, delay) in [2, 4, 8, 16, 32].into_iter().enumerate() {
            let failure_count = index as u32 + 1;
            assert_eq!(
                update(&mut state, failed(0.0)),
                schedule(delay, failure_count)
            );
            update(
                &mut state,
                MediaSourceMsg::ReopenTimerFired { failure_count },
            );
        }
        assert_eq!(
            update(&mut state, failed(0.0)),
            MediaSourceCmd::Multi(vec![
                MediaSourceCmd::PausePlayback { media_id: ID },
                MediaSourceCmd::DetachSource { media_id: ID },
                MediaSourceCmd::ClearScreen { media_id: ID },
            ])
        );
        let shown = view(&state);
        assert_eq!(shown.video_state, VideoState::VsError);
        assert!(shown.retry_status.gave_up);
        assert_eq!(shown.retry_status.attempt, MAX_REOPENS);
        assert_eq!(shown.retry_status.failures, MAX_REOPENS + 1);
    }

    #[test]
    fn giving_up_is_final() {
        let mut state = playing();
        for failure_count in 1..=MAX_REOPENS + 1 {
            update(&mut state, failed(0.0));
            update(
                &mut state,
                MediaSourceMsg::ReopenTimerFired { failure_count },
            );
        }
        let before = state;
        for msg in [
            MediaSourceMsg::ScenePlayRequested,
            MediaSourceMsg::ScenePauseRequested,
            MediaSourceMsg::SceneSeekRequested {
                position_seconds: 4.0,
            },
            observed(ElementObservation::BecamePlayable, snapshot(0.0)),
            failed(0.0),
            MediaSourceMsg::ReopenTimerFired {
                failure_count: MAX_REOPENS + 1,
            },
        ] {
            assert_eq!(update(&mut state, msg), MediaSourceCmd::None);
        }
        assert_eq!(view(&state).video_state, VideoState::VsError);
        assert_eq!(view(&state), view(&before));
    }

    #[test]
    fn a_live_end_is_a_failure_and_an_on_demand_end_reports_none_and_clears_the_screen() {
        let mut live = playing();
        update(
            &mut live,
            observed(ElementObservation::PositionAdvanced, live_snapshot()),
        );
        let cmd = update(
            &mut live,
            observed(ElementObservation::ReachedEnd, snapshot(3.0)),
        );
        assert_eq!(cmd, schedule(2, 1));
        assert_eq!(
            view(&live).retry_status.last_error,
            Some("live stream ended")
        );

        let mut on_demand = playing();
        assert_eq!(
            update(
                &mut on_demand,
                observed(ElementObservation::ReachedEnd, snapshot(60.0))
            ),
            MediaSourceCmd::ClearScreen { media_id: ID }
        );
        assert_eq!(video_state(&on_demand), VideoState::VsNone);
        assert!(!on_demand.wants_element_playback());
        assert_eq!(
            update(&mut on_demand, MediaSourceMsg::ScenePlayRequested),
            MediaSourceCmd::StartPlayback { media_id: ID }
        );
    }

    #[test]
    fn an_on_demand_reopen_resumes_where_it_failed_or_was_seeked() {
        let mut state = playing();
        update(&mut state, failed(12.0));
        update(
            &mut state,
            MediaSourceMsg::ReopenTimerFired { failure_count: 1 },
        );
        assert_eq!(
            update(
                &mut state,
                observed(ElementObservation::BecamePlayable, snapshot(0.0))
            ),
            MediaSourceCmd::Multi(vec![
                MediaSourceCmd::SeekTo {
                    media_id: ID,
                    position_seconds: 12.0
                },
                MediaSourceCmd::StartPlayback { media_id: ID },
            ])
        );

        let mut seeked = playing();
        update(&mut seeked, failed(12.0));
        update(
            &mut seeked,
            MediaSourceMsg::SceneSeekRequested {
                position_seconds: 40.0,
            },
        );
        update(
            &mut seeked,
            MediaSourceMsg::ReopenTimerFired { failure_count: 1 },
        );
        let cmd = update(
            &mut seeked,
            observed(ElementObservation::BecamePlayable, snapshot(0.0)),
        );
        assert!(matches!(
            cmd,
            MediaSourceCmd::Multi(ref cmds) if cmds[0] == MediaSourceCmd::SeekTo { media_id: ID, position_seconds: 40.0 }
        ));
    }

    #[test]
    fn a_live_reopen_resumes_at_the_live_edge() {
        let mut state = playing();
        update(
            &mut state,
            observed(ElementObservation::PositionAdvanced, live_snapshot()),
        );
        update(
            &mut state,
            observed(
                ElementObservation::Failed(MediaFailureKind::Network),
                live_snapshot(),
            ),
        );
        update(
            &mut state,
            MediaSourceMsg::SceneSeekRequested {
                position_seconds: 9.0,
            },
        );
        update(
            &mut state,
            MediaSourceMsg::ReopenTimerFired { failure_count: 1 },
        );
        assert_eq!(
            update(
                &mut state,
                observed(ElementObservation::BecamePlayable, live_snapshot())
            ),
            MediaSourceCmd::StartPlayback { media_id: ID }
        );
    }

    #[test]
    fn a_source_without_a_url_gives_up_on_its_first_failure() {
        let (mut state, cmd) = init(ID, MediaReopenPolicy::Never);
        assert_eq!(cmd, MediaSourceCmd::None);
        update(
            &mut state,
            observed(ElementObservation::StartedPlaying, snapshot(0.0)),
        );
        assert_eq!(video_state(&state), VideoState::VsPlaying);
        assert_eq!(update(&mut state, failed(0.0)), MediaSourceCmd::None);
        assert_eq!(video_state(&state), VideoState::VsError);
        assert!(view(&state).retry_status.gave_up);
    }

    #[test]
    fn element_msgs_while_failed_are_ignored() {
        let mut state = playing();
        update(&mut state, failed(0.0));
        let before = state;
        for observation in [
            ElementObservation::BecamePlayable,
            ElementObservation::StartedPlaying,
            ElementObservation::Failed(MediaFailureKind::Decode),
        ] {
            assert_eq!(
                update(&mut state, observed(observation, snapshot(0.0))),
                MediaSourceCmd::None
            );
        }
        assert_eq!(state, before);
    }

    #[test]
    fn looping_and_volume_pass_through() {
        let mut state = playing();
        assert_eq!(
            update(
                &mut state,
                MediaSourceMsg::SceneLoopingChanged { looping: true }
            ),
            MediaSourceCmd::SetLooping {
                media_id: ID,
                looping: true
            }
        );
        assert_eq!(
            update(
                &mut state,
                MediaSourceMsg::SceneVolumeChanged { volume: 0.5 }
            ),
            MediaSourceCmd::SetVolume {
                media_id: ID,
                volume: 0.5
            }
        );
    }
}
