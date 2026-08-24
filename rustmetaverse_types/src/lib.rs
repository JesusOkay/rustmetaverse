//! Foundational types for rustmetaverse.
//!
//! This crate provides the primitive types used across the rustmetaverse
//! workspace: [`UUID`] (a Second Life / OpenSimulator-compatible identifier),
//! [`Vector3`] (a 3-component float vector with the math operations the
//! protocol needs), [`Quaternion`] (a rotation quaternion), and the math
//! helpers in [`utils`].
//!
//! These types are intentionally dependency-light so they can be shared by
//! the protocol, structured-data, and client crates without pulling in
//! networking code.

pub mod quaternion;
pub mod utils;
pub mod uuid;
pub mod vector3;

pub use quaternion::Quaternion;
pub use uuid::UUID;
pub use vector3::Vector3;
