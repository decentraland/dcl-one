//! The chat bubble's transitions, apart from the ECS so a test can drive them through time.

use bevy::math::Vec2;

use super::layout::{BubbleProgress, COMMUNITY_ROW_HEIGHT, DM_BADGE_MAX_WIDTH};

/// How long every transition takes, in seconds.
pub const TRANSITION: f64 = 0.3;
/// The message container starts resizing this long after the rest, in seconds.
pub const CONTAINER_DELAY: f64 = 0.1;
/// How long a message stays up from its arrival, in seconds; a new message starts the count
/// again.
pub const BUBBLE_LIFETIME: f64 = 5.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Curve {
    /// The CSS `ease` curve.
    Ease,
    /// Overshoots the target, then settles.
    OutBack,
    OutCubic,
}

impl Curve {
    pub fn at(self, t: f32) -> f32 {
        if t <= 0.0 {
            return 0.0;
        }
        if t >= 1.0 {
            return 1.0;
        }
        match self {
            Curve::Ease => cubic_bezier(0.25, 0.1, 0.25, 1.0, t),
            Curve::OutBack => {
                const OVERSHOOT: f32 = 1.70158;
                let u = t - 1.0;
                1.0 + (OVERSHOOT + 1.0) * u * u * u + OVERSHOOT * u * u
            }
            Curve::OutCubic => 1.0 - (1.0 - t).powi(3),
        }
    }
}

/// The y of the cubic bezier from (0, 0) to (1, 1) with control points (x1, y1) and (x2, y2),
/// at `x`.
fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, x: f32) -> f32 {
    let bezier = |a: f32, b: f32, s: f32| {
        let inv = 1.0 - s;
        3.0 * inv * inv * s * a + 3.0 * inv * s * s * b + s * s * s
    };
    // x is monotonic in the curve parameter
    let (mut low, mut high) = (0.0, 1.0);
    for _ in 0..24 {
        let mid = (low + high) * 0.5;
        if bezier(x1, x2, mid) < x {
            low = mid;
        } else {
            high = mid;
        }
    }
    bezier(y1, y2, (low + high) * 0.5)
}

/// A value moving to a target over [`TRANSITION`].
#[derive(Clone, Copy, Debug)]
pub struct Eased {
    from: f32,
    to: f32,
    start: f64,
    curve: Curve,
}

impl Eased {
    pub fn settled_at(value: f32) -> Self {
        Self {
            from: value,
            to: value,
            start: f64::NEG_INFINITY,
            curve: Curve::Ease,
        }
    }

    pub fn value(&self, now: f64) -> f32 {
        let t = ((now - self.start) / TRANSITION) as f32;
        self.from + (self.to - self.from) * self.curve.at(t)
    }

    pub fn target(&self) -> f32 {
        self.to
    }

    pub fn settled(&self, now: f64) -> bool {
        now - self.start >= TRANSITION
    }

    /// Head for `to` from wherever the value is now, starting after `delay`. A transition
    /// already heading there carries on.
    pub fn retarget(&mut self, now: f64, to: f32, curve: Curve, delay: f64) {
        if to == self.to {
            return;
        }
        *self = Self {
            from: self.value(now),
            to,
            start: now + delay,
            curve,
        };
    }
}

/// What a message asks of the bubble.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BubbleTarget {
    /// The message container's size showing the whole message.
    pub container: Vec2,
    pub mention: bool,
    pub community: bool,
    pub dm: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BubblePhase {
    Showing,
    Hiding,
    /// Back to the idle tag: the message can be dropped.
    Hidden,
}

/// A tag's bubble: every animated value, and when the message comes down.
#[derive(Clone, Debug)]
pub struct BubbleAnimation {
    padding: Eased,
    tail: Eased,
    mention: Eased,
    community: Eased,
    dm: Eased,
    width: Eased,
    height: Eased,
    hide_at: f64,
    hiding: bool,
}

