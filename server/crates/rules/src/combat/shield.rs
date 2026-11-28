use crate::math::GameplayMath as _;

/// An allied building is covered when its footprint overlaps at least half the field.
/// Integrate the circle/rectangle intersection analytically in normalized circle space.
pub fn shield_covers_building(center: (f32, f32), radius: f32, rect: (f32, f32, f32, f32)) -> bool {
    let (left, top, right, bottom) = rect;
    if ![center.0, center.1, radius, left, top, right, bottom]
        .iter()
        .all(|v| v.is_finite())
        || radius <= 0.0
        || left >= right
        || top >= bottom
    {
        return false;
    }
    let r = f64::from(radius);
    let x0 = (f64::from(left) - f64::from(center.0)) / r;
    let x1 = (f64::from(right) - f64::from(center.0)) / r;
    let y0 = (f64::from(top) - f64::from(center.1)) / r;
    let y1 = (f64::from(bottom) - f64::from(center.1)) / r;
    if x0 >= 1.0 || x1 <= -1.0 || y0 >= 1.0 || y1 <= -1.0 {
        return false;
    }
    let area = quadrant(x1, y1) - quadrant(x0, y1) - quadrant(x1, y0) + quadrant(x0, y0);
    // Only absorb floating-point integration roundoff at the inclusive 50% boundary.
    area >= std::f64::consts::FRAC_PI_2 - 1e-12
}

/// Signed area between the origin and (x, y), clipped to the unit disk.
fn quadrant(x: f64, y: f64) -> f64 {
    let sign = x.signum() * y.signum();
    let x = x.abs().min(1.0);
    let y = y.abs().min(1.0);
    let flat_end = x.min((1.0 - y * y).max(0.0).det_sqrt());
    sign * (y * flat_end + primitive(x) - primitive(flat_end))
}

fn primitive(x: f64) -> f64 {
    0.5 * (x * (1.0 - x * x).max(0.0).det_sqrt() + x.det_asin())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shield_building_area_boundary_and_corner_overlap() {
        for rect in [
            (-2.0, -2.0, 2.0, 2.0),
            (0.0, -2.0, 2.0, 2.0),
            (-2.0, -2.0, 0.0, 2.0),
            (-2.0, 0.0, 2.0, 2.0),
            (-2.0, -2.0, 2.0, 0.0),
            (-0.7, -0.7, 0.7, 0.7),
        ] {
            assert!(shield_covers_building((0.0, 0.0), 1.0, rect), "{rect:?}");
        }
        for rect in [
            (0.001, -2.0, 2.0, 2.0),
            (-2.0, -2.0, -0.001, 2.0),
            (0.0, 0.0, 2.0, 2.0),
            (-0.5, -0.5, 0.5, 0.5),
            (1.0, -2.0, 2.0, 2.0),
            (2.0, 2.0, 3.0, 3.0),
        ] {
            assert!(!shield_covers_building((0.0, 0.0), 1.0, rect), "{rect:?}");
        }
    }
}
