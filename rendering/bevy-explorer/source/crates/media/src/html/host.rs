//! The page side: owns the media elements and serves [`HostCommand`]s. Runs as a task on the
//! page's event loop; nothing here may block. Each element's playback and reopen state is a
//! [`MediaSourceState`]: the scene's commands, the element's events and the reopen timer become
//! [`MediaSourceMsg`]s for [`update`], [`execute`] carries out the [`MediaSourceCmd`] it returns,
//! and changes to its [`view`] are reported to the engine.

use std::{cell::RefCell, rc::Rc};

use bevy::{
    log::{debug, warn},
    platform::collections::HashMap,
};
use common::util::ReportErr;
use futures_channel::mpsc::{UnboundedSender, unbounded};
use futures_lite::StreamExt;
use js_sys::{Function, Reflect};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::JsFuture;
use web_sys::{HtmlMediaElement, HtmlVideoElement, VideoFrame, Worker};

use super::{HOST, HostCommand, HostHandle, MediaEvent, MediaId};
use crate::{
    AVCommand, ElementObservation, ElementSnapshot, MediaFailureKind, MediaReopenPolicy,
    MediaSourceCmd, MediaSourceMsg, MediaSourceState, MediaSourceView, first_reopen_delay, init,
    update, view,
};

type RcClosure = Rc<RefCell<Option<Closure<dyn FnMut(f64, JsValue)>>>>;

thread_local! {
    /// The page's media elements by id; other page code may add to it ([`adopt_video_element`]).
    static MEDIA: RefCell<HashMap<MediaId, HostMedia>> = RefCell::new(HashMap::default());
    /// The event sender and render worker, set by [`media_host_main`].
    static PAGE: RefCell<Option<(UnboundedSender<MediaEvent>, Worker)>> = const { RefCell::new(None) };
}

/// Page entry: starts serving media commands, transferring video frames to `render_worker`.
/// Call after the module is initialised on the page.
pub fn media_host_main(render_worker: Worker) {
    let (commands_tx, mut commands_rx) = unbounded::<HostCommand>();
    let (events_tx, events_rx) = unbounded::<MediaEvent>();
    if HOST
        .set(HostHandle {
            commands: commands_tx,
            events: std::sync::Mutex::new(Some(events_rx)),
        })
        .is_err()
    {
        panic!("media host already started");
    }
    PAGE.with(|page| *page.borrow_mut() = Some((events_tx.clone(), render_worker.clone())));

    // A `play()` the browser refused (no user gesture yet) is retried on the next gesture.
    let document = web_sys::window().unwrap().document().unwrap();
    let on_gesture = Closure::<dyn FnMut()>::new(|| {
        let refused: Vec<_> = MEDIA.with(|media| {
            media
                .borrow()
                .iter()
                .filter(|(_, m)| {
                    m.play_refusal == PlayRefusal::Refused && m.state.wants_element_playback()
                })
                .map(|(id, m)| (*id, m.media.clone()))
                .collect()
        });
        for (id, media) in refused {
            try_play(id, &media);
        }
    });
    for event in ["pointerdown", "keydown"] {
        document
            .add_event_listener_with_callback(event, on_gesture.as_ref().unchecked_ref())
            .report();
    }
    on_gesture.forget();

    wasm_bindgen_futures::spawn_local(async move {
        while let Some(command) = commands_rx.next().await {
            match command {
                HostCommand::Create { id, url, has_video } => {
                    let media = HostMedia::new(
                        id,
                        url,
                        has_video,
                        events_tx.clone(),
                        render_worker.clone(),
                    );
                    insert(id, media);
                }
                HostCommand::Command(id, command) => {
                    let msg = match command {
                        AVCommand::Play => MediaSourceMsg::ScenePlayRequested,
                        AVCommand::Pause => MediaSourceMsg::ScenePauseRequested,
                        AVCommand::Repeat(looping) => {
                            MediaSourceMsg::SceneLoopingChanged { looping }
                        }
                        AVCommand::Seek(position_seconds) => {
                            MediaSourceMsg::SceneSeekRequested { position_seconds }
                        }
                        AVCommand::Volume(volume) => MediaSourceMsg::SceneVolumeChanged { volume },
                        // no msg: only `HostCommand::Drop` removes the element
                        AVCommand::Dispose => continue,
                    };
                    dispatch(id, msg);
                }
                HostCommand::Drop(id) => {
                    // dropped after the borrow ends: `HostMedia`'s drop calls into the element
                    let removed = MEDIA.with(|media| media.borrow_mut().remove(&id));
                    drop(removed);
                }
            }
        }
    });
}

