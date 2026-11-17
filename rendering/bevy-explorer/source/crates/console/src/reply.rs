//! What the automation commands' replies and arguments share
//! (docs/adr/2026-11-17-console-command-conventions.md): the `screen` a coordinate is read
//! against, normalized coordinates, a duration given in frames or in seconds, refusals that name
//! their cause, and warnings.

use bevy::{prelude::*, window::PrimaryWindow};
use serde_json::{json, Value};

/// The primary window in logical pixels, the space `/click_at`, `/entity_screen_position` and the
/// pointer ray use, and its scale: a screenshot's pixels are `scale` times as many.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Screen {
    pub width: f32,
    pub height: f32,
    pub scale: f32,
}

impl Screen {
    pub fn of(window: &Window) -> Self {
        Self {
            width: window.width(),
            height: window.height(),
            scale: window.scale_factor(),
        }
    }

    /// The primary window's, if there is one.
    pub fn primary(windows: &Query<&Window, With<PrimaryWindow>>) -> Option<Self> {
        windows.single().ok().map(Self::of)
    }

    /// `{"width", "height", "scale"}`, as every reply that carries a coordinate carries it.
    pub fn to_json(&self) -> Value {
        json!({
            "width": round(self.width),
            "height": round(self.height),
            "scale": round(self.scale),
        })
    }

    /// A normalized point (0 to 1 from the top left) in logical pixels.
    pub fn to_pixels(&self, normalized: Vec2) -> Vec2 {
        normalized * Vec2::new(self.width, self.height)
    }

    /// A point in logical pixels, normalized (0 to 1 from the top left).
    pub fn to_normalized(&self, pixels: Vec2) -> Vec2 {
        pixels / Vec2::new(self.width, self.height)
    }
}

/// A normalized point, refused outside 0 to 1.
pub fn normalized_point(x: f32, y: f32) -> Result<Vec2, String> {
    if (0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y) {
        Ok(Vec2::new(x, y))
    } else {
        Err(format!(
            "normalized coordinates run from 0 to 1 from the top left; got ({x}, {y})"
        ))
    }
}

/// Hundredths, finer than anything a click or a scene can tell, so replies stay short.
pub fn round(v: f32) -> f64 {
    (f64::from(v) * 100.0).round() / 100.0
}

/// Ten-thousandths, for normalized coordinates: a hundredth of a pixel on a 10000-pixel screen.
pub fn round_fine(v: f32) -> f64 {
    (f64::from(v) * 10000.0).round() / 10000.0
}

/// How long a gesture lasts: `--frames` as given, or `--seconds` counted in frames at the frame
/// rate [`FrameRate`] measured when the command starts. Flatten it into a command.
#[derive(clap::Args, Clone, Debug, Default, PartialEq)]
pub struct Span {
    /// how many frames
    #[arg(long, conflicts_with = "seconds")]
    pub frames: Option<u32>,
    /// how many seconds, counted in frames at the measured frame rate
    #[arg(long)]
    pub seconds: Option<f32>,
}

impl Span {
    /// The frames asked for, `None` when neither is given; a negative or non-finite number of
    /// seconds is refused. A positive number of seconds lasts at least one frame.
    pub fn frames(&self, rate: &FrameRate) -> Result<Option<u32>, String> {
        match (self.frames, self.seconds) {
            (Some(frames), _) => Ok(Some(frames)),
            (None, Some(seconds)) if !seconds.is_finite() || seconds < 0.0 => Err(format!(
                "--seconds takes a duration of 0 or more; got {seconds}"
            )),
            (None, Some(seconds)) => Ok(Some(rate.frames_in(seconds))),
            (None, None) => Ok(None),
        }
    }
}

/// The frame rate, smoothed over about the last second of frames, for turning seconds into frames.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct FrameRate {
    /// seconds a frame takes
    pub frame_seconds: f32,
}

impl Default for FrameRate {
    fn default() -> Self {
        Self {
            frame_seconds: 1.0 / 60.0,
        }
    }
}

impl FrameRate {
    pub fn frames_in(&self, seconds: f32) -> u32 {
        if seconds <= 0.0 {
            return 0;
        }
        (seconds / self.frame_seconds).round().max(1.0) as u32
    }

