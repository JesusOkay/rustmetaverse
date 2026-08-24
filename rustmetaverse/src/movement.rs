//! Avatar movement — sending `AgentUpdate` packets to drive the avatar.
//!
//! The viewer sends `AgentUpdate` ~10 times per second to communicate body
//! rotation, camera position/orientation, and movement control flags. The
//! simulator uses `control_flags` to move the avatar forward, backward, turn,
//! etc.
//!
//! Movement flags are bit-mapped values in `control_flags`:
//!
//! | Flag | Bit |
//! |------|-----|
//! | Forward | 0x01 |
//! | Backward | 0x02 |
//! | Left | 0x04 |
//! | Right | 0x08 |
//! | Up (jump/fly) | 0x10 |
//! | Down | 0x20 |
//! | Turn Left | 0x100 |
//! | Turn Right | 0x200 |
//! | ... | |
//!
//! See libopenmetaverse `AgentManager.UpdateMovement()` and the
//! `AGENT_CONTROL_` constants in the viewer source.

use crate::networking::network_manager::NetworkManager;
use crate::simulator::Simulator;
use rustmetaverse_protocol::header::{Header, PacketFrequency};
use rustmetaverse_protocol::packets::{AgentUpdateAgentDataBlock, AgentUpdatePacket};
use rustmetaverse_types::{Quaternion, Vector3};
use std::sync::Arc;
use tokio::sync::Mutex;

// ── Movement control flags ──────────────────────────────────────────────

pub const CONTROL_AT_POS: u32 = 0x00000001; // Forward
pub const CONTROL_AT_NEG: u32 = 0x00000002; // Backward
pub const CONTROL_LEFT_POS: u32 = 0x00000004; // Strafe left
pub const CONTROL_LEFT_NEG: u32 = 0x00000008; // Strafe right
pub const CONTROL_UP_POS: u32 = 0x00000010; // Up / fly up
pub const CONTROL_UP_NEG: u32 = 0x00000020; // Down / crouch
pub const CONTROL_TURN_LEFT: u32 = 0x00000100; // Yaw left
pub const CONTROL_TURN_RIGHT: u32 = 0x00000200; // Yaw right
pub const CONTROL_FINISH_TERRAIN: u32 = 0x00000800;
pub const CONTROL_NUDGE_AT_POS: u32 = 0x00001000;
pub const CONTROL_NUDGE_AT_NEG: u32 = 0x00002000;
pub const CONTROL_NUDGE_LEFT_POS: u32 = 0x00004000;
pub const CONTROL_NUDGE_LEFT_NEG: u32 = 0x00008000;
pub const CONTROL_NUDGE_UP_POS: u32 = 0x00010000;
pub const CONTROL_NUDGE_UP_NEG: u32 = 0x00020000;
pub const CONTROL_FAST_AT: u32 = 0x00040000;
pub const CONTROL_FAST_LEFT: u32 = 0x00080000;
pub const CONTROL_FAST_UP: u32 = 0x00100000;
pub const CONTROL_FLY: u32 = 0x00200000;
pub const CONTROL_STOP: u32 = 0x00400000;

/// Agent state flags (the `state` byte in AgentUpdate).
pub const AGENT_STATE_NONE: u8 = 0;
pub const AGENT_STATE_TYPING: u8 = 1 << 2;
pub const AGENT_STATE_EDITING: u8 = 1 << 3;

/// Flags byte in AgentUpdate.
pub const AGENT_FLAG_NONE: u8 = 0;

/// Build and send an `AgentUpdate` packet.
///
/// Call this ~10×/s to continuously drive avatar movement. The simulator
/// interpolates between updates; if you stop sending, the avatar stops.
#[allow(clippy::too_many_arguments)]
pub async fn send_agent_update(
    network: &Arc<NetworkManager>,
    simulator: &Arc<Mutex<Option<Simulator>>>,
    body_rotation: Quaternion,
    head_rotation: Quaternion,
    camera_center: Vector3,
    camera_at_axis: Vector3,
    camera_left_axis: Vector3,
    camera_up_axis: Vector3,
    far: f32,
    control_flags: u32,
    state: u8,
    flags: u8,
) -> Result<(), std::io::Error> {
    let (agent_id, session_id) = {
        let sim = simulator.lock().await;
        if let Some(s) = sim.as_ref() {
            (s.client, s.session_id)
        } else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "No simulator connected",
            ));
        }
    };

    let seq = network.get_next_sequence();
    let mut packet = AgentUpdatePacket {
        header: Header {
            frequency: PacketFrequency::High,
            id: 4,           // AgentUpdate is High frequency message 4
            reliable: false, // AgentUpdate is sent unreliable for low latency
            sequence: seq,
            ..Default::default()
        },
        agent_data: AgentUpdateAgentDataBlock {
            agent_i_d: agent_id,
            session_i_d: session_id,
            body_rotation,
            head_rotation,
            state,
            camera_center,
            camera_at_axis,
            camera_left_axis,
            camera_up_axis,
            far,
            control_flags,
            flags,
        },
    };

    network.send_packet(&mut packet).await
}

/// Convenience: send a movement-only AgentUpdate with the given control flags.
pub async fn send_movement(
    network: &Arc<NetworkManager>,
    simulator: &Arc<Mutex<Option<Simulator>>>,
    control_flags: u32,
    body_rotation: Quaternion,
    camera_center: Vector3,
    camera_at_axis: Vector3,
) -> Result<(), std::io::Error> {
    // Derive camera left and up from the forward (at) axis. The standard
    // viewer convention: forward = at_axis, up ≈ +Z, left = up × forward.
    let world_up = Vector3::new(0.0, 0.0, 1.0);
    let camera_left = world_up.cross(&camera_at_axis).normalized();
    let camera_up = camera_at_axis.cross(&camera_left).normalized();

    send_agent_update(
        network,
        simulator,
        body_rotation,
        Quaternion::IDENTITY,
        camera_center,
        camera_at_axis,
        camera_left,
        camera_up,
        128.0, // far clip
        control_flags,
        AGENT_STATE_NONE,
        AGENT_FLAG_NONE,
    )
    .await
}

/// Send an `AgentUpdate` that signals "stop all movement".
pub async fn send_stop(
    network: &Arc<NetworkManager>,
    simulator: &Arc<Mutex<Option<Simulator>>>,
    camera_center: Vector3,
    camera_at_axis: Vector3,
    body_rotation: Quaternion,
) -> Result<(), std::io::Error> {
    send_movement(
        network,
        simulator,
        CONTROL_STOP,
        body_rotation,
        camera_center,
        camera_at_axis,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_flags_are_distinct() {
        // Ensure the key flags don't accidentally overlap
        assert_ne!(CONTROL_AT_POS, CONTROL_AT_NEG);
        assert_ne!(CONTROL_LEFT_POS, CONTROL_LEFT_NEG);
        assert_ne!(CONTROL_UP_POS, CONTROL_UP_NEG);
        assert_ne!(CONTROL_TURN_LEFT, CONTROL_TURN_RIGHT);
    }

    #[test]
    fn control_flags_can_combine() {
        let combined = CONTROL_AT_POS | CONTROL_TURN_LEFT;
        assert!(combined & CONTROL_AT_POS != 0);
        assert!(combined & CONTROL_TURN_LEFT != 0);
    }
}
