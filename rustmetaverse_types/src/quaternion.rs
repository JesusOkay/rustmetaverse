use crate::Vector3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Quaternion {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Default for Quaternion {
    fn default() -> Self {
        Quaternion::IDENTITY
    }
}

impl Quaternion {
    pub const IDENTITY: Quaternion = Quaternion {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 1.0,
    };

    #[inline]
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Quaternion { x, y, z, w }
    }

    #[inline]
    pub fn normalize(&mut self) {
        let len_sq = self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w;
        if len_sq > f32::EPSILON {
            let inv_len = len_sq.sqrt().recip();
            self.x *= inv_len;
            self.y *= inv_len;
            self.z *= inv_len;
            self.w *= inv_len;
        }
    }

    #[inline]
    pub fn normalized(&self) -> Self {
        let mut q = *self;
        q.normalize();
        q
    }

    #[inline]
    pub fn conjugate(&self) -> Self {
        Quaternion {
            x: -self.x,
            y: -self.y,
            z: -self.z,
            w: self.w,
        }
    }

    #[inline]
    pub fn inverse(&self) -> Self {
        let len_sq = self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w;
        if len_sq <= f32::EPSILON {
            return Quaternion::IDENTITY;
        }
        let inv_len_sq = len_sq.recip();
        Quaternion {
            x: -self.x * inv_len_sq,
            y: -self.y * inv_len_sq,
            z: -self.z * inv_len_sq,
            w: self.w * inv_len_sq,
        }
    }

    /// Quaternion multiplication (Hamilton product). Order matters: self * other.
    #[inline]
    pub fn multiply(&self, other: &Quaternion) -> Quaternion {
        Quaternion {
            x: self.w * other.x + self.x * other.w + self.y * other.z - self.z * other.y,
            y: self.w * other.y - self.x * other.z + self.y * other.w + self.z * other.x,
            z: self.w * other.z + self.x * other.y - self.y * other.x + self.z * other.w,
            w: self.w * other.w - self.x * other.x - self.y * other.y - self.z * other.z,
        }
    }

    /// Rotate a vector by this quaternion.
    #[inline]
    pub fn rotate(&self, v: &Vector3) -> Vector3 {
        // Standard quaternion rotation: v' = q * v * q^{-1}
        // Optimized formula for unit quaternion: v' = v + 2.0 * w * cross(u, v) + 2.0 * cross(u, cross(u, v))
        // where u = (x, y, z) is the vector part of the quaternion.
        let u = Vector3::new(self.x, self.y, self.z);
        let uv = u.cross(v);
        let uuv = u.cross(&uv);
        // v + 2.0 * (w * uv + uuv)
        *v + (uv * (2.0 * self.w) + uuv * 2.0)
    }

    /// Spherical-linear interpolation between two quaternions.
    pub fn slerp(&self, other: &Quaternion, t: f32) -> Quaternion {
        let mut dot = self.x * other.x + self.y * other.y + self.z * other.z + self.w * other.w;

        // If the dot product is negative, slerp takes the shorter arc.
        let mut other = *other;
        if dot < 0.0 {
            other = Quaternion::new(-other.x, -other.y, -other.z, -other.w);
            dot = -dot;
        }

        // If the quaternions are very close, fall back to lerp to avoid division by near-zero.
        if dot > 0.9995 {
            return Quaternion::new(
                self.x + (other.x - self.x) * t,
                self.y + (other.y - self.y) * t,
                self.z + (other.z - self.z) * t,
                self.w + (other.w - self.w) * t,
            )
            .normalized();
        }

        let theta_0 = dot.acos();
        let theta = theta_0 * t;
        let sin_theta = theta.sin();
        let sin_theta_0 = theta_0.sin();

        let s0 = ((1.0 - t) * theta_0).sin() / sin_theta_0;
        let s1 = sin_theta / sin_theta_0;

        Quaternion::new(
            s0 * self.x + s1 * other.x,
            s0 * self.y + s1 * other.y,
            s0 * self.z + s1 * other.z,
            s0 * self.w + s1 * other.w,
        )
    }

    /// Create a quaternion from an axis-angle rotation.
    pub fn from_axis_angle(axis: &Vector3, angle: f32) -> Self {
        let half = angle * 0.5;
        let sin_half = half.sin();
        let a = (*axis).normalized() * sin_half;
        Quaternion::new(a.x, a.y, a.z, half.cos())
    }
}

impl std::ops::Mul for Quaternion {
    type Output = Self;
    #[inline]
    fn mul(self, other: Self) -> Self {
        self.multiply(&other)
    }
}

impl std::ops::MulAssign for Quaternion {
    #[inline]
    fn mul_assign(&mut self, other: Self) {
        *self = self.multiply(&other);
    }
}

impl std::ops::Sub for Quaternion {
    type Output = Self;
    #[inline]
    fn sub(self, other: Self) -> Self {
        Quaternion::new(
            self.x - other.x,
            self.y - other.y,
            self.z - other.z,
            self.w - other.w,
        )
    }
}

impl Quaternion {
    #[inline]
    pub fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w).sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_rotate_is_identity() {
        let v = Vector3::new(1.0, 2.0, 3.0);
        let result = Quaternion::IDENTITY.rotate(&v);
        assert!((result - v).length() < 1e-5);
    }

    #[test]
    fn conjugate_negates_imaginary() {
        let q = Quaternion::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(q.conjugate(), Quaternion::new(-1.0, -2.0, -3.0, 4.0));
    }

    #[test]
    fn inverse_of_normalized_is_conjugate() {
        let q = Quaternion::new(1.0, 0.0, 0.0, 0.0).normalized();
        let inv = q.inverse();
        // q * inverse should be identity
        let product = q.multiply(&inv);
        assert!((product - Quaternion::IDENTITY).length() < 1e-5);
    }

    #[test]
    fn slerp_endpoints() {
        let a = Quaternion::IDENTITY;
        let b = Quaternion::from_axis_angle(&Vector3::UNIT_Y, std::f32::consts::FRAC_PI_2);
        assert!((a.slerp(&b, 0.0) - a).length() < 1e-5);
        assert!((a.slerp(&b, 1.0) - b).length() < 1e-5);
    }

    #[test]
    fn from_axis_angle_90_degrees() {
        let q = Quaternion::from_axis_angle(&Vector3::UNIT_Z, std::f32::consts::FRAC_PI_2);
        let v = Vector3::UNIT_X;
        let rotated = q.rotate(&v);
        // Rotating X by 90° around Z gives Y
        assert!((rotated - Vector3::UNIT_Y).length() < 1e-5);
    }
}
