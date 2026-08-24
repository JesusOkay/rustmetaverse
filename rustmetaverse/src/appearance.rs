//! Avatar appearance — requesting rebakes and handling appearance data.
//!
//! Baked textures and visual parameters are the core of the SL appearance
//! system. The viewer sends `AgentSetAppearance` with serialized visual
//! parameters and baked texture IDs. The simulator sends `AvatarAppearance`
//! to nearby clients to announce the avatar's visual state.
//!
//! This module provides:
//! - `request_rebake()` — ask the simulator to rebake textures
//! - `parse_avatar_appearance()` — extract appearance data from incoming packets
//! - `AvatarAppearance` struct — typed representation of the appearance packet

use crate::networking::network_manager::NetworkManager;
use crate::simulator::Simulator;
use rustmetaverse_protocol::header::{Header, PacketFrequency};
use rustmetaverse_protocol::packets::{
    AvatarAppearanceAttachmentBlockBlock, AvatarAppearanceObjectDataBlock,
    AvatarAppearanceSenderBlock, RebakeAvatarTexturesPacket, RebakeAvatarTexturesTextureDataBlock,
    WrappedPacket,
};
use rustmetaverse_types::{Vector3, UUID};
use std::sync::Arc;
use tokio::sync::Mutex;

// ── Wearable type constants ──────────────────────────────────────────────

pub const WEARABLE_SHAPE: u8 = 0;
pub const WEARABLE_SKIN: u8 = 1;
pub const WEARABLE_HAIR: u8 = 2;
pub const WEARABLE_EYES: u8 = 3;
pub const WEARABLE_SHIRT: u8 = 4;
pub const WEARABLE_PANTS: u8 = 5;
pub const WEARABLE_SHOES: u8 = 6;
pub const WEARABLE_SOCKS: u8 = 7;
pub const WEARABLE_JACKET: u8 = 8;
pub const WEARABLE_GLOVES: u8 = 9;
pub const WEARABLE_UNDERSHIRT: u8 = 10;
pub const WEARABLE_UNDERPANTS: u8 = 11;
pub const WEARABLE_SKIRT: u8 = 12;
pub const WEARABLE_ALPHA: u8 = 13;
pub const WEARABLE_TATTOO: u8 = 14;
pub const WEARABLE_PHYSICS: u8 = 15;

// ── Baked texture indices ────────────────────────────────────────────────

pub const BAKED_HEAD: u8 = 0;
pub const BAKED_UPPER: u8 = 1;
pub const BAKED_LOWER: u8 = 2;
pub const BAKED_EYES: u8 = 3;
pub const BAKED_HAIR: u8 = 4;
pub const BAKED_SKIRT: u8 = 5;

/// Typed avatar appearance data extracted from the `AvatarAppearance` packet.
#[derive(Debug, Clone)]
pub struct AvatarAppearance {
    pub sender_id: UUID,
    pub is_trial: bool,
    pub texture_entry: Vec<u8>,
    pub param_value: u8,
    pub appearance_version: u8,
    pub cof_version: i32,
    pub flags: u32,
    pub hover_height: Vector3,
    pub attachments: Vec<AvatarAttachment>,
}

/// An attachment on an avatar.
#[derive(Debug, Clone)]
pub struct AvatarAttachment {
    pub item_id: UUID,
    pub attachment_point: u8,
}

/// Request the simulator to rebake the avatar's textures.
pub async fn request_rebake(
    network: &Arc<NetworkManager>,
    simulator: &Arc<Mutex<Option<Simulator>>>,
    texture_id: UUID,
) -> Result<(), std::io::Error> {
    {
        let sim = simulator.lock().await;
        if sim.is_none() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "No simulator connected",
            ));
        }
    }

    let seq = network.get_next_sequence();
    let mut packet = RebakeAvatarTexturesPacket {
        header: Header {
            frequency: PacketFrequency::Low,
            id: 87,
            reliable: true,
            sequence: seq,
            ..Default::default()
        },
        texture_data: RebakeAvatarTexturesTextureDataBlock {
            texture_i_d: texture_id,
        },
    };

    network.send_packet(&mut packet).await
}

/// Parse an `AvatarAppearance` packet into a typed [`AvatarAppearance`].
pub fn parse_avatar_appearance(packet: &WrappedPacket) -> Option<AvatarAppearance> {
    let WrappedPacket::AvatarAppearance(appearance) = packet else {
        return None;
    };

    let sender: &AvatarAppearanceSenderBlock = &appearance.sender;
    let object_data: &AvatarAppearanceObjectDataBlock = &appearance.object_data;

    let attachments: Vec<AvatarAttachment> = appearance
        .attachment_block
        .iter()
        .map(
            |a: &AvatarAppearanceAttachmentBlockBlock| AvatarAttachment {
                item_id: a.i_d,
                attachment_point: a.attachment_point,
            },
        )
        .collect();

    // visual_param, appearance_data, appearance_hover are Vecs — use first element if present
    let param_value = appearance
        .visual_param
        .first()
        .map(|v| v.param_value)
        .unwrap_or(0);
    let (appearance_version, cof_version, flags) = appearance
        .appearance_data
        .first()
        .map(|d| (d.appearance_version, d.cof_version, d.flags))
        .unwrap_or((0, 0, 0));
    let hover_height = appearance
        .appearance_hover
        .first()
        .map(|h| h.hover_height)
        .unwrap_or_default();

    Some(AvatarAppearance {
        sender_id: sender.i_d,
        is_trial: sender.is_trial,
        texture_entry: object_data.texture_entry.clone(),
        param_value,
        appearance_version,
        cof_version,
        flags,
        hover_height,
        attachments,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wearable_constants() {
        assert_eq!(WEARABLE_SHAPE, 0);
        assert_eq!(WEARABLE_SKIN, 1);
        assert_eq!(WEARABLE_HAIR, 2);
    }

    #[test]
    fn baked_constants() {
        assert_eq!(BAKED_HEAD, 0);
        assert_eq!(BAKED_UPPER, 1);
        assert_eq!(BAKED_HAIR, 4);
    }
}
