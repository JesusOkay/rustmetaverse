//! Core packet handlers — handlers for essential packets beyond the
//! login/handshake sequence.
//!
//! These handlers are registered by `GridClient::new()` to process incoming
//! packets for:
//! - `AgentMovementComplete` — avatar position after entering a region
//! - `ChatFromSimulator` — local chat messages
//! - `HealthMessage` — avatar health updates
//! - `LogoutReply` — logout confirmation
//! - `DisableSimulator` — simulator is shutting down
//! - `UUIDNameReply` — display name resolution

use crate::simulator::Simulator;
use rustmetaverse_protocol::packets::WrappedPacket;
use rustmetaverse_types::UUID;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

/// Avatar position and look-at direction from `AgentMovementComplete`.
#[derive(Debug, Clone, Default)]
pub struct AvatarPosition {
    pub position: rustmetaverse_types::Vector3,
    pub look_at: rustmetaverse_types::Vector3,
    pub region_handle: u64,
    pub timestamp: u32,
}

/// State shared by core handlers.
#[derive(Default)]
pub struct CoreState {
    /// Latest avatar position (from AgentMovementComplete).
    pub avatar_position: RwLock<AvatarPosition>,
    /// Avatar health (from HealthMessage).
    pub health: RwLock<f32>,
    /// Display name cache: agent UUID → (first, last).
    pub display_names: RwLock<HashMap<UUID, (String, String)>>,
    /// Whether logout has been acknowledged.
    pub logout_confirmed: RwLock<bool>,
    /// Whether the simulator has sent DisableSimulator.
    pub simulator_disabled: RwLock<bool>,
}

impl CoreState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

/// Handle `AgentMovementComplete` — store the avatar's position.
pub async fn handle_agent_movement_complete(
    packet: &WrappedPacket,
    state: Arc<CoreState>,
    _simulator: Arc<Mutex<Option<Simulator>>>,
) {
    let WrappedPacket::AgentMovementComplete(movement) = packet else {
        return;
    };

    let pos = AvatarPosition {
        position: movement.data.position,
        look_at: movement.data.look_at,
        region_handle: movement.data.region_handle,
        timestamp: movement.data.timestamp,
    };

    log::info!(
        "AgentMovementComplete: position={:?} look_at={:?} region_handle={}",
        pos.position,
        pos.look_at,
        pos.region_handle
    );

    *state.avatar_position.write().await = pos;
}

/// Handle `ChatFromSimulator` — log and store the chat message.
pub async fn handle_chat_from_simulator(packet: &WrappedPacket) {
    let WrappedPacket::ChatFromSimulator(chat) = packet else {
        return;
    };

    let from_name = String::from_utf8_lossy(&chat.chat_data.from_name);
    let message = String::from_utf8_lossy(&chat.chat_data.message);

    log::info!("[CHAT] {}: {}", from_name.trim_end_matches('\0'), message);
}

/// Handle `HealthMessage` — store avatar health.
pub async fn handle_health_message(packet: &WrappedPacket, state: Arc<CoreState>) {
    let WrappedPacket::HealthMessage(health) = packet else {
        return;
    };

    let hp = health.health_data.health;
    log::debug!("Health: {}", hp);
    *state.health.write().await = hp;
}

/// Handle `LogoutReply` — confirm logout.
pub async fn handle_logout_reply(packet: &WrappedPacket, state: Arc<CoreState>) {
    let WrappedPacket::LogoutReply(reply) = packet else {
        return;
    };

    log::info!(
        "LogoutReply received from agent {:?}",
        reply.agent_data.agent_i_d
    );
    *state.logout_confirmed.write().await = true;
}

/// Handle `DisableSimulator` — mark simulator as disabled.
pub async fn handle_disable_simulator(state: Arc<CoreState>) {
    log::warn!("DisableSimulator received — simulator is shutting down");
    *state.simulator_disabled.write().await = true;
}

/// Handle `UUIDNameReply` — cache display names.
pub async fn handle_uuid_name_reply(packet: &WrappedPacket, state: Arc<CoreState>) {
    let WrappedPacket::UUIDNameReply(reply) = packet else {
        return;
    };

    let mut names = state.display_names.write().await;
    for block in &reply.u_u_i_d_name_block {
        let first = String::from_utf8_lossy(&block.first_name).to_string();
        let last = String::from_utf8_lossy(&block.last_name).to_string();
        log::debug!("Name reply: {} {} ({})", first, last, block.i_d);
        names.insert(block.i_d, (first, last));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_state_default() {
        let state = CoreState::default();
        assert_eq!(state.health.into_inner(), 0.0);
        assert!(!state.logout_confirmed.into_inner());
        assert!(!state.simulator_disabled.into_inner());
    }
}