/// Registers an existing page `<video>` element (e.g. one a livekit track is attached to) as the
/// media `id` ([`super::adopt_video`]), so its frames flow to the render worker like any other
/// media. The host owns the element from here.
pub fn adopt_video_element(id: MediaId, element: HtmlVideoElement) {
    let Some((events, render_worker)) = PAGE.with(|page| page.borrow().clone()) else {
        debug!("no media host: cannot adopt video element {id}");
        return;
    };
    insert(
        id,
        HostMedia::from_element(id, element, None, true, events, render_worker),
    );
}

/// Adds `media` to [`MEDIA`] and carries out its first command.
fn insert(id: MediaId, (media, cmd): (HostMedia, MediaSourceCmd)) {
    MEDIA.with(|m| m.borrow_mut().insert(id, media));
    execute(cmd);
}

/// Updates media `id`'s state with `msg`, reports its changed view and carries out the result.
fn dispatch(id: MediaId, msg: MediaSourceMsg) {
    let cmd = MEDIA.with(|media| {
        let Ok(mut media) = media.try_borrow_mut() else {
            warn!("media {id}: {msg:?} arrived during another update; dropped");
            return None;
        };
        let m = media.get_mut(&id)?;
        let cmd = update(&mut m.state, msg);
        m.report();
        Some(cmd)
    });
    if let Some(cmd) = cmd {
        execute(cmd);
    }
}

/// Carries out `cmd` on the element of its media id; one no longer on the page is skipped.
fn execute(cmd: MediaSourceCmd) {
    let media_id = match cmd {
        MediaSourceCmd::None => return,
        MediaSourceCmd::Multi(cmds) => {
            for cmd in cmds {
                execute(cmd);
            }
            return;
        }
        MediaSourceCmd::AttachSource { media_id }
        | MediaSourceCmd::DetachSource { media_id }
        | MediaSourceCmd::StartPlayback { media_id }
        | MediaSourceCmd::PausePlayback { media_id }
        | MediaSourceCmd::SeekTo { media_id, .. }
        | MediaSourceCmd::SetLooping { media_id, .. }
        | MediaSourceCmd::SetVolume { media_id, .. }
        | MediaSourceCmd::ClearScreen { media_id }
        | MediaSourceCmd::ScheduleReopen { media_id, .. } => media_id,
    };
    let Some((media, url)) = MEDIA.with(|m| {
        m.borrow()
            .get(&media_id)
            .map(|m| (m.media.clone(), m.url.clone()))
    }) else {
        return;
    };
    match cmd {
        MediaSourceCmd::AttachSource { .. } => {
            debug!("media {media_id}: attaching the source");
            if let Some(url) = url {
                media.set_src(&url);
            }
        }
        MediaSourceCmd::DetachSource { .. } => detach(&media),
        MediaSourceCmd::StartPlayback { .. } => try_play(media_id, &media),
        MediaSourceCmd::PausePlayback { .. } => {
            set_play_refusal(media_id, PlayRefusal::NotRefused);
            media.pause().report();
        }
        MediaSourceCmd::SeekTo {
            position_seconds, ..
        } => media.set_current_time(position_seconds),
        MediaSourceCmd::SetLooping { looping, .. } => media.set_loop(looping),
        MediaSourceCmd::SetVolume { volume, .. } => media.set_volume(volume.clamp(0.0, 1.0) as f64),
        MediaSourceCmd::ClearScreen { .. } => clear_screen(media_id),
        MediaSourceCmd::ScheduleReopen {
            delay,
            failure_count,
            ..
        } => {
            debug!("media {media_id}: reopen in {delay:?}");
            // holds only the id: once the element is removed it dispatches to nothing
            let callback = Closure::once_into_js(move || {
                dispatch(media_id, MediaSourceMsg::ReopenTimerFired { failure_count })
            });
            if let Some(window) = web_sys::window() {
                window
                    .set_timeout_with_callback_and_timeout_and_arguments_0(
                        callback.unchecked_ref(),
                        delay.as_millis().min(i32::MAX as u128) as i32,
                    )
                    .report();
            }
        }
        MediaSourceCmd::None | MediaSourceCmd::Multi(_) => (),
    }
}

/// Removes the element's source; it raises no further events for it.
fn detach(media: &HtmlMediaElement) {
    let _ = media.remove_attribute("src");
    media.load();
}

/// Shows the cleared frame in place of the last one.
fn clear_screen(id: MediaId) {
    let Some(render_worker) = PAGE.with(|page| page.borrow().as_ref().map(|(_, w)| w.clone()))
    else {
        return;
    };
    let message = js_sys::Object::new();
    Reflect::set(&message, &"type".into(), &"VIDEO_CLEAR".into()).unwrap();
    Reflect::set(&message, &"id".into(), &JsValue::from(id)).unwrap();
    render_worker.post_message(&message).report();
}

