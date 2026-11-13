//! The page side: owns the media elements and serves [`HostCommand`]s. Runs as a task on the
//! page's event loop; nothing here may block.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

use bevy::{
    log::{debug, warn},
    platform::collections::HashMap,
};
use common::util::ReportErr;
use dcl_component::proto_components::sdk::components::VideoState;
use futures_channel::mpsc::{UnboundedSender, unbounded};
use futures_lite::StreamExt;
use js_sys::{Function, Reflect};
use wasm_bindgen::{JsCast, JsValue, closure::Closure, prelude::wasm_bindgen};
use wasm_bindgen_futures::JsFuture;
use web_sys::{HtmlMediaElement, HtmlVideoElement, VideoFrame, Worker};

use super::{HOST, HostCommand, HostHandle, MediaEvent, MediaId};
use crate::{AVCommand, MediaRetry, MediaRetryStatus, RetryStep, first_reopen_delay};

type RcClosure = Rc<RefCell<Option<Closure<dyn FnMut(f64, JsValue)>>>>;

// Defined by engine.js on the page (HLS-aware source assignment).
#[wasm_bindgen]
extern "C" {
    /// A source engine.js attached to a media element.
    type AttachedSource;

    /// Attaches `src`; `on_fatal` gets the type of an hls.js error hls.js could not recover.
    #[wasm_bindgen(js_namespace = window, js_name = setVideoSource)]
    fn set_video_source(elt: &HtmlVideoElement, src: &str, on_fatal: &Function) -> AttachedSource;

    /// Detaches the source from its element.
    #[wasm_bindgen(method)]
    fn destroy(this: &AttachedSource);

    /// The source is a live stream.
    #[wasm_bindgen(method, getter)]
    fn live(this: &AttachedSource) -> bool;

    /// The source plays through hls.js, which reports its own failures to `on_fatal`.
    #[wasm_bindgen(method, getter)]
    fn hls(this: &AttachedSource) -> bool;
}

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
        MEDIA.with(|media| {
            for m in media.borrow().values() {
                if m.source.wants_play.get() && m.source.play_refused.get() {
                    try_play(&m.media, &m.source.play_refused);
                }
            }
        });
    });
    for event in ["pointerdown", "keydown"] {
        document
            .add_event_listener_with_callback(event, on_gesture.as_ref().unchecked_ref())
            .report();
    }
    on_gesture.forget();

    wasm_bindgen_futures::spawn_local(async move {
        while let Some(command) = commands_rx.next().await {
            MEDIA.with(|media| {
                let mut media = media.borrow_mut();
                match command {
                    HostCommand::Create { id, url, has_video } => {
                        media.insert(
                            id,
                            HostMedia::new(
                                id,
                                url,
                                has_video,
                                events_tx.clone(),
                                render_worker.clone(),
                            ),
                        );
                    }
                    HostCommand::Command(id, command) => {
                        if let Some(m) = media.get(&id) {
                            m.command(command);
                        }
                    }
                    HostCommand::Drop(id) => {
                        media.remove(&id);
                    }
                }
            });
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
    let media = HostMedia::from_element(id, element, None, true, events, render_worker);
    MEDIA.with(|m| m.borrow_mut().insert(id, media));
}

/// Calls `play()`, noting a refusal (autoplay policy) for the gesture retry.
fn try_play(media: &HtmlMediaElement, refused: &Rc<Cell<bool>>) {
    let Ok(promise) = media.play() else {
        return;
    };
    let refused = refused.clone();
    wasm_bindgen_futures::spawn_local(async move {
        if let Err(e) = JsFuture::from(promise).await {
            debug!("play refused: {e:?}");
            refused.set(true);
        }
    });
}

/// The kind of the element's current error.
fn media_error_label(media: &HtmlMediaElement) -> &'static str {
    match media.error().map(|error| error.code()) {
        Some(1) => "aborted",
        Some(2) => "network error",
        Some(3) => "decode error",
        Some(4) => "source not supported",
        _ => "media error",
    }
}

/// The kind of an hls.js error type.
fn hls_error_label(kind: Option<&str>) -> &'static str {
    match kind {
        Some("networkError") => "hls network error",
        Some("mediaError") => "hls media error",
        _ => "hls error",
    }
}