    pub fn per_second(&self) -> f32 {
        1.0 / self.frame_seconds
    }

    fn observe(&mut self, delta: f32) {
        // a stall (a load, a hidden tab) would skew the mean for seconds; the next frames count
        if delta > 0.0 && delta < 1.0 {
            self.frame_seconds += (delta - self.frame_seconds) * 0.05;
        }
    }
}

pub(crate) fn measure_frame_rate(time: Res<Time<Real>>, mut rate: ResMut<FrameRate>) {
    rate.observe(time.delta_secs());
}

/// A refusal: the command did nothing, and says why and what stood in the way. Sent as the failed
/// reply's text, `{"refused": reason, "cause": {...}}`, the cause naming the element, area,
/// entity or input that refused (its `crdtEntityId`, its kind, ...).
pub fn refusal(reason: impl Into<String>, cause: Option<Value>) -> String {
    let mut reply = json!({ "refused": reason.into() });
    if let Some(cause) = cause {
        reply["cause"] = cause;
    }
    reply.to_string()
}

/// Add a `warning` to a reply: the command did what it was asked, but something the caller should
/// know about it may not be what they expect (a rig limit, an entity declaring no such event).
pub fn add_warning(reply: &mut Value, warning: Option<String>) {
    if let Some(warning) = warning {
        reply["warning"] = Value::String(warning);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Screen = Screen {
        width: 1280.0,
        height: 720.0,
        scale: 2.0,
    };

    #[test]
    fn normalized_points_scale_to_the_window_and_back() {
        assert_eq!(
            SCREEN.to_pixels(Vec2::new(0.5, 0.25)),
            Vec2::new(640.0, 180.0)
        );
        assert_eq!(
            SCREEN.to_normalized(Vec2::new(640.0, 180.0)),
            Vec2::new(0.5, 0.25)
        );
        assert_eq!(
            SCREEN.to_json(),
            json!({"width": 1280.0, "height": 720.0, "scale": 2.0})
        );
    }

    #[test]
    fn a_normalized_point_off_the_window_is_refused() {
        assert_eq!(normalized_point(0.0, 1.0), Ok(Vec2::new(0.0, 1.0)));
        assert!(normalized_point(1.5, 0.5).is_err());
        assert!(normalized_point(0.5, -0.1).is_err());
        assert!(normalized_point(f32::NAN, 0.5).is_err());
    }

    #[test]
    fn seconds_count_in_frames_at_the_measured_rate() {
        let rate = FrameRate {
            frame_seconds: 1.0 / 30.0,
        };
        let span = |frames, seconds| Span { frames, seconds };
        assert_eq!(span(Some(7), None).frames(&rate), Ok(Some(7)));
        assert_eq!(span(None, Some(2.0)).frames(&rate), Ok(Some(60)));
        assert_eq!(span(None, Some(0.001)).frames(&rate), Ok(Some(1)));
        assert_eq!(span(None, Some(0.0)).frames(&rate), Ok(Some(0)));
        assert_eq!(span(None, None).frames(&rate), Ok(None));
        assert!(span(None, Some(-1.0)).frames(&rate).is_err());
    }

    #[test]
    fn the_rate_follows_frames_and_ignores_a_stall() {
        let mut rate = FrameRate::default();
        for _ in 0..200 {
            rate.observe(1.0 / 30.0);
        }
        rate.observe(5.0);
        assert!((rate.per_second() - 30.0).abs() < 0.5, "{rate:?}");
    }

    #[test]
    fn a_refusal_names_its_cause() {
        let text = refusal(
            "the camera mode is held by an area",
            Some(json!({"crdtEntityId": 512})),
        );
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["refused"], "the camera mode is held by an area");
        assert_eq!(value["cause"]["crdtEntityId"], 512);
        assert_eq!(refusal("busy", None), r#"{"refused":"busy"}"#);
    }

    #[test]
    fn a_warning_is_added_only_when_there_is_one() {
        let mut reply = json!({"ok": true});
        add_warning(&mut reply, None);
        assert!(reply.get("warning").is_none());
        add_warning(&mut reply, Some("aim error above 2 degrees".to_owned()));
        assert_eq!(reply["warning"], "aim error above 2 degrees");
    }
}