/// Whether the browser refused the last `play()` (autoplay policy) since the last pause.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlayRefusal {
    Refused,
    NotRefused,
}

fn set_play_refusal(id: MediaId, play_refusal: PlayRefusal) {
    MEDIA.with(|media| {
        if let Ok(mut media) = media.try_borrow_mut()
            && let Some(m) = media.get_mut(&id)
        {
            m.play_refusal = play_refusal;
        }
    });
}

/// Calls `play()`; a rejected promise sets [`PlayRefusal::Refused`].
fn try_play(id: MediaId, media: &HtmlMediaElement) {
    let Ok(promise) = media.play() else {
        return;
    };
    wasm_bindgen_futures::spawn_local(async move {
        if let Err(e) = JsFuture::from(promise).await {
            debug!("play refused: {e:?}");
            set_play_refusal(id, PlayRefusal::Refused);
        }
    });
}

fn snapshot(media: &HtmlMediaElement) -> ElementSnapshot {
    ElementSnapshot {
        position_seconds: media.current_time(),
        duration_seconds: media.duration(),
        first_reopen_delay: first_reopen_delay(),
    }
}

/// The observation a DOM event of `media` is, if any.
fn observation(event: &str, media: &HtmlMediaElement) -> Option<ElementObservation> {
    Some(match event {
        "canplay" => ElementObservation::BecamePlayable,
        "playing" => ElementObservation::StartedPlaying,
        "waiting" => ElementObservation::Stalled,
        "seeking" => ElementObservation::StartedSeeking,
        // a pause at the end maps to nothing; `ended` maps to `ReachedEnd`
        "pause" if media.ended() => return None,
        "pause" => ElementObservation::Paused,
        "timeupdate" => ElementObservation::PositionAdvanced,
        "ended" => ElementObservation::ReachedEnd,
        "error" => ElementObservation::Failed(MediaFailureKind::from_media_error_code(
            media.error().map(|error| error.code()),
        )),
        _ => return None,
    })
}

struct HostMedia {
    media: HtmlMediaElement,
    video: HtmlVideoElement,
    url: Option<String>,
    state: MediaSourceState,
    /// The view last sent to the engine.
    reported: MediaSourceView,
    play_refusal: PlayRefusal,
    events: UnboundedSender<MediaEvent>,
    /// The element's event listeners, removed on drop.
    listeners: Vec<(&'static str, Closure<dyn FnMut()>)>,
    frame_closure: RcClosure,
    frame_callback_handle: Rc<RefCell<Option<u32>>>,
}

impl HostMedia {
    fn new(
        id: MediaId,
        url: Option<String>,
        has_video: bool,
        events: UnboundedSender<MediaEvent>,
        render_worker: Worker,
    ) -> (Self, MediaSourceCmd) {
        let document = web_sys::window().unwrap().document().unwrap();
        let video = document
            .create_element("video")
            .unwrap()
            .dyn_into::<HtmlVideoElement>()
            .unwrap();
        Self::from_element(id, video, url, has_video, events, render_worker)
    }

    /// The element `video` as media `id`, and the command that opens it.
    fn from_element(
        id: MediaId,
        video: HtmlVideoElement,
        url: Option<String>,
        has_video: bool,
        events: UnboundedSender<MediaEvent>,
        render_worker: Worker,
    ) -> (Self, MediaSourceCmd) {
        let media = video.clone().dyn_into::<HtmlMediaElement>().unwrap();
        let reopen_policy = match url {
            Some(_) => MediaReopenPolicy::Reopen,
            None => MediaReopenPolicy::Never,
        };
        let (state, cmd) = init(id, reopen_policy);

        let mut listeners = Vec::default();
        for event in [
            "canplay",
            "playing",
            "waiting",
            "seeking",
            "pause",
            "timeupdate",
            "ended",
            "error",
        ] {
            let element = media.clone();
            let closure = Closure::<dyn FnMut()>::new(move || {
                if event == "playing" {
                    set_play_refusal(id, PlayRefusal::NotRefused);
                }
                if let Some(observation) = observation(event, &element) {
                    dispatch(
                        id,
                        MediaSourceMsg::ElementObserved {
                            observation,
                            snapshot: snapshot(&element),
                        },
                    );
                }
            });
            media
                .add_event_listener_with_callback(event, closure.as_ref().unchecked_ref())
                .report();
            listeners.push((event, closure));
        }

        let mut slf = Self {
            media,
            video: video.clone(),
            url,
            reported: view(&state),
            state,
            play_refusal: PlayRefusal::NotRefused,
            events: events.clone(),
            listeners,
            frame_closure: Default::default(),
            frame_callback_handle: Default::default(),
        };

        // only a video's frames are read, which needs a CORS load: an audio stream from a server
        // without CORS headers still plays
        if has_video {
            slf.video_init(id, video, events, render_worker);
        }

        (slf, cmd)
    }

