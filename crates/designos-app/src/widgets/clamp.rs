//! `clamp` that cannot panic when the window is too small for what it places.

/// `v` limited to `lo..=hi`, for bounds that come from window geometry.
///
/// `f32::clamp` panics when `lo > hi`, which a narrow window produces (`right - width - 8` below `left + 8`).
/// Here the upper bound wins then, so a popover sticks to the window's right edge instead of crashing. A NaN bound
/// is ignored.
pub fn clamp_within(v: f32, lo: f32, hi: f32) -> f32 {
    let v = if lo.is_nan() { v } else { v.max(lo) };
    if hi.is_nan() {
        v
    } else {
        v.min(hi)
    }
}

#[cfg(test)]
mod tests {
    use super::clamp_within;

    #[test]
    fn it_clamps_like_clamp_when_the_bounds_are_in_order() {
        assert_eq!(clamp_within(5.0, 0.0, 10.0), 5.0);
        assert_eq!(clamp_within(-5.0, 0.0, 10.0), 0.0);
        assert_eq!(clamp_within(15.0, 0.0, 10.0), 10.0);
        assert_eq!(clamp_within(3.0, 3.0, 3.0), 3.0);
    }

    #[test]
    fn the_upper_bound_wins_when_the_bounds_cross() {
        assert_eq!(clamp_within(50.0, 260.0, 220.0), 220.0);
        assert_eq!(clamp_within(500.0, 260.0, 220.0), 220.0);
        assert_eq!(clamp_within(0.0, 8.0, -20.0), -20.0);
    }

    #[test]
    fn a_nan_bound_is_ignored_and_infinite_bounds_are_fine() {
        assert_eq!(clamp_within(5.0, f32::NAN, 10.0), 5.0);
        assert_eq!(clamp_within(50.0, f32::NAN, 10.0), 10.0);
        assert_eq!(clamp_within(5.0, 0.0, f32::NAN), 5.0);
        assert_eq!(clamp_within(5.0, f32::NEG_INFINITY, f32::INFINITY), 5.0);
        assert_eq!(clamp_within(5.0, f32::INFINITY, f32::INFINITY), f32::INFINITY);
    }
}