/// A media element's source: `url` attached through engine.js, reopened after a failure on
/// [`MediaRetry`]'s schedule. Without a url (a placeholder, an adopted element) a failure is
/// final.
struct Source {
    id: MediaId,
    media: HtmlMediaElement,
    video: HtmlVideoElement,
    url: Option<String>,
    events: UnboundedSender<MediaEvent>,
    render_worker: Worker,
    /// The engine's last word was play (cleared when the media ends).
    wants_play: Cell<bool>,
    /// The last `play()` was refused.
    play_refused: Rc<Cell<bool>>,
    attached: RefCell<Option<AttachedSource>>,
    /// The attached source's `on_fatal`.
    on_fatal: RefCell<Option<Closure<dyn FnMut(JsValue)>>>,
    retry: Cell<MediaRetry>,
    last_error: Cell<Option<&'static str>>,
    /// Failed and not playable since: the element's own states are not reported.
    failed: Cell<bool>,
    /// A reopen a pause cancelled or a failure while paused, done on the next play.
    reopen_on_play: Cell<bool>,
    /// Where a reopened source that is not live resumes.
    resume_at: Cell<Option<f64>>,
    /// The scheduled reopen's timeout handle.
    timeout: Cell<Option<i32>>,
    /// The last scheduled reopen's callback, kept until the next one or the drop.
    timer: RefCell<Option<Closure<dyn FnMut()>>>,
}

impl Source {
    fn send_state(&self, state: VideoState) {
        debug!("media {} state -> {state:?}", self.id);
        let _ = self.events.unbounded_send(MediaEvent::State {
            id: self.id,
            state,
            duration: self.media.duration(),
        });
    }

    fn send_retry(&self, next_delay: Option<Duration>) {
        let retry = self.retry.get();
        let _ = self.events.unbounded_send(MediaEvent::Retry {
            id: self.id,
            status: MediaRetryStatus {
                attempt: retry.attempts(),
                next_delay,
                gave_up: retry.gave_up(),
                last_error: self.last_error.get(),
            },
        });
    }

    /// Attaches `url` to the element.
    fn open(self: &Rc<Self>) {
        let Some(url) = &self.url else {
            return;
        };
        let source = Rc::downgrade(self);
        let on_fatal = Closure::<dyn FnMut(JsValue)>::new(move |kind: JsValue| {
            if let Some(source) = source.upgrade() {
                source.fail(hls_error_label(kind.as_string().as_deref()));
            }
        });
        let attached = set_video_source(&self.video, url, on_fatal.as_ref().unchecked_ref());
        *self.attached.borrow_mut() = Some(attached);
        *self.on_fatal.borrow_mut() = Some(on_fatal);
    }

    /// Detaches the attached source.
    fn detach(&self) {
        if let Some(attached) = self.attached.borrow_mut().take() {
            attached.destroy();
        }
        self.on_fatal.borrow_mut().take();
    }

    fn is_live(&self) -> bool {
        self.attached
            .borrow()
            .as_ref()
            .is_some_and(|attached| attached.live())
    }

    fn uses_hls(&self) -> bool {
        self.attached
            .borrow()
            .as_ref()
            .is_some_and(|attached| attached.hls())
    }

    /// A failure of the attached source: schedules its reopen, or gives up. Failures while a
    /// reopen is scheduled or after giving up are ignored; one while paused waits for the next
    /// play.
    fn fail(self: &Rc<Self>, error: &'static str) {
        if self.url.is_none() {
            self.send_state(VideoState::VsError);
            return;
        }
        let mut retry = self.retry.get();
        if retry.gave_up() || self.timeout.get().is_some() {
            return;
        }
        self.failed.set(true);
        self.last_error.set(Some(error));
        if !self.wants_play.get() {
            debug!("media {}: {error} while paused; reopen on play", self.id);
            self.reopen_on_play.set(true);
            self.send_retry(None);
            return;
        }
        let step = retry.fail(first_reopen_delay());
        self.retry.set(retry);
        match step {
            RetryStep::Reopen { attempt, delay } => {
                debug!("media {}: {error}; reopen {attempt} in {delay:?}", self.id);
                self.send_state(VideoState::VsLoading);
                self.schedule_reopen(delay);
                self.send_retry(Some(delay));
            }
            RetryStep::GiveUp => {
                warn!("media {}: {error}; giving up", self.id);
                self.send_state(VideoState::VsError);
                self.send_retry(None);
                self.clear_screen();
            }
        }
    }