impl BubbleAnimation {
    /// The idle tag, about to show `target` until `hide_at`.
    pub fn show(now: f64, target: BubbleTarget, hide_at: f64) -> Self {
        let mut animation = Self {
            padding: Eased::settled_at(0.0),
            tail: Eased::settled_at(0.0),
            mention: Eased::settled_at(0.0),
            community: Eased::settled_at(0.0),
            dm: Eased::settled_at(0.0),
            width: Eased::settled_at(0.0),
            height: Eased::settled_at(0.0),
            hide_at: 0.0,
            hiding: false,
        };
        animation.retarget(now, target, hide_at);
        animation
    }

    /// Show `target` until `hide_at` from wherever the bubble is: over a message still up, or
    /// one coming down.
    pub fn retarget(&mut self, now: f64, target: BubbleTarget, hide_at: f64) {
        self.hiding = false;
        self.hide_at = hide_at;
        self.padding.retarget(now, 1.0, Curve::OutBack, 0.0);
        self.tail.retarget(now, 1.0, Curve::Ease, 0.0);
        self.mention
            .retarget(now, f32::from(target.mention), Curve::Ease, 0.0);
        let community = if target.community {
            COMMUNITY_ROW_HEIGHT
        } else {
            0.0
        };
        self.community.retarget(now, community, Curve::Ease, 0.0);
        let dm = if target.dm { DM_BADGE_MAX_WIDTH } else { 0.0 };
        self.dm.retarget(now, dm, Curve::Ease, 0.0);
        self.resize(now, target.container);
    }

    /// The message was replaced by one of another size (a translation); its time up is kept.
    /// A bubble already coming down keeps coming down: it is not opened again for words that
    /// would be gone before they could be read.
    pub fn resize(&mut self, now: f64, container: Vec2) {
        if self.hiding {
            return;
        }
        // an overshoot while shrinking would take the size below its target, to less than nothing
        let shrinking = container.x < self.width.target() || container.y < self.height.target();
        let curve = if shrinking {
            Curve::OutCubic
        } else {
            Curve::OutBack
        };
        self.width
            .retarget(now, container.x, curve, CONTAINER_DELAY);
        self.height
            .retarget(now, container.y, curve, CONTAINER_DELAY);
    }

    /// Advance to `now`: the message comes down once its time is up.
    pub fn tick(&mut self, now: f64) -> BubblePhase {
        if !self.hiding && now >= self.hide_at {
            self.hide(now);
        }
        if !self.hiding {
            BubblePhase::Showing
        } else if self.settled(now) {
            BubblePhase::Hidden
        } else {
            BubblePhase::Hiding
        }
    }

    fn hide(&mut self, now: f64) {
        self.hiding = true;
        self.padding.retarget(now, 0.0, Curve::OutBack, 0.0);
        for value in [
            &mut self.tail,
            &mut self.mention,
            &mut self.community,
            &mut self.dm,
        ] {
            value.retarget(now, 0.0, Curve::Ease, 0.0);
        }
        for value in [&mut self.width, &mut self.height] {
            value.retarget(now, 0.0, Curve::OutCubic, CONTAINER_DELAY);
        }
    }

    /// Nothing is moving: the tag's mesh stays as it is until the next change.
    pub fn settled(&self, now: f64) -> bool {
        [
            &self.padding,
            &self.tail,
            &self.mention,
            &self.community,
            &self.dm,
            &self.width,
            &self.height,
        ]
        .into_iter()
        .all(|value| value.settled(now))
    }

