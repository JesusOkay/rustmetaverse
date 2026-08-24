//! Chat — sending and receiving local chat messages.
//!
//! The viewer sends `ChatFromViewer` to speak on local channels. The
//! simulator broadcasts `ChatFromSimulator` to all nearby avatars.
//!
//! Chat types (the `type` field in `ChatFromViewer`):
//! - 0 = Whisper
//! - 1 = Normal
//! - 2 = Shout
//! - 3 = Say (region message)
//! - 4 = Owner message
//! - 6 = Start typing
//! - 7 = Stop typing

use crate::networking::network_manager::NetworkManager;
use crate::simulator::Simulator;
use rustmetaverse_protocol::header::{Header, PacketFrequency};
use rustmetaverse_protocol::packets::{
    ChatFromSimulatorChatDataBlock, ChatFromViewerAgentDataBlock, ChatFromViewerChatDataBlock,
    ChatFromViewerPacket, WrappedPacket,
};
use rustmetaverse_types::UUID;
use std::sync::Arc;
use tokio::sync::Mutex;

// ── Chat type constants ──────────────────────────────────────────────────

pub const CHAT_TYPE_WHISPER: u8 = 0;
pub const CHAT_TYPE_NORMAL: u8 = 1;
pub const CHAT_TYPE_SHOUT: u8 = 2;
pub const CHAT_TYPE_SAY: u8 = 3;
pub const CHAT_TYPE_START_TYPING: u8 = 6;
pub const CHAT_TYPE_STOP_TYPING: u8 = 7;

// ── Chat source types (ChatFromSimulator) ────────────────────────────────

pub const CHAT_SOURCE_SYSTEM: u8 = 0;
pub const CHAT_SOURCE_AGENT: u8 = 1;
pub const CHAT_SOURCE_OBJECT: u8 = 2;

/// A received chat message from the simulator.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub from_name: String,
    pub source_id: UUID,
    pub owner_id: UUID,
    pub source_type: u8,
    pub chat_type: u8,
    pub audible: u8,
    pub position: rustmetaverse_types::Vector3,
    pub message: String,
}

/// Send a chat message on the given channel.
///
/// Channel 0 is public chat. Negative channels are scripts only (not
/// visible to avatars).
pub async fn send_chat(
    network: &Arc<NetworkManager>,
    simulator: &Arc<Mutex<Option<Simulator>>>,
    message: &str,
    chat_type: u8,
    channel: i32,
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
    let mut packet = ChatFromViewerPacket {
        header: Header {
            frequency: PacketFrequency::Low,
            id: 80,
            reliable: true,
            sequence: seq,
            ..Default::default()
        },
        agent_data: ChatFromViewerAgentDataBlock {
            agent_i_d: agent_id,
            session_i_d: session_id,
        },
        chat_data: ChatFromViewerChatDataBlock {
            message: std::borrow::Cow::Owned(message.as_bytes().to_vec()),
            r#type: chat_type,
            channel,
        },
    };

    network.send_packet(&mut packet).await
}

/// Convenience: say a message on public chat (channel 0).
pub async fn say(
    network: &Arc<NetworkManager>,
    simulator: &Arc<Mutex<Option<Simulator>>>,
    message: &str,
) -> Result<(), std::io::Error> {
    send_chat(network, simulator, message, CHAT_TYPE_NORMAL, 0).await
}

/// Convenience: shout a message on public chat (channel 0).
pub async fn shout(
    network: &Arc<NetworkManager>,
    simulator: &Arc<Mutex<Option<Simulator>>>,
    message: &str,
) -> Result<(), std::io::Error> {
    send_chat(network, simulator, message, CHAT_TYPE_SHOUT, 0).await
}

/// Convenience: whisper a message on public chat (channel 0).
pub async fn whisper(
    network: &Arc<NetworkManager>,
    simulator: &Arc<Mutex<Option<Simulator>>>,
    message: &str,
) -> Result<(), std::io::Error> {
    send_chat(network, simulator, message, CHAT_TYPE_WHISPER, 0).await
}

/// Parse a `ChatFromSimulator` packet into a [`ChatMessage`].
pub fn parse_chat_from_simulator(packet: &WrappedPacket) -> Option<ChatMessage> {
    if let WrappedPacket::ChatFromSimulator(chat) = packet {
        let chat_data: &ChatFromSimulatorChatDataBlock = &chat.chat_data;
        Some(ChatMessage {
            from_name: String::from_utf8_lossy(&chat_data.from_name).to_string(),
            source_id: chat_data.source_i_d,
            owner_id: chat_data.owner_i_d,
            source_type: chat_data.source_type,
            chat_type: chat_data.chat_type,
            audible: chat_data.audible,
            position: chat_data.position,
            message: String::from_utf8_lossy(&chat_data.message).to_string(),
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_type_constants() {
        assert_eq!(CHAT_TYPE_WHISPER, 0);
        assert_eq!(CHAT_TYPE_NORMAL, 1);
        assert_eq!(CHAT_TYPE_SHOUT, 2);
        assert_eq!(CHAT_TYPE_SAY, 3);
    }
}
