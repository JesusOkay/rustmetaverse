//! Object manipulation — creating, deleting, linking, and renaming objects.
//!
//! Objects in SL are created with `ObjectAdd`, which raycasts from a start
//! point to an end point and creates a prim where the ray hits. Objects are
//! identified by `object_local_id` (a per-region u32) for delete/link/name
//! operations.

use crate::networking::network_manager::NetworkManager;
use crate::simulator::Simulator;
use rustmetaverse_protocol::header::{Header, PacketFrequency};
use rustmetaverse_protocol::packets::{
    ObjectAddAgentDataBlock, ObjectAddObjectDataBlock, ObjectAddPacket, ObjectDeleteAgentDataBlock,
    ObjectDeleteObjectDataBlock, ObjectDeletePacket, ObjectDelinkAgentDataBlock,
    ObjectDelinkObjectDataBlock, ObjectDelinkPacket, ObjectDescriptionAgentDataBlock,
    ObjectDescriptionObjectDataBlock, ObjectDescriptionPacket, ObjectLinkAgentDataBlock,
    ObjectLinkObjectDataBlock, ObjectLinkPacket, ObjectNameAgentDataBlock,
    ObjectNameObjectDataBlock, ObjectNamePacket,
};
use rustmetaverse_types::{Quaternion, Vector3, UUID};
use std::sync::Arc;
use tokio::sync::Mutex;

// ── P-Code (prim type) constants ─────────────────────────────────────────

pub const P_CODE_SPHERE: u8 = 0x20;
pub const P_CODE_BOX: u8 = 0x10;
pub const P_CODE_CYLINDER: u8 = 0x30;
pub const P_CODE_CONE: u8 = 0x40;
pub const P_CODE_TORUS: u8 = 0x50;
pub const P_CODE_PRISM: u8 = 0x60;
pub const P_CODE_SCULPT: u8 = 0x70;

// ── Material constants ───────────────────────────────────────────────────

pub const MATERIAL_STONE: u8 = 0;
pub const MATERIAL_METAL: u8 = 1;
pub const MATERIAL_GLASS: u8 = 2;
pub const MATERIAL_WOOD: u8 = 3;
pub const MATERIAL_FLESH: u8 = 4;
pub const MATERIAL_PLASTIC: u8 = 5;
pub const MATERIAL_RUBBER: u8 = 6;
pub const MATERIAL_LIGHT: u8 = 7;

/// Create a prim by raycasting from `ray_start` to `ray_end`.
#[allow(clippy::too_many_arguments)]
pub async fn create_prim(
    network: &Arc<NetworkManager>,
    simulator: &Arc<Mutex<Option<Simulator>>>,
    p_code: u8,
    material: u8,
    scale: Vector3,
    rotation: Quaternion,
    ray_start: Vector3,
    ray_end: Vector3,
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
    let mut packet = ObjectAddPacket {
        header: Header {
            frequency: PacketFrequency::Medium,
            id: 1,
            reliable: true,
            sequence: seq,
            ..Default::default()
        },
        agent_data: ObjectAddAgentDataBlock {
            agent_i_d: agent_id,
            session_i_d: session_id,
            group_i_d: UUID::ZERO,
        },
        object_data: ObjectAddObjectDataBlock {
            p_code,
            material,
            add_flags: 0,
            path_curve: 16,   // straight path
            profile_curve: 1, // circle profile
            path_begin: 0,
            path_end: 0,
            path_scale_x: 100,
            path_scale_y: 100,
            path_shear_x: 0,
            path_shear_y: 0,
            path_twist: 0,
            path_twist_begin: 0,
            path_radius_offset: 0,
            path_taper_x: 0,
            path_taper_y: 0,
            path_revolutions: 0,
            path_skew: 0,
            profile_begin: 0,
            profile_end: 0,
            profile_hollow: 0,
            bypass_raycast: 0,
            ray_start,
            ray_end,
            ray_target_i_d: UUID::ZERO,
            ray_end_is_intersection: 1,
            scale,
            rotation,
            state: 0,
        },
    };

    network.send_packet(&mut packet).await
}

/// Convenience: create a default box prim at the given position.
pub async fn create_box(
    network: &Arc<NetworkManager>,
    simulator: &Arc<Mutex<Option<Simulator>>>,
    position: Vector3,
    scale: Vector3,
) -> Result<(), std::io::Error> {
    create_prim(
        network,
        simulator,
        P_CODE_BOX,
        MATERIAL_WOOD,
        scale,
        Quaternion::IDENTITY,
        position - Vector3::new(0.0, 0.0, 3.0),
        position,
    )
    .await
}

