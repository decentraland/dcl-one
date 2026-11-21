//! What the automation commands' replies and arguments share
//! (docs/adr/2026-11-17-console-command-conventions.md): the `screen` a coordinate is read
//! against, normalized coordinates, points, the measured frame time a command counts seconds in,
//! and refusals that name their cause.

use bevy::{
    diagnostic::{Diagnostic, DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    prelude::*,
    window::PrimaryWindow,
};
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
    fn of(window: &Window) -> Self {
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

    /// A normalized point (0 to 1 from the top left) in logical pixels, kept inside the window:
    /// 1 is its last pixel, not the one past it, where nothing would take a click.
    pub fn to_pixels(&self, normalized: Vec2) -> Vec2 {
        let size = Vec2::new(self.width, self.height);
        (normalized * size).min(size - 1.0).max(Vec2::ZERO)
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

/// Hundredths, finer than anything a click or a scene can tell, so replies stay short. Never a
/// negative zero (adding 0 makes it a plain one), which JSON would print as `-0.0`.
pub fn round(v: f32) -> f64 {
    (f64::from(v) * 100.0).round() / 100.0 + 0.0
}

/// Ten-thousandths, for normalized coordinates: a hundredth of a pixel on a 10000-pixel screen.
pub fn round_fine(v: f32) -> f64 {
    (f64::from(v) * 10000.0).round() / 10000.0
}

/// A point or a vector as `{"x", "y", "z"}`, in hundredths.
pub fn xyz([x, y, z]: [f32; 3]) -> Value {
    json!({ "x": round(x), "y": round(y), "z": round(z) })
}

/// How long a frame takes, as `FrameTimeDiagnosticsPlugin` smooths it, for a command that counts
/// seconds in frames. A sixtieth of a second until a frame has been measured.
#[derive(Clone, Copy, Debug, PartialEq)]
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
    pub fn measured(diagnostics: &DiagnosticsStore) -> Self {
        diagnostics
            .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
            .and_then(Diagnostic::smoothed)
            .map(|milliseconds| (milliseconds / 1000.0) as f32)
            .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
            .map_or_else(Self::default, |frame_seconds| Self { frame_seconds })
    }

    /// The frames `seconds` last, at least one for a positive duration.
    pub fn frames_in(&self, seconds: f32) -> u32 {
        if seconds <= 0.0 {
            return 0;
        }
        (seconds / self.frame_seconds).round().max(1.0) as u32
    }
}

/// A refusal: the command did nothing, and says why and what stood in the way. Sent as the failed
/// reply's text, `{"refused": reason, "cause": {...}}`, the cause naming the input, holder or
/// entity that refused.
pub fn refusal(reason: impl Into<String>, cause: Option<Value>) -> String {
    let mut reply = json!({ "refused": reason.into() });
    if let Some(cause) = cause {
        reply["cause"] = cause;
    }
    reply.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounding_never_gives_a_negative_zero() {
        assert!(round(-0.0).is_sign_positive());
        assert!(round(-0.001).is_sign_positive());
        assert_eq!(round(-1.234), -1.23);
        assert_eq!(
            xyz([1.006, -0.001, -2.5]).to_string(),
            r#"{"x":1.01,"y":0.0,"z":-2.5}"#
        );
    }

    #[test]
    fn seconds_count_in_frames_at_the_measured_rate() {
        let rate = FrameRate {
            frame_seconds: 1.0 / 30.0,
        };
        assert_eq!(rate.frames_in(2.0), 60);
        assert_eq!(rate.frames_in(0.001), 1);
        assert_eq!(rate.frames_in(0.0), 0);
        // nothing measured yet: a sixtieth of a second
        assert_eq!(
            FrameRate::measured(&DiagnosticsStore::default()),
            FrameRate::default()
        );
    }

    #[test]
    fn a_refusal_names_its_cause() {
        let text = refusal(
            "the HUD holds the input",
            Some(json!({"input": "IaForward"})),
        );
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["refused"], "the HUD holds the input");
        assert_eq!(value["cause"]["input"], "IaForward");
        assert_eq!(refusal("busy", None), r#"{"refused":"busy"}"#);
    }

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
        assert_eq!(SCREEN.to_pixels(Vec2::ONE), Vec2::new(1279.0, 719.0));
        assert_eq!(SCREEN.to_pixels(Vec2::ZERO), Vec2::ZERO);
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
}