    /// Sends the parts of the view that changed since the last report.
    fn report(&mut self) {
        let id = self.state.media_id();
        let current = view(&self.state);
        let same_duration = current.duration_seconds == self.reported.duration_seconds
            || (current.duration_seconds.is_nan() && self.reported.duration_seconds.is_nan());
        if current.video_state != self.reported.video_state || !same_duration {
            debug!("media {id} state -> {:?}", current.video_state);
            let _ = self.events.unbounded_send(MediaEvent::State {
                id,
                state: current.video_state,
                duration: current.duration_seconds,
            });
        }
        if current.retry_status != self.reported.retry_status {
            let _ = self.events.unbounded_send(MediaEvent::Retry {
                id,
                status: current.retry_status,
            });
        }
        self.reported = current;
    }

    fn video_init(
        &mut self,
        id: MediaId,
        video: HtmlVideoElement,
        events: UnboundedSender<MediaEvent>,
        render_worker: Worker,
    ) {
        video.set_cross_origin(Some("anonymous"));

        // video frame callback - no wasm_bindgen for this!
        let rvc_prop = Reflect::get(&video, &"requestVideoFrameCallback".into()).unwrap();
        if rvc_prop.is_undefined() {
            panic!("no requestVideoFrameCallback");
        }
        let rvc_fn = rvc_prop.dyn_into::<Function>().unwrap();

        let callback: RcClosure = Rc::new(RefCell::new(None));
        let callback_handle: Rc<RefCell<Option<u32>>> = Rc::new(RefCell::new(None));
        let callback_clone = callback.clone();
        let handle_clone = callback_handle.clone();
        let rvc_clone = rvc_fn.clone();

        *callback.borrow_mut() = Some(Closure::wrap(Box::new({
            let video = video.clone();
            move |_now: f64, metadata: JsValue| {
                let media_time = Reflect::get(&metadata, &"mediaTime".into())
                    .ok()
                    .and_then(|mt| mt.as_f64())
                    .unwrap_or_default();
                match VideoFrame::new_with_html_video_element(&video) {
                    Ok(frame) => {
                        let visible_rect = frame.visible_rect().unwrap();
                        let (width, height) =
                            (visible_rect.width() as u32, visible_rect.height() as u32);
                        let message = js_sys::Object::new();
                        Reflect::set(&message, &"type".into(), &"VIDEO_FRAME".into()).unwrap();
                        Reflect::set(&message, &"id".into(), &JsValue::from(id)).unwrap();
                        Reflect::set(&message, &"frame".into(), &frame).unwrap();
                        let transfer = js_sys::Array::of1(&frame);
                        if render_worker
                            .post_message_with_transfer(&message, &transfer)
                            .is_ok()
                        {
                            let _ = events.unbounded_send(MediaEvent::Frame {
                                id,
                                media_time,
                                width,
                                height,
                            });
                        } else {
                            frame.close();
                        }
                    }
                    Err(e) => debug!("media {id}: no frame: {e:?}"),
                }

                if let Some(cb) = callback_clone.borrow().as_ref() {
                    if let Ok(new_handle) = rvc_clone.call1(&video, cb.as_ref().unchecked_ref()) {
                        *handle_clone.borrow_mut() = new_handle.as_f64().map(|f| f as u32);
                    }
                } else {
                    debug!("no cb - dropping");
                }
            }
        }) as Box<dyn FnMut(f64, JsValue)>));
        let initial_handle = rvc_fn
            .call1(
                &video,
                callback.borrow().as_ref().unwrap().as_ref().unchecked_ref(),
            )
            .unwrap();
        *callback_handle.borrow_mut() = initial_handle.as_f64().map(|f| f as u32);

        self.frame_closure = callback;
        self.frame_callback_handle = callback_handle;
    }
}

impl Drop for HostMedia {
    fn drop(&mut self) {
        debug!("shutdown");
        if let Some(handle) = self.frame_callback_handle.borrow_mut().take() {
            Reflect::get(&self.video, &"cancelVideoFrameCallback".into())
                .unwrap()
                .dyn_into::<Function>()
                .unwrap()
                .call1(&self.video, &JsValue::from(handle))
                .unwrap();
        }
        self.frame_closure.take();
        for (event, listener) in &self.listeners {
            self.media
                .remove_event_listener_with_callback(event, listener.as_ref().unchecked_ref())
                .report();
        }
        let _ = self.media.pause();
        if self.url.is_some() {
            detach(&self.media);
        }
        self.media.remove();
    }
}