/// Delete one or more objects by their local IDs.
pub async fn delete_objects(
    network: &Arc<NetworkManager>,
    simulator: &Arc<Mutex<Option<Simulator>>>,
    object_local_ids: &[u32],
    force: bool,
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
    let mut packet = ObjectDeletePacket {
        header: Header {
            frequency: PacketFrequency::Low,
            id: 89,
            reliable: true,
            sequence: seq,
            ..Default::default()
        },
        agent_data: ObjectDeleteAgentDataBlock {
            agent_i_d: agent_id,
            session_i_d: session_id,
            force,
        },
        object_data: object_local_ids
            .iter()
            .map(|&id| ObjectDeleteObjectDataBlock {
                object_local_i_d: id,
            })
            .collect(),
    };

    network.send_packet(&mut packet).await
}

/// Link multiple objects together by their local IDs. The first ID becomes
/// the root prim.
pub async fn link_objects(
    network: &Arc<NetworkManager>,
    simulator: &Arc<Mutex<Option<Simulator>>>,
    object_local_ids: &[u32],
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
    let mut packet = ObjectLinkPacket {
        header: Header {
            frequency: PacketFrequency::Low,
            id: 115,
            reliable: true,
            sequence: seq,
            ..Default::default()
        },
        agent_data: ObjectLinkAgentDataBlock {
            agent_i_d: agent_id,
            session_i_d: session_id,
        },
        object_data: object_local_ids
            .iter()
            .map(|&id| ObjectLinkObjectDataBlock {
                object_local_i_d: id,
            })
            .collect(),
    };

    network.send_packet(&mut packet).await
}

/// Delink (unlink) objects by their local IDs.
pub async fn delink_objects(
    network: &Arc<NetworkManager>,
    simulator: &Arc<Mutex<Option<Simulator>>>,
    object_local_ids: &[u32],
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
    let mut packet = ObjectDelinkPacket {
        header: Header {
            frequency: PacketFrequency::Low,
            id: 116,
            reliable: true,
            sequence: seq,
            ..Default::default()
        },
        agent_data: ObjectDelinkAgentDataBlock {
            agent_i_d: agent_id,
            session_i_d: session_id,
        },
        object_data: object_local_ids
            .iter()
            .map(|&id| ObjectDelinkObjectDataBlock {
                object_local_i_d: id,
            })
            .collect(),
    };

    network.send_packet(&mut packet).await
}

/// Set the name of an object by its local ID.
pub async fn set_object_name(
    network: &Arc<NetworkManager>,
    simulator: &Arc<Mutex<Option<Simulator>>>,
    object_local_id: u32,
    name: &str,
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
    let mut packet = ObjectNamePacket {
        header: Header {
            frequency: PacketFrequency::Low,
            id: 107,
            reliable: true,
            sequence: seq,
            ..Default::default()
        },
        agent_data: ObjectNameAgentDataBlock {
            agent_i_d: agent_id,
            session_i_d: session_id,
        },
        object_data: vec![ObjectNameObjectDataBlock {
            local_i_d: object_local_id,
            name: name.as_bytes().to_vec(),
        }],
    };

    network.send_packet(&mut packet).await
}

/// Set the description of an object by its local ID.
pub async fn set_object_description(
    network: &Arc<NetworkManager>,
    simulator: &Arc<Mutex<Option<Simulator>>>,
    object_local_id: u32,
    description: &str,
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
    let mut packet = ObjectDescriptionPacket {
        header: Header {
            frequency: PacketFrequency::Low,
            id: 108,
            reliable: true,
            sequence: seq,
            ..Default::default()
        },
        agent_data: ObjectDescriptionAgentDataBlock {
            agent_i_d: agent_id,
            session_i_d: session_id,
        },
        object_data: vec![ObjectDescriptionObjectDataBlock {
            local_i_d: object_local_id,
            description: description.as_bytes().to_vec(),
        }],
    };

    network.send_packet(&mut packet).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p_code_constants() {
        assert_eq!(P_CODE_BOX, 0x10);
        assert_eq!(P_CODE_SPHERE, 0x20);
        assert_eq!(P_CODE_CYLINDER, 0x30);
        assert_eq!(P_CODE_CONE, 0x40);
        assert_eq!(P_CODE_TORUS, 0x50);
    }

    #[test]
    fn material_constants() {
        assert_eq!(MATERIAL_STONE, 0);
        assert_eq!(MATERIAL_WOOD, 3);
        assert_eq!(MATERIAL_LIGHT, 7);
    }
}
