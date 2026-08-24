//! Group operations — joining, leaving, and querying group profiles.
//!
//! Group membership in SL is managed via the `JoinGroupRequest` /
//! `LeaveGroupRequest` packets and their corresponding reply packets.
//! Group profile data is fetched with `GroupProfileRequest`.

use crate::networking::network_manager::NetworkManager;
use crate::simulator::Simulator;
use rustmetaverse_protocol::header::{Header, PacketFrequency};
use rustmetaverse_protocol::packets::{
    GroupProfileReplyGroupDataBlock, JoinGroupReplyAgentDataBlock, JoinGroupReplyGroupDataBlock,
    JoinGroupRequestAgentDataBlock, JoinGroupRequestGroupDataBlock, JoinGroupRequestPacket,
    LeaveGroupReplyAgentDataBlock, LeaveGroupReplyGroupDataBlock, LeaveGroupRequestAgentDataBlock,
    LeaveGroupRequestGroupDataBlock, LeaveGroupRequestPacket, WrappedPacket,
};
use rustmetaverse_types::UUID;
use std::sync::Arc;
use tokio::sync::Mutex;

/// A group profile received from `GroupProfileReply`.
#[derive(Debug, Clone)]
pub struct GroupProfile {
    pub group_id: UUID,
    pub name: String,
    pub charter: String,
    pub show_in_list: bool,
    pub member_title: String,
    pub powers_mask: u64,
    pub insignia_id: UUID,
    pub founder_id: UUID,
    pub membership_fee: i32,
    pub open_enrollment: bool,
    pub money: i32,
    pub group_membership_count: i32,
    pub group_roles_count: i32,
    pub allow_publish: bool,
    pub mature_publish: bool,
    pub owner_role: UUID,
}

/// Join a group by its UUID.
pub async fn join_group(
    network: &Arc<NetworkManager>,
    simulator: &Arc<Mutex<Option<Simulator>>>,
    group_id: UUID,
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
    let mut packet = JoinGroupRequestPacket {
        header: Header {
            frequency: PacketFrequency::Low,
            id: 343,
            reliable: true,
            sequence: seq,
            ..Default::default()
        },
        agent_data: JoinGroupRequestAgentDataBlock {
            agent_i_d: agent_id,
            session_i_d: session_id,
        },
        group_data: JoinGroupRequestGroupDataBlock {
            group_i_d: group_id,
        },
    };

    network.send_packet(&mut packet).await
}

/// Leave a group by its UUID.
pub async fn leave_group(
    network: &Arc<NetworkManager>,
    simulator: &Arc<Mutex<Option<Simulator>>>,
    group_id: UUID,
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
    let mut packet = LeaveGroupRequestPacket {
        header: Header {
            frequency: PacketFrequency::Low,
            id: 347,
            reliable: true,
            sequence: seq,
            ..Default::default()
        },
        agent_data: LeaveGroupRequestAgentDataBlock {
            agent_i_d: agent_id,
            session_i_d: session_id,
        },
        group_data: LeaveGroupRequestGroupDataBlock {
            group_i_d: group_id,
        },
    };

    network.send_packet(&mut packet).await
}

/// Parse a `JoinGroupReply` packet.
pub fn parse_join_group_reply(packet: &WrappedPacket) -> Option<(UUID, bool)> {
    let WrappedPacket::JoinGroupReply(reply) = packet else {
        return None;
    };
    let agent: &JoinGroupReplyAgentDataBlock = &reply.agent_data;
    let group: &JoinGroupReplyGroupDataBlock = &reply.group_data;
    Some((agent.agent_i_d, group.success))
}

/// Parse a `LeaveGroupReply` packet.
pub fn parse_leave_group_reply(packet: &WrappedPacket) -> Option<(UUID, bool)> {
    let WrappedPacket::LeaveGroupReply(reply) = packet else {
        return None;
    };
    let agent: &LeaveGroupReplyAgentDataBlock = &reply.agent_data;
    let group: &LeaveGroupReplyGroupDataBlock = &reply.group_data;
    Some((agent.agent_i_d, group.success))
}

/// Parse a `GroupProfileReply` packet.
pub fn parse_group_profile_reply(packet: &WrappedPacket) -> Option<GroupProfile> {
    let WrappedPacket::GroupProfileReply(reply) = packet else {
        return None;
    };
    let group: &GroupProfileReplyGroupDataBlock = &reply.group_data;
    Some(GroupProfile {
        group_id: group.group_i_d,
        name: String::from_utf8_lossy(&group.name).to_string(),
        charter: String::from_utf8_lossy(&group.charter).to_string(),
        show_in_list: group.show_in_list,
        member_title: String::from_utf8_lossy(&group.member_title).to_string(),
        powers_mask: group.powers_mask,
        insignia_id: group.insignia_i_d,
        founder_id: group.founder_i_d,
        membership_fee: group.membership_fee,
        open_enrollment: group.open_enrollment,
        money: group.money,
        group_membership_count: group.group_membership_count,
        group_roles_count: group.group_roles_count,
        allow_publish: group.allow_publish,
        mature_publish: group.mature_publish,
        owner_role: group.owner_role,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_profile_roundtrip_fields() {
        let profile = GroupProfile {
            group_id: UUID::ZERO,
            name: "Test Group".to_string(),
            charter: "A test".to_string(),
            show_in_list: true,
            member_title: "Member".to_string(),
            powers_mask: 0,
            insignia_id: UUID::ZERO,
            founder_id: UUID::ZERO,
            membership_fee: 0,
            open_enrollment: false,
            money: 0,
            group_membership_count: 1,
            group_roles_count: 1,
            allow_publish: false,
            mature_publish: false,
            owner_role: UUID::ZERO,
        };
        assert_eq!(profile.name, "Test Group");
        assert_eq!(profile.group_membership_count, 1);
    }
}
