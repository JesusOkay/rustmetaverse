pub const PI: f32 = std::f32::consts::PI;
pub const TWO_PI: f32 = std::f32::consts::TAU;
pub const HALF_PI: f32 = std::f32::consts::FRAC_PI_2;
pub const DEG_TO_RAD: f32 = PI / 180.0;
pub const RAD_TO_DEG: f32 = 180.0 / PI;

#[inline]
pub fn clamp(value: f32, min: f32, max: f32) -> f32 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

#[inline]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[inline]
pub fn deg_to_rad(deg: f32) -> f32 {
    deg * DEG_TO_RAD
}

#[inline]
pub fn rad_to_deg(rad: f32) -> f32 {
    rad * RAD_TO_DEG
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_works() {
        assert_eq!(clamp(5.0, 0.0, 10.0), 5.0);
        assert_eq!(clamp(-1.0, 0.0, 10.0), 0.0);
        assert_eq!(clamp(15.0, 0.0, 10.0), 10.0);
    }

    #[test]
    fn lerp_works() {
        assert_eq!(lerp(0.0, 10.0, 0.0), 0.0);
        assert_eq!(lerp(0.0, 10.0, 1.0), 10.0);
        assert_eq!(lerp(0.0, 10.0, 0.5), 5.0);
    }

    #[test]
    fn deg_rad_roundtrip() {
        let deg = 45.0;
        let rad = deg_to_rad(deg);
        assert!((rad - std::f32::consts::FRAC_PI_4).abs() < 1e-5);
        assert!((rad_to_deg(rad) - deg).abs() < 1e-5);
    }
}