    /// The source can play: a scheduled reopen is cancelled and the schedule starts again.
    fn opened(&self) {
        self.failed.set(false);
        self.reopen_on_play.set(false);
        self.cancel_reopen();
        if let Some(time) = self.resume_at.take() {
            self.media.set_current_time(time);
        }
        let mut retry = self.retry.get();
        if retry.attempts() > 0 {
            retry.opened();
            self.retry.set(retry);
            self.send_retry(None);
        }
    }

    fn schedule_reopen(self: &Rc<Self>, delay: Duration) {
        let source = Rc::downgrade(self);
        let callback = Closure::<dyn FnMut()>::new(move || {
            if let Some(source) = source.upgrade() {
                source.timeout.set(None);
                source.reopen();
            }
        });
        let Some(window) = web_sys::window() else {
            return;
        };
        match window.set_timeout_with_callback_and_timeout_and_arguments_0(
            callback.as_ref().unchecked_ref(),
            delay.as_millis().min(i32::MAX as u128) as i32,
        ) {
            Ok(handle) => {
                self.timeout.set(Some(handle));
                *self.timer.borrow_mut() = Some(callback);
            }
            Err(e) => warn!("media {}: cannot schedule a reopen: {e:?}", self.id),
        }
    }

    /// Cancels the scheduled reopen; true if one was scheduled.
    fn cancel_reopen(&self) -> bool {
        let Some(handle) = self.timeout.take() else {
            return false;
        };
        if let Some(window) = web_sys::window() {
            window.clear_timeout_with_handle(handle);
        }
        true
    }

    fn reopen(self: &Rc<Self>) {
        debug!("media {}: reopening", self.id);
        self.reopen_on_play.set(false);
        let time = self.media.current_time();
        if !self.is_live() && time > 0.0 {
            self.resume_at.set(Some(time));
        }
        self.detach();
        self.open();
    }

    /// Shows the cleared frame in place of the last one.
    fn clear_screen(&self) {
        let message = js_sys::Object::new();
        Reflect::set(&message, &"type".into(), &"VIDEO_CLEAR".into()).unwrap();
        Reflect::set(&message, &"id".into(), &JsValue::from(self.id)).unwrap();
        self.render_worker.post_message(&message).report();
    }

    /// Cancels the scheduled reopen and detaches the source.
    fn shutdown(&self) {
        self.cancel_reopen();
        self.timer.borrow_mut().take();
        self.detach();
    }
}

