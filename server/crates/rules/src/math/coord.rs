use serde::{Deserialize, Serialize};

/// Signed Q19.12 world coordinate: 1/4096 world unit, 131072 substeps per 32-unit tile.
/// Integer storage and checkpoint JSON contain raw substeps, never floating positions.
/// Transient f32 geometry rounds to IEEE nearest-even; at the future map edge its ULP
/// is 1/512 world unit. Storage/JSON never depend on that projection rounding.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Coord(i32);

impl Coord {
    pub const SCALE: i32 = 4096;

    /// Nearest substep, ties away from zero. Widen BEFORE scaling/rounding so conversion
    /// cannot lose the low bits or overflow near i32 limits. No wrapping or panics.
    pub fn try_from_world(world: f32) -> Option<Self> {
        let raw = (f64::from(world) * f64::from(Self::SCALE)).round();
        (raw.is_finite() && raw >= f64::from(i32::MIN) && raw <= f64::from(i32::MAX))
            .then_some(Self(raw as i32))
    }

    /// Saturating conversion for trusted geometry. Rust's float-to-int cast clamps finite
    /// overflow and infinities; NaN becomes zero. Admission uses `try_from_world` and map
    /// bounds instead, so saturation is a last-resort arithmetic policy, never wrapping.
    pub fn from_world(world: f32) -> Self {
        Self((f64::from(world) * f64::from(Self::SCALE)).round() as i32)
    }

    pub fn to_world(self) -> f32 {
        self.0 as f32 / Self::SCALE as f32
    }
    pub fn raw(self) -> i32 {
        self.0
    }

    /// Full i32 endpoints can differ by more than i32::MAX. Even i64 is insufficient for
    /// their squared 2D diagonal, so widen differences to i64 and products/sums to i128.
    /// Grid-based circular inclusion for physical area effects. Query points and radii use
    /// the same nearest-substep rule as physical positions, with widened squared products.
    pub fn within_radius(center: (Self, Self), point: (f32, f32), radius: f32) -> bool {
        if radius < 0.0 {
            return false;
        }
        let (Some(x), Some(y), Some(radius)) = (
            Self::try_from_world(point.0),
            Self::try_from_world(point.1),
            Self::try_from_world(radius),
        ) else {
            return false;
        };
        Self::distance_squared(center, (x, y)) <= i128::from(radius.0) * i128::from(radius.0)
    }

    pub fn distance_squared(a: (Self, Self), b: (Self, Self)) -> i128 {
        let dx = i128::from(i64::from(a.0 .0) - i64::from(b.0 .0));
        let dy = i128::from(i64::from(a.1 .0) - i64::from(b.1 .0));
        dx * dx + dy * dy
    }
}

#[cfg(test)]
mod tests {
    use super::Coord;

    #[test]
    fn storage_rounding_and_invalid_inputs() {
        assert_eq!(std::mem::size_of::<Coord>(), 4);
        for (input, raw) in [
            (0.0, 0),
            (0.5 / 4096.0, 1),
            (-0.5 / 4096.0, -1),
            (1.49 / 4096.0, 1),
            (-1.49 / 4096.0, -1),
        ] {
            assert_eq!(Coord::from_world(input).raw(), raw);
        }
        for input in [
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::MAX,
            f32::MIN,
        ] {
            assert_eq!(Coord::try_from_world(input), None);
        }
    }

    #[test]
    fn planned_map_projection_is_bounded_including_far_edge() {
        // Subsample the full future map and verify projection loses no more than half an
        // f32 ULP at the far edge (four raw substeps), with exact fixed storage throughout.
        for raw in (0..=512 * 32 * Coord::SCALE).step_by(17) {
            let value = Coord(raw);
            let projected = Coord::try_from_world(value.to_world()).unwrap();
            assert!((projected.raw() - raw).abs() <= 4);
        }
        let extent = Coord::from_world(512.0 * 32.0);
        let expected = 2_i128 * i128::from(512 * 32 * Coord::SCALE).pow(2);
        assert_eq!(
            Coord::distance_squared((Coord(0), Coord(0)), (extent, extent)),
            expected
        );
    }

    #[test]
    fn circular_inclusion_is_integer_and_boundary_exact() {
        let center = (Coord::from_world(16000.0), Coord::from_world(16000.0));
        assert!(Coord::within_radius(center, (16003.0, 16004.0), 5.0));
        assert!(!Coord::within_radius(center, (16003.0, 16004.0), 4.99));
        assert!(!Coord::within_radius(center, (f32::NAN, 0.0), 5.0));
        assert!(!Coord::within_radius(center, (16000.0, 16000.0), -1.0));
    }

    #[test]
    fn extreme_distance_uses_wide_intermediates_and_checkpoint_is_raw_integer() {
        let a = (Coord(i32::MIN), Coord(i32::MIN));
        let b = (Coord(i32::MAX), Coord(i32::MAX));
        assert_eq!(
            Coord::distance_squared(a, b),
            2 * i128::from(u32::MAX).pow(2)
        );
        // Serde's integer representation is independent of decimal floating parsing.
        assert_eq!(Coord::from_world(16384.0).raw(), 67_108_864);
    }
}