    pub fn progress(&self, now: f64) -> BubbleProgress {
        BubbleProgress {
            padding: self.padding.value(now),
            tail: self.tail.value(now),
            mention: self.mention.value(now),
            community_height: self.community.value(now),
            dm_max_width: self.dm.value(now),
            // an overshoot past nothing draws nothing
            container: Vec2::new(self.width.value(now), self.height.value(now)).max(Vec2::ZERO),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TARGET: BubbleTarget = BubbleTarget {
        container: Vec2::new(120.0, 40.0),
        mention: false,
        community: false,
        dm: false,
    };

    #[test]
    fn curves_start_and_end_on_their_values() {
        for curve in [Curve::Ease, Curve::OutBack, Curve::OutCubic] {
            assert!(curve.at(0.0).abs() < 1e-4, "{curve:?}");
            assert!((curve.at(1.0) - 1.0).abs() < 1e-4, "{curve:?}");
        }
        // the css `ease` curve at its midpoint
        assert!((Curve::Ease.at(0.5) - 0.8024).abs() < 1e-3);
        assert!(Curve::OutBack.at(0.6) > 1.0);
        assert!((Curve::OutCubic.at(0.5) - 0.875).abs() < 1e-6);
    }

    #[test]
    fn a_message_shows_then_comes_down_after_its_lifetime() {
        let mut bubble = BubbleAnimation::show(10.0, TARGET, 10.0 + BUBBLE_LIFETIME);
        assert_eq!(bubble.tick(10.0), BubblePhase::Showing);
        assert_eq!(bubble.progress(10.0).padding, 0.0);

        let shown = 10.0 + CONTAINER_DELAY + TRANSITION;
        assert_eq!(bubble.tick(shown), BubblePhase::Showing);
        assert!(bubble.settled(shown));
        let progress = bubble.progress(shown);
        assert_eq!(progress.padding, 1.0);
        assert_eq!(progress.tail, 1.0);
        assert_eq!(progress.container, TARGET.container);

        assert_eq!(
            bubble.tick(10.0 + BUBBLE_LIFETIME - 0.01),
            BubblePhase::Showing
        );
        assert_eq!(bubble.tick(10.0 + BUBBLE_LIFETIME), BubblePhase::Hiding);
        let hidden = 10.0 + BUBBLE_LIFETIME + CONTAINER_DELAY + TRANSITION;
        assert_eq!(bubble.tick(hidden), BubblePhase::Hidden);
        let progress = bubble.progress(hidden);
        assert_eq!(progress.padding, 0.0);
        assert_eq!(progress.container, Vec2::ZERO);
    }

    #[test]
    fn the_container_waits_before_resizing() {
        let bubble = BubbleAnimation::show(0.0, TARGET, BUBBLE_LIFETIME);
        assert_eq!(bubble.progress(CONTAINER_DELAY).container, Vec2::ZERO);
        assert!(bubble.progress(CONTAINER_DELAY + 0.05).container.x > 0.0);
        assert!(bubble.progress(0.05).padding > 0.0);
    }

    #[test]
    fn a_new_message_restarts_the_lifetime_from_the_current_size() {
        let mut bubble = BubbleAnimation::show(0.0, TARGET, BUBBLE_LIFETIME);
        bubble.tick(4.0);
        let larger = BubbleTarget {
            container: Vec2::new(300.0, 80.0),
            mention: true,
            ..TARGET
        };
        bubble.retarget(4.0, larger, 4.0 + BUBBLE_LIFETIME);
        // from the size it had, not from nothing
        assert_eq!(bubble.progress(4.0).container, TARGET.container);
        assert_eq!(bubble.progress(4.0).padding, 1.0);
        assert_eq!(bubble.tick(5.0 + 0.5), BubblePhase::Showing);
        assert_eq!(bubble.progress(5.5).container, larger.container);
        assert_eq!(bubble.progress(5.5).mention, 1.0);
        assert_eq!(bubble.tick(4.0 + BUBBLE_LIFETIME), BubblePhase::Hiding);
    }

    #[test]
    fn a_message_arriving_while_the_last_comes_down_shows_from_where_it_was() {
        let mut bubble = BubbleAnimation::show(0.0, TARGET, BUBBLE_LIFETIME);
        let mid_hide = BUBBLE_LIFETIME + 0.15;
        assert_eq!(bubble.tick(BUBBLE_LIFETIME), BubblePhase::Hiding);
        assert_eq!(bubble.tick(mid_hide), BubblePhase::Hiding);
        let before = bubble.progress(mid_hide);
        assert!(before.tail > 0.0 && before.tail < 1.0);

        bubble.retarget(mid_hide, TARGET, mid_hide + BUBBLE_LIFETIME);
        assert_eq!(bubble.tick(mid_hide), BubblePhase::Showing);
        assert_eq!(bubble.progress(mid_hide).tail, before.tail);
        assert_eq!(bubble.progress(mid_hide).padding, before.padding);
        assert_eq!(bubble.progress(mid_hide + 1.0).container, TARGET.container);
    }

    #[test]
    fn shrinking_never_overshoots_below_the_target() {
        let mut bubble = BubbleAnimation::show(0.0, TARGET, BUBBLE_LIFETIME);
        bubble.tick(1.0);
        let smaller = Vec2::new(60.0, 20.0);
        bubble.resize(1.0, smaller);
        let mut now = 1.0;
        while now < 1.6 {
            let container = bubble.progress(now).container;
            assert!(container.x >= smaller.x - 1e-3 && container.y >= smaller.y - 1e-3);
            now += 0.01;
        }
        // the lifetime is the first message's
        assert_eq!(bubble.tick(BUBBLE_LIFETIME), BubblePhase::Hiding);
    }

    #[test]
    fn other_words_while_the_bubble_opens_resize_it_and_keep_its_time() {
        let mut bubble = BubbleAnimation::show(0.0, TARGET, BUBBLE_LIFETIME);
        let mid_show = 0.15;
        assert_eq!(bubble.tick(mid_show), BubblePhase::Showing);
        let before = bubble.progress(mid_show);
        let wider = Vec2::new(200.0, 60.0);
        bubble.resize(mid_show, wider);
        // from the size it had got to, and nothing else starts over
        assert_eq!(bubble.progress(mid_show).container, before.container);
        assert_eq!(bubble.progress(mid_show).padding, before.padding);
        assert_eq!(bubble.progress(1.0).container, wider);
        assert_eq!(bubble.progress(1.0).padding, 1.0);
        assert_eq!(bubble.tick(BUBBLE_LIFETIME - 0.01), BubblePhase::Showing);
        assert_eq!(bubble.tick(BUBBLE_LIFETIME), BubblePhase::Hiding);
    }

    #[test]
    fn other_words_while_the_bubble_comes_down_leave_it_coming_down() {
        let mut bubble = BubbleAnimation::show(0.0, TARGET, BUBBLE_LIFETIME);
        let mid_hide = BUBBLE_LIFETIME + 0.15;
        assert_eq!(bubble.tick(BUBBLE_LIFETIME), BubblePhase::Hiding);
        assert_eq!(bubble.tick(mid_hide), BubblePhase::Hiding);
        let before = bubble.progress(mid_hide);

        bubble.resize(mid_hide, Vec2::new(300.0, 80.0));
        assert_eq!(bubble.tick(mid_hide), BubblePhase::Hiding);
        assert_eq!(bubble.progress(mid_hide), before);
        // it never grows back, and is down when it would have been
        let mut now = mid_hide;
        let mut last = before.container;
        while now < BUBBLE_LIFETIME + CONTAINER_DELAY + TRANSITION {
            let container = bubble.progress(now).container;
            assert!(container.x <= last.x + 1e-3 && container.y <= last.y + 1e-3);
            last = container;
            now += 0.01;
        }
        let hidden = BUBBLE_LIFETIME + CONTAINER_DELAY + TRANSITION + 1e-6;
        assert_eq!(bubble.tick(hidden), BubblePhase::Hidden);
        assert_eq!(bubble.progress(hidden).container, Vec2::ZERO);
    }

    #[test]
    fn a_message_first_seen_late_has_only_the_rest_of_its_time() {
        // it came at 0 and its tag shows it from 3
        let mut bubble = BubbleAnimation::show(3.0, TARGET, BUBBLE_LIFETIME);
        assert_eq!(bubble.tick(3.0), BubblePhase::Showing);
        assert_eq!(bubble.progress(3.0).padding, 0.0);
        assert_eq!(bubble.tick(BUBBLE_LIFETIME), BubblePhase::Hiding);
    }
}
