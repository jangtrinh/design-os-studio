//! Easing curves for prototype transitions (re-exported from `designos_doc`).

pub use designos_doc::{ease, effective_duration_ms, preset_name, PRESETS};

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{control_points, cubic_bezier, spring_at, spring_settle_secs, Easing, EasingType, DEFAULT_BEZIER};

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn endpoints_are_fixed() {
        for (name, e) in PRESETS {
            assert!(close(ease(e, 0.0), 0.0), "{name} start");
            assert!(close(ease(e, 1.0), 1.0), "{name} end");
        }
    }

    #[test]
    fn linear_is_identity_and_clamps() {
        assert!(close(ease(&Easing::Linear, 0.3), 0.3));
        assert!(close(ease(&Easing::Linear, -1.0), 0.0));
        assert!(close(ease(&Easing::Linear, 2.0), 1.0));
    }

    #[test]
    fn standard_curves_have_expected_shape() {
        assert!(ease(&Easing::EaseIn, 0.5) < 0.5);
        assert!(ease(&Easing::EaseOut, 0.5) > 0.5);
        // REA R5 INOUT = [0.645, 0.045, 0.355, 1.0] -> at s=0.5, x=0.5 and y=0.516875
        assert!(close(ease(&Easing::EaseInOut, 0.5), 0.516875));
        // CSS ease-in-out [0.42, 0.0, 0.58, 1.0] at t=0.25 ~ 0.129
        assert!((cubic_bezier([0.42, 0.0, 0.58, 1.0], 0.25) - 0.129).abs() < 0.01);
    }

    #[test]
    fn curves_are_monotonic_except_back() {
        for e in [Easing::EaseIn, Easing::EaseOut, Easing::EaseInOut, Easing::Bezier([0.1, 0.9, 0.9, 0.1])] {
            let mut prev = 0.0;
            for i in 0..=100 {
                let y = ease(&e, i as f32 / 100.0);
                assert!(y >= prev - 1e-4, "{e:?} at {i}");
                prev = y;
            }
        }
    }

    #[test]
    fn back_curves_overshoot() {
        let min_in = (0..=100).map(|i| ease(&Easing::EaseInBack, i as f32 / 100.0)).fold(f32::MAX, f32::min);
        assert!(min_in < -0.02, "ease in back dips below 0: {min_in}");
        let max_out = (0..=100).map(|i| ease(&Easing::EaseOutBack, i as f32 / 100.0)).fold(f32::MIN, f32::max);
        assert!(max_out > 1.02, "ease out back overshoots 1: {max_out}");
    }

    #[test]
    fn bezier_solver_handles_steep_curves() {
        // x1 = x2 = 0 makes dx/ds zero at s = 0; the solver must still converge.
        let y = cubic_bezier([0.0, 0.0, 0.0, 1.0], 0.001);
        assert!((0.0..=1.0).contains(&y));
        let mid = cubic_bezier([1.0, 0.0, 0.0, 1.0], 0.5);
        assert!(close(mid, 0.5));
    }

    #[test]
    fn bouncy_spring_overshoots_and_gentle_does_not_much() {
        let bouncy = Easing::Spring { mass: 1.0, stiffness: 600.0, damping: 15.0 };
        let max = (0..=200).map(|i| ease(&bouncy, i as f32 / 200.0)).fold(f32::MIN, f32::max);
        assert!(max > 1.05, "bouncy overshoot {max}");
        let overdamped = Easing::Spring { mass: 1.0, stiffness: 100.0, damping: 60.0 };
        let max = (0..=200).map(|i| ease(&overdamped, i as f32 / 200.0)).fold(f32::MIN, f32::max);
        assert!(max <= 1.001, "overdamped never overshoots {max}");
    }

    #[test]
    fn spring_settles_and_ignores_requested_duration() {
        let e = Easing::Spring { mass: 1.0, stiffness: 300.0, damping: 20.0 };
        let secs = spring_settle_secs(1.0, 300.0, 20.0);
        assert!(secs > 0.3 && secs < 1.5, "{secs}");
        assert!(close(spring_at(1.0, 300.0, 20.0, secs), 1.0) || (spring_at(1.0, 300.0, 20.0, secs) - 1.0).abs() < 0.01);
        assert_eq!(effective_duration_ms(&e, 123), (secs * 1000.0) as u32);
        assert_eq!(effective_duration_ms(&Easing::Linear, 123), 123);
    }

    #[test]
    fn critically_damped_is_continuous_with_neighbours() {
        let w0 = 10.0f32;
        let (m, k) = (1.0, w0 * w0);
        let c = 2.0 * (k * m).sqrt();
        let a = spring_at(m, k, c, 0.2);
        let b = spring_at(m, k, c * 0.99, 0.2);
        let d = spring_at(m, k, c * 1.01, 0.2);
        assert!((a - b).abs() < 0.02 && (a - d).abs() < 0.02, "{a} {b} {d}");
    }

    #[test]
    fn preset_names() {
        assert_eq!(preset_name(&Easing::EaseOut), "Ease out");
        assert_eq!(preset_name(&Easing::Bezier([0.1, 0.2, 0.3, 0.4])), "Custom bezier");
        assert_eq!(preset_name(&Easing::Spring { mass: 2.0, stiffness: 5.0, damping: 1.0 }), "Custom spring");
    }

    #[test]
    fn rea_r5_presets_match_figma_cubic_and_spring_values() {
        assert_eq!(control_points(&Easing::EaseIn), Some([0.55, 0.055, 0.675, 0.19]));
        assert_eq!(control_points(&Easing::EaseOut), Some([0.215, 0.61, 0.355, 1.0]));
        assert_eq!(control_points(&Easing::EaseInOut), Some([0.645, 0.045, 0.355, 1.0]));
        assert_eq!(control_points(&Easing::EaseInBack), Some([0.3, -0.05, 0.7, -0.5]));
        assert_eq!(control_points(&Easing::EaseOutBack), Some([0.45, 1.45, 0.8, 1.0]));
        assert_eq!(control_points(&Easing::EaseInOutBack), Some([0.7, -0.4, 0.4, 1.4]));
        assert_eq!(DEFAULT_BEZIER, [0.0, 0.0, 0.58, 1.0]);
        assert_eq!(EasingType::ALL.len(), 15);
        for (i, et) in EasingType::ALL.iter().enumerate() {
            assert_eq!(et.kiwi_tag() as usize, i);
            assert_eq!(EasingType::from_kiwi_tag(i as u32), Some(*et));
            assert_eq!(EasingType::from_kiwi_name(et.kiwi_name()), Some(*et));
        }
        assert_eq!(EasingType::Hold.ease_at(None, None, 0.5), 0.0);
        assert_eq!(EasingType::Hold.ease_at(None, None, 1.0), 1.0);
    }
}
