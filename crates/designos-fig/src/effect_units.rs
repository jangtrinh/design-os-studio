//! Conversions between Studio's effect values and the values a `.fig` or REST file stores.
//!
//! Each stored value has one pair of functions, and nothing else does arithmetic on it:
//!
//! - The noise and texture size: the stored `noiseSize` is the size the editor shows, along X and
//!   along Y independently. Studio keeps X in `size` and Y in `size_y`, which is `None` while Y equals X.
//! - The specular angle: the stored `specularAngle` is in degrees, the unit Studio stores in
//!   `light_angle`.
//!
//! Both are the identity today. If a later reading of the format shows another unit, the change is made
//! here and every import and export follows, so a round trip stays lossless.

/// Studio's `(size, size_y)` from the stored X and Y size. Y is kept only while it differs from X, so a
/// file with one size reads and saves as one size.
pub(crate) fn noise_size_from_kiwi(x: f32, y: f32) -> (f32, Option<f32>) {
    let (x, y) = (x, y);
    (x, (y != x).then_some(y))
}

/// The stored X and Y size of Studio's `(size, size_y)`.
pub(crate) fn noise_size_to_kiwi(size: f32, size_y: Option<f32>) -> (f32, f32) {
    (size, size_y.unwrap_or(size))
}

/// Studio's light angle (degrees) from the stored specular angle.
pub(crate) fn specular_angle_from_kiwi(angle: f32) -> f32 {
    angle
}

/// The stored specular angle of Studio's light angle (degrees).
pub(crate) fn specular_angle_to_kiwi(angle: f32) -> f32 {
    angle
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_size_stays_one_size_and_two_sizes_stay_two() {
        assert_eq!(noise_size_from_kiwi(4.0, 4.0), (4.0, None));
        assert_eq!(noise_size_from_kiwi(3.0, 7.0), (3.0, Some(7.0)));
        assert_eq!(noise_size_to_kiwi(4.0, None), (4.0, 4.0));
        assert_eq!(noise_size_to_kiwi(3.0, Some(7.0)), (3.0, 7.0));
    }

    #[test]
    fn sizes_and_angles_round_trip_through_the_stored_form() {
        for (x, y) in [(0.5, 0.5), (3.0, 7.0), (7.0, 3.0), (1.25, 100.0)] {
            let (size, size_y) = noise_size_from_kiwi(x, y);
            assert_eq!(noise_size_to_kiwi(size, size_y), (x, y));
        }
        for a in [-899.0, -45.0, 0.0, 135.0, 900.0] {
            assert_eq!(specular_angle_from_kiwi(specular_angle_to_kiwi(a)), a);
        }
    }
}
