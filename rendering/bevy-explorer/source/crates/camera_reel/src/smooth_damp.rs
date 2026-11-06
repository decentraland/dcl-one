//! Critically damped springs: `smooth_damp` eases a value to a target over roughly
//! `smooth_time` seconds without overshooting, and `follow_damp` is the exponential lag a follow
//! camera applies, closing 99% of the gap in `damp_time` seconds.

/// Moves `current` toward `target`, updating `velocity`, over about `smooth_time` seconds.
pub fn smooth_damp(
    current: f32,
    target: f32,
    velocity: &mut f32,
    smooth_time: f32,
    dt: f32,
) -> f32 {
    let smooth_time = smooth_time.max(0.0001);
    let omega = 2.0 / smooth_time;
    let x = omega * dt;
    let exp = 1.0 / (1.0 + x + 0.48 * x * x + 0.235 * x * x * x);
    let change = current - target;
    let temp = (*velocity + omega * change) * dt;
    *velocity = (*velocity - omega * temp) * exp;
    let mut output = target + (change + temp) * exp;
    // the spring must not pass the target
    if (target - current > 0.0) == (output > target) {
        output = target;
        *velocity = 0.0;
    }
    output
}

/// The fraction of a gap a follow camera closes in `dt` when it closes 99% of it in `damp_time`.
pub fn follow_damp_fraction(damp_time: f32, dt: f32) -> f32 {
    if damp_time <= 0.0 {
        return 1.0;
    }
    const LOG_NEGLIGIBLE_RESIDUAL: f32 = -4.605_170_2; // ln(0.01)
    1.0 - (dt * LOG_NEGLIGIBLE_RESIDUAL / damp_time).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smooth_damp_settles_without_overshoot() {
        let (mut current, mut velocity) = (0.0f32, 0.0f32);
        let mut max = 0.0f32;
        for _ in 0..200 {
            current = smooth_damp(current, 10.0, &mut velocity, 0.1, 1.0 / 60.0);
            max = max.max(current);
        }
        assert!((current - 10.0).abs() < 1e-3, "{current}");
        assert!(max <= 10.0 + 1e-3, "overshot to {max}");
    }

    #[test]
    fn smooth_damp_reaches_most_of_the_way_in_smooth_time() {
        let (mut current, mut velocity) = (0.0f32, 0.0f32);
        let dt = 1.0 / 60.0;
        let steps = (0.1 / dt) as usize;
        for _ in 0..steps {
            current = smooth_damp(current, 1.0, &mut velocity, 0.1, dt);
        }
        assert!(current > 0.5 && current < 1.0, "{current}");
    }

    #[test]
    fn a_zero_length_frame_keeps_the_velocity_finite() {
        let mut velocity = 0.0f32;
        let out = smooth_damp(1.0, 1.0, &mut velocity, 0.1, 0.0);
        assert_eq!(out, 1.0);
        assert!(velocity.is_finite());
    }

    #[test]
    fn follow_damp_closes_most_of_the_gap_within_damp_time() {
        let mut gap = 1.0f32;
        for _ in 0..60 {
            gap -= gap * follow_damp_fraction(1.0, 1.0 / 60.0);
        }
        assert!(gap < 0.02, "{gap}");
        assert_eq!(follow_damp_fraction(0.0, 0.016), 1.0);
    }
}
