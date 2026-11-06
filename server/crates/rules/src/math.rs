//! Versioned, platform-independent gameplay math.
//!
//! Coordinates are stored as signed 32-bit fixed point (see `Coord`). Transient geometry
//! uses strict IEEE-754 arithmetic; Rust does not implicitly contract multiply/add into FMA.
//! Transcendentals and explicit FMA use pinned, software-only libm rather than the OS or
//! target-specific intrinsics. Never enable fast-math or flush-to-zero for simulation code.

mod coord;
pub use coord::Coord;

/// Explicit names make accidental use of platform math detectable by the source guard.
pub trait GameplayMath: Sized + Copy {
    fn det_sqrt(self) -> Self;
    fn det_sin(self) -> Self;
    fn det_cos(self) -> Self;
    fn det_atan2(self, a: Self) -> Self;
    fn det_hypot(self, a: Self) -> Self;
    fn det_acos(self) -> Self;
    fn det_asin(self) -> Self;
    fn det_tan(self) -> Self;
    fn det_atan(self) -> Self;
    fn det_powf(self, a: Self) -> Self;
    fn det_exp(self) -> Self;
    fn det_ln(self) -> Self;
    fn det_mul_add(self, a: Self, b: Self) -> Self;
    fn det_sin_cos(self) -> (Self, Self);
    fn det_powi(self, exponent: i32) -> Self;
}

impl GameplayMath for f32 {
    fn det_sqrt(self) -> Self {
        libm::sqrtf(self)
    }
    fn det_sin(self) -> Self {
        libm::sinf(self)
    }
    fn det_cos(self) -> Self {
        libm::cosf(self)
    }
    fn det_atan2(self, a: Self) -> Self {
        libm::atan2f(self, a)
    }
    fn det_hypot(self, a: Self) -> Self {
        libm::hypotf(self, a)
    }
    fn det_acos(self) -> Self {
        libm::acosf(self)
    }
    fn det_asin(self) -> Self {
        libm::asinf(self)
    }
    fn det_tan(self) -> Self {
        libm::tanf(self)
    }
    fn det_atan(self) -> Self {
        libm::atanf(self)
    }
    fn det_powf(self, a: Self) -> Self {
        libm::powf(self, a)
    }
    fn det_exp(self) -> Self {
        libm::expf(self)
    }
    fn det_ln(self) -> Self {
        libm::logf(self)
    }
    fn det_mul_add(self, a: Self, b: Self) -> Self {
        libm::fmaf(self, a, b)
    }
    fn det_sin_cos(self) -> (Self, Self) {
        (self.det_sin(), self.det_cos())
    }
    fn det_powi(self, exponent: i32) -> Self {
        // Defined multiply order, including i32::MIN. LLVM's powi intrinsic does not
        // specify one; its rounding can vary with target/codegen settings.
        let mut power = exponent.unsigned_abs();
        let mut base = if exponent < 0 { 1.0 / self } else { self };
        let mut result = 1.0;
        while power != 0 {
            if power & 1 != 0 {
                result *= base;
            }
            power >>= 1;
            if power != 0 {
                base *= base;
            }
        }
        result
    }
}
impl GameplayMath for f64 {
    fn det_sqrt(self) -> Self {
        libm::sqrt(self)
    }
    fn det_sin(self) -> Self {
        libm::sin(self)
    }
    fn det_cos(self) -> Self {
        libm::cos(self)
    }
    fn det_atan2(self, a: Self) -> Self {
        libm::atan2(self, a)
    }
    fn det_hypot(self, a: Self) -> Self {
        libm::hypot(self, a)
    }
    fn det_acos(self) -> Self {
        libm::acos(self)
    }
    fn det_asin(self) -> Self {
        libm::asin(self)
    }
    fn det_tan(self) -> Self {
        libm::tan(self)
    }
    fn det_atan(self) -> Self {
        libm::atan(self)
    }
    fn det_powf(self, a: Self) -> Self {
        libm::pow(self, a)
    }
    fn det_exp(self) -> Self {
        libm::exp(self)
    }
    fn det_ln(self) -> Self {
        libm::log(self)
    }
    fn det_mul_add(self, a: Self, b: Self) -> Self {
        libm::fma(self, a, b)
    }
    fn det_sin_cos(self) -> (Self, Self) {
        (self.det_sin(), self.det_cos())
    }
    fn det_powi(self, exponent: i32) -> Self {
        // Defined multiply order, including i32::MIN. LLVM's powi intrinsic does not
        // specify one; its rounding can vary with target/codegen settings.
        let mut power = exponent.unsigned_abs();
        let mut base = if exponent < 0 { 1.0 / self } else { self };
        let mut result = 1.0;
        while power != 0 {
            if power & 1 != 0 {
                result *= base;
            }
            power >>= 1;
            if power != 0 {
                base *= base;
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::GameplayMath;

    #[test]
    fn angle_quadrants_axes_and_roots_have_stable_bits() {
        for (y, x, expected) in [
            (0.0_f32, 1.0_f32, 0x0000_0000),
            (-0.0, 1.0, 0x8000_0000),
            (1.0, 0.0, 0x3fc9_0fdb),
            (-1.0, 0.0, 0xbfc9_0fdb),
            (1.0, 1.0, 0x3f49_0fdb),
            (-1.0, -1.0, 0xc016_cbe4),
        ] {
            assert_eq!(y.det_atan2(x).to_bits(), expected);
        }
        assert_eq!(2.0_f32.det_sqrt().to_bits(), 0x3fb5_04f3);
        assert_eq!(1.0_f32.det_sin().to_bits(), 0x3f57_6aa4);
        assert_eq!(1.0_f32.det_cos().to_bits(), 0x3f0a_5140);
        assert_eq!(3.0_f32.det_hypot(4.0).to_bits(), 5.0_f32.to_bits());
        assert_eq!(2.0_f32.det_powi(-3), 0.125);
        assert_eq!(1.0_f32.det_powi(i32::MIN), 1.0);
        assert_eq!(0.0_f32.det_powi(0), 1.0);
        assert_eq!(1.0_f64.det_sin().to_bits(), 0x3fea_ed54_8f09_0cee);
    }
}
