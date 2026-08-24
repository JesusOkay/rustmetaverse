use serde::{Deserialize, Serialize};
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

#[derive(Clone, Copy, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vector3 {
    pub const ZERO: Vector3 = Vector3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    pub const ONE: Vector3 = Vector3 {
        x: 1.0,
        y: 1.0,
        z: 1.0,
    };
    pub const UNIT_X: Vector3 = Vector3 {
        x: 1.0,
        y: 0.0,
        z: 0.0,
    };
    pub const UNIT_Y: Vector3 = Vector3 {
        x: 0.0,
        y: 1.0,
        z: 0.0,
    };
    pub const UNIT_Z: Vector3 = Vector3 {
        x: 0.0,
        y: 0.0,
        z: 1.0,
    };

    #[inline]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Vector3 { x, y, z }
    }

    #[inline]
    pub fn length(&self) -> f32 {
        self.length_squared().sqrt()
    }

    #[inline]
    pub fn length_squared(&self) -> f32 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    #[inline]
    pub fn normalize(&mut self) {
        let len_sq = self.length_squared();
        if len_sq > f32::EPSILON {
            let inv_len = len_sq.sqrt().recip();
            self.x *= inv_len;
            self.y *= inv_len;
            self.z *= inv_len;
        }
    }

    #[inline]
    pub fn normalized(&self) -> Self {
        let mut v = *self;
        v.normalize();
        v
    }

    #[inline]
    pub fn dot(&self, other: &Vector3) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    #[inline]
    pub fn cross(&self, other: &Vector3) -> Vector3 {
        Vector3 {
            x: self.y * other.z - self.z * other.y,
            y: self.z * other.x - self.x * other.z,
            z: self.x * other.y - self.y * other.x,
        }
    }

    #[inline]
    pub fn distance(&self, other: &Vector3) -> f32 {
        self.distance_squared(other).sqrt()
    }

    #[inline]
    pub fn distance_squared(&self, other: &Vector3) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        dx * dx + dy * dy + dz * dz
    }

    /// Linear interpolation between `self` and `other` by factor `t` (0.0–1.0).
    #[inline]
    pub fn lerp(&self, other: &Vector3, t: f32) -> Vector3 {
        Vector3 {
            x: self.x + (other.x - self.x) * t,
            y: self.y + (other.y - self.y) * t,
            z: self.z + (other.z - self.z) * t,
        }
    }

    /// Component-wise minimum.
    #[inline]
    pub fn min(&self, other: &Vector3) -> Vector3 {
        Vector3 {
            x: self.x.min(other.x),
            y: self.y.min(other.y),
            z: self.z.min(other.z),
        }
    }

    /// Component-wise maximum.
    #[inline]
    pub fn max(&self, other: &Vector3) -> Vector3 {
        Vector3 {
            x: self.x.max(other.x),
            y: self.y.max(other.y),
            z: self.z.max(other.z),
        }
    }

    /// Returns true if all components are finite (not NaN or Inf).
    #[inline]
    pub fn is_finite(&self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

impl Neg for Vector3 {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        Vector3 {
            x: -self.x,
            y: -self.y,
            z: -self.z,
        }
    }
}

impl Add for Vector3 {
    type Output = Self;
    #[inline]
    fn add(self, other: Self) -> Self {
        Vector3 {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
        }
    }
}

impl Sub for Vector3 {
    type Output = Self;
    #[inline]
    fn sub(self, other: Self) -> Self {
        Vector3 {
            x: self.x - other.x,
            y: self.y - other.y,
            z: self.z - other.z,
        }
    }
}

impl Mul<f32> for Vector3 {
    type Output = Self;
    #[inline]
    fn mul(self, scalar: f32) -> Self {
        Vector3 {
            x: self.x * scalar,
            y: self.y * scalar,
            z: self.z * scalar,
        }
    }
}

impl Div<f32> for Vector3 {
    type Output = Self;
    #[inline]
    fn div(self, scalar: f32) -> Self {
        let inv = scalar.recip();
        Vector3 {
            x: self.x * inv,
            y: self.y * inv,
            z: self.z * inv,
        }
    }
}

impl AddAssign for Vector3 {
    #[inline]
    fn add_assign(&mut self, other: Self) {
        self.x += other.x;
        self.y += other.y;
        self.z += other.z;
    }
}

impl SubAssign for Vector3 {
    #[inline]
    fn sub_assign(&mut self, other: Self) {
        self.x -= other.x;
        self.y -= other.y;
        self.z -= other.z;
    }
}

impl MulAssign<f32> for Vector3 {
    #[inline]
    fn mul_assign(&mut self, scalar: f32) {
        self.x *= scalar;
        self.y *= scalar;
        self.z *= scalar;
    }
}

impl DivAssign<f32> for Vector3 {
    #[inline]
    fn div_assign(&mut self, scalar: f32) {
        let inv = scalar.recip();
        self.x *= inv;
        self.y *= inv;
        self.z *= inv;
    }
}

impl Mul<Vector3> for Vector3 {
    type Output = Self;
    /// Component-wise multiplication (Hadamard product).
    #[inline]
    fn mul(self, other: Vector3) -> Self {
        Vector3 {
            x: self.x * other.x,
            y: self.y * other.y,
            z: self.z * other.z,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_squared_matches_distance() {
        let a = Vector3::new(1.0, 2.0, 3.0);
        let b = Vector3::new(4.0, 6.0, 8.0);
        assert!((a.distance_squared(&b) - a.distance(&b).powi(2)).abs() < 1e-5);
    }

    #[test]
    fn lerp_endpoints() {
        let a = Vector3::new(0.0, 0.0, 0.0);
        let b = Vector3::new(10.0, 20.0, 30.0);
        assert_eq!(a.lerp(&b, 0.0), a);
        assert_eq!(a.lerp(&b, 1.0), b);
    }

    #[test]
    fn neg_works() {
        let v = Vector3::new(1.0, -2.0, 3.0);
        assert_eq!(-v, Vector3::new(-1.0, 2.0, -3.0));
    }

    #[test]
    fn assign_ops_work() {
        let mut v = Vector3::new(1.0, 2.0, 3.0);
        v += Vector3::new(1.0, 1.0, 1.0);
        assert_eq!(v, Vector3::new(2.0, 3.0, 4.0));
        v -= Vector3::new(1.0, 1.0, 1.0);
        assert_eq!(v, Vector3::new(1.0, 2.0, 3.0));
        v *= 2.0;
        assert_eq!(v, Vector3::new(2.0, 4.0, 6.0));
        v /= 2.0;
        assert_eq!(v, Vector3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn hadamard_product() {
        let a = Vector3::new(1.0, 2.0, 3.0);
        let b = Vector3::new(4.0, 5.0, 6.0);
        assert_eq!(a * b, Vector3::new(4.0, 10.0, 18.0));
    }

    #[test]
    fn is_finite_detects_nan() {
        assert!(Vector3::new(1.0, 2.0, 3.0).is_finite());
        assert!(!Vector3::new(f32::NAN, 2.0, 3.0).is_finite());
        assert!(!Vector3::new(1.0, f32::INFINITY, 3.0).is_finite());
    }
}
