pub const PI: f32 = std::f32::consts::PI;
pub const TWO_PI: f32 = PI * 2.0;
pub const HALF_PI: f32 = PI / 2.0;

pub fn clamp(value: f32, min: f32, max: f32) -> f32 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}
