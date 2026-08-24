//! Instant messaging — sending and receiving IMs between avatars.
//!
//! `ImprovedInstantMessage` is the packet used for all IM communication:
//! private messages, group notices, teleport lures, friendship offers, etc.
//!
//! The `dialog` field selects the IM type:
//! - 0 = MessageFromAgent (normal private IM)
//! - 1 = MessageFromObject
//! - 15 = Invitation
//! - 19 = RequestTeleport (lure)
//! - 20 = GotoIm (accept lure)
//! - 21 = SessionSend (group chat)

use crate::networking::network_manager::NetworkManager;
use crate::simulator::Simulator;
use rustmetaverse_protocol::header::{Header, PacketFrequency};
use rustmetaverse_protocol::packets::{
    ImprovedInstantMessageAgentDataBlock, ImprovedInstantMessageEstateBlockBlock,
    ImprovedInstantMessageMessageBlockBlock, ImprovedInstantMessagePacket, WrappedPacket,
};
use rustmetaverse_types::{Vector3, UUID};
use std::sync::Arc;
use tokio::sync::Mutex;

// ── IM dialog types ──────────────────────────────────────────────────────

pub const IM_DIALOG_MESSAGE_FROM_AGENT: u8 = 0;
pub const IM_DIALOG_MESSAGE_FROM_OBJECT: u8 = 1;
pub const IM_DIALOG_GROUP_INVITATION: u8 = 15;
pub const IM_DIALOG_REQUEST_TELEPORT: u8 = 19;
pub const IM_DIALOG_GOTO_IM: u8 = 20;
pub const IM_DIALOG_SESSION_SEND: u8 = 21;
pub const IM_DIALOG_TYPING_START: u8 = 41;
pub const IM_DIALOG_TYPING_STOP: u8 = 42;
pub const IM_DIALOG_FRIENDSHIP_OFFERED: u8 = 38;
pub const IM_DIALOG_FRIENDSHIP_ACCEPTED: u8 = 39;
pub const IM_DIALOG_FRIENDSHIP_DECLINED: u8 = 40;

/// An incoming instant message.
#[derive(Debug, Clone)]
pub struct IncomingIM {
    pub from_agent_id: UUID,
    pub from_agent_name: String,
    pub to_agent_id: UUID,
    pub dialog: u8,
    pub im_id: UUID,
    pub timestamp: u32,
    pub message: String,
    pub binary_bucket: Vec<u8>,
}

/// Send a private instant message to another agent.
pub async fn send_im(
    network: &Arc<NetworkManager>,
    simulator: &Arc<Mutex<Option<Simulator>>>,
    target_id: UUID,
    message: &str,
    dialog: u8,
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

    let region_id = {
        let sim = simulator.lock().await;
        sim.as_ref().map(|s| s.region_id).unwrap_or_default()
    };

    let timestamp = chrono::Utc::now().timestamp() as u32;
    let im_id = UUID::new();

    let seq = network.get_next_sequence();
    let mut packet = ImprovedInstantMessagePacket {
        header: Header {
            frequency: PacketFrequency::Low,
            id: 254,
            reliable: true,
            sequence: seq,
            ..Default::default()
        },
        agent_data: ImprovedInstantMessageAgentDataBlock {
            agent_i_d: agent_id,
            session_i_d: session_id,
        },
        message_block: ImprovedInstantMessageMessageBlockBlock {
            from_group: false,
            to_agent_i_d: target_id,
            parent_estate_i_d: 1,
            region_i_d: region_id,
            position: Vector3::ZERO,
            offline: 0,
            dialog,
            i_d: im_id,
            timestamp,
            from_agent_name: Vec::new(), // filled by simulator from agent ID
            message: message.as_bytes().to_vec(),
            binary_bucket: Vec::new(),
        },
        estate_block: ImprovedInstantMessageEstateBlockBlock { estate_i_d: 1 },
        meta_data: Vec::new(),
    };

    network.send_packet(&mut packet).await
}

/// Convenience: send a normal private IM.
pub async fn send_private_im(
    network: &Arc<NetworkManager>,
    simulator: &Arc<Mutex<Option<Simulator>>>,
    target_id: UUID,
    message: &str,
) -> Result<(), std::io::Error> {
    send_im(
        network,
        simulator,
        target_id,
        message,
        IM_DIALOG_MESSAGE_FROM_AGENT,
    )
    .await
}

/// Send a teleport lure to another agent.
pub async fn send_teleport_lure(
    network: &Arc<NetworkManager>,
    simulator: &Arc<Mutex<Option<Simulator>>>,
    target_id: UUID,
    message: &str,
) -> Result<(), std::io::Error> {
    send_im(
        network,
        simulator,
        target_id,
        message,
        IM_DIALOG_REQUEST_TELEPORT,
    )
    .await
}

/// Parse an incoming `ImprovedInstantMessage` packet.
pub fn parse_im(packet: &WrappedPacket) -> Option<IncomingIM> {
    if let WrappedPacket::ImprovedInstantMessage(im) = packet {
        let mb = &im.message_block;
        Some(IncomingIM {
            from_agent_id: im.agent_data.agent_i_d,
            from_agent_name: String::from_utf8_lossy(&mb.from_agent_name).to_string(),
            to_agent_id: mb.to_agent_i_d,
            dialog: mb.dialog,
            im_id: mb.i_d,
            timestamp: mb.timestamp,
            message: String::from_utf8_lossy(&mb.message).to_string(),
            binary_bucket: mb.binary_bucket.clone(),
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn im_dialog_constants() {
        assert_eq!(IM_DIALOG_MESSAGE_FROM_AGENT, 0);
        assert_eq!(IM_DIALOG_REQUEST_TELEPORT, 19);
        assert_eq!(IM_DIALOG_FRIENDSHIP_OFFERED, 38);
    }
}