struct HostMedia {
    media: HtmlMediaElement,
    video: HtmlVideoElement,
    source: Rc<Source>,
    _closures: Vec<Closure<dyn FnMut()>>,
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
    ) -> Self {
        let document = web_sys::window().unwrap().document().unwrap();
        let video = document
            .create_element("video")
            .unwrap()
            .dyn_into::<HtmlVideoElement>()
            .unwrap();
        Self::from_element(id, video, url, has_video, events, render_worker)
    }

    fn from_element(
        id: MediaId,
        video: HtmlVideoElement,
        url: Option<String>,
        has_video: bool,
        events: UnboundedSender<MediaEvent>,
        render_worker: Worker,
    ) -> Self {
        let media = video.clone().dyn_into::<HtmlMediaElement>().unwrap();
        let source = Rc::new(Source {
            id,
            media: media.clone(),
            video: video.clone(),
            url,
            events: events.clone(),
            render_worker: render_worker.clone(),
            wants_play: Cell::new(false),
            play_refused: Rc::new(Cell::new(false)),
            attached: RefCell::new(None),
            on_fatal: RefCell::new(None),
            retry: Cell::new(MediaRetry::default()),
            last_error: Cell::new(None),
            failed: Cell::new(false),
            reopen_on_play: Cell::new(false),
            resume_at: Cell::new(None),
            timeout: Cell::new(None),
            timer: RefCell::new(None),
        });

        let mut closures = Vec::default();
        let mut register = |handler: Box<dyn FnMut()>| -> Function {
            let closure = Closure::wrap(handler);
            let function = closure.as_ref().unchecked_ref::<Function>().clone();
            closures.push(closure);
            function
        };
        let state = |new_state: VideoState| -> Box<dyn FnMut()> {
            let source = source.clone();
            Box::new(move || {
                // a reopen's own states stay hidden until it can play
                if source.failed.get() && new_state != VideoState::VsReady {
                    return;
                }
                // the end's own handler reports it
                if new_state == VideoState::VsPaused && source.media.ended() {
                    return;
                }
                if new_state == VideoState::VsReady {
                    source.opened();
                }
                source.send_state(new_state);
                match new_state {
                    // a `play()` issued before the source could play takes effect now
                    VideoState::VsReady if source.wants_play.get() && source.media.paused() => {
                        try_play(&source.media, &source.play_refused);
                    }
                    VideoState::VsPlaying => source.play_refused.set(false),
                    _ => (),
                }
            })
        };
        media.set_oncanplay(Some(&register(state(VideoState::VsReady))));
        media.set_onwaiting(Some(&register(state(VideoState::VsBuffering))));
        media.set_onseeking(Some(&register(state(VideoState::VsSeeking))));
        media.set_onplaying(Some(&register(state(VideoState::VsPlaying))));
        media.set_onpause(Some(&register(state(VideoState::VsPaused))));
        media.set_onerror(Some(&register({
            let source = source.clone();
            Box::new(move || {
                // hls.js recovers its own errors first and reports the rest to `on_fatal`
                if !source.uses_hls() {
                    source.fail(media_error_label(&source.media));
                }
            })
        })));
        media.set_onended(Some(&register({
            let source = source.clone();
            Box::new(move || {
                if source.failed.get() {
                    return;
                }
                // a live stream only ends when its feed drops
                if source.is_live() {
                    source.fail("live stream ended");
                    return;
                }
                source.send_state(VideoState::VsPaused);
                // `play()` on an ended element restarts it: only the engine may ask
                source.wants_play.set(false);
                source.clear_screen();
            })
        })));

        let mut slf = Self {
            media,
            video: video.clone(),
            source,
            _closures: closures,
            frame_closure: Default::default(),
            frame_callback_handle: Default::default(),
        };

        // only a video's frames are read, which needs a CORS load: an audio stream from a server
        // without CORS headers still plays
        if has_video {
            slf.video_init(id, video, events, render_worker);
        }
        slf.source.open();

        slf
    }

    fn command(&self, command: AVCommand) {
        match command {
            AVCommand::Play => {
                self.source.wants_play.set(true);
                if self.source.reopen_on_play.get() {
                    self.source.reopen();
                } else {
                    try_play(&self.media, &self.source.play_refused);
                }
            }
            AVCommand::Pause => {
                self.source.wants_play.set(false);
                self.media.pause().report();
                // a pause cancels a scheduled or loading reopen; the next play reopens at once
                self.source.cancel_reopen();
                if self.source.failed.get() && !self.source.retry.get().gave_up() {
                    let mut retry = self.source.retry.get();
                    retry.cancel();
                    self.source.retry.set(retry);
                    self.source.reopen_on_play.set(true);
                    self.source.send_state(VideoState::VsPaused);
                    self.source.send_retry(None);
                }
            }
            AVCommand::Repeat(looping) => self.media.set_loop(looping),
            AVCommand::Seek(time) => self.media.set_current_time(time),
            AVCommand::Volume(volume) => self.media.set_volume(volume.clamp(0.0, 1.0) as f64),
            // dispose arrives as `HostCommand::Drop`
            AVCommand::Dispose => (),
        }
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
        self.source.shutdown();
        self.media.set_oncanplay(None);
        self.media.set_onerror(None);
        self.media.set_onwaiting(None);
        self.media.set_onseeking(None);
        self.media.set_onplaying(None);
        self.media.set_onpause(None);
        self.media.set_onended(None);
        let _ = self.media.pause();
        self.media.remove();
    }
}
