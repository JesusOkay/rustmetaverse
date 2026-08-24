//! Inventory operations — fetching folders, items, and managing inventory.
//!
//! The inventory system in SL/OpenSim works via the `FetchInventoryDescendents`
//! request and `InventoryDescendents` reply. The root folder UUID is obtained
//! from the login response and used as the starting point for traversal.

use crate::networking::network_manager::NetworkManager;
use crate::simulator::Simulator;
use rustmetaverse_protocol::header::{Header, PacketFrequency};
use rustmetaverse_protocol::packets::{
    FetchInventoryDescendentsAgentDataBlock, FetchInventoryDescendentsInventoryDataBlock,
    FetchInventoryDescendentsPacket, InventoryDescendentsFolderDataBlock,
    InventoryDescendentsItemDataBlock, WrappedPacket,
};
use rustmetaverse_types::UUID;
use std::sync::Arc;
use tokio::sync::Mutex;

// ── Inventory sort order constants ───────────────────────────────────────

pub const SORT_ORDER_BY_NAME: i32 = 0;
pub const SORT_ORDER_BY_DATE: i32 = 1;
pub const SORT_ORDER_FOLDERS_BY_NAME: i32 = 2;
pub const SORT_ORDER_SYSTEM_FOLDERS_TO_TOP: i32 = 4;

// ── Inventory item type constants ────────────────────────────────────────

pub const INV_TYPE_TEXTURE: i8 = 0;
pub const INV_TYPE_SOUND: i8 = 1;
pub const INV_TYPE_CALLING_CARD: i8 = 2;
pub const INV_TYPE_LANDMARK: i8 = 3;
pub const INV_TYPE_CLOTHING: i8 = 5;
pub const INV_TYPE_OBJECT: i8 = 6;
pub const INV_TYPE_NOTECARD: i8 = 7;
pub const INV_TYPE_LSL: i8 = 8;
pub const INV_TYPE_BODY_PART: i8 = 13;
pub const INV_TYPE_ANIMATION: i8 = 19;
pub const INV_TYPE_GESTURE: i8 = 20;

// ── Asset type constants ─────────────────────────────────────────────────

pub const ASSET_TYPE_TEXTURE: i8 = 0;
pub const ASSET_TYPE_SOUND: i8 = 1;
pub const ASSET_TYPE_CALLING_CARD: i8 = 2;
pub const ASSET_TYPE_LANDMARK: i8 = 3;
pub const ASSET_TYPE_CLOTHING: i8 = 5;
pub const ASSET_TYPE_OBJECT: i8 = 6;
pub const ASSET_TYPE_NOTECARD: i8 = 7;
pub const ASSET_TYPE_LSL_TEXT: i8 = 8;
pub const ASSET_TYPE_LSL_BYTECODE: i8 = 9;
pub const ASSET_TYPE_BODYPART: i8 = 13;
pub const ASSET_TYPE_ANIMATION: i8 = 19;
pub const ASSET_TYPE_GESTURE: i8 = 20;

/// An inventory folder received from `InventoryDescendents`.
#[derive(Debug, Clone)]
pub struct InventoryFolder {
    pub folder_id: UUID,
    pub parent_id: UUID,
    pub folder_type: i8,
    pub name: String,
}

/// An inventory item received from `InventoryDescendents`.
#[derive(Debug, Clone)]
pub struct InventoryItem {
    pub item_id: UUID,
    pub folder_id: UUID,
    pub creator_id: UUID,
    pub owner_id: UUID,
    pub group_id: UUID,
    pub base_mask: u32,
    pub owner_mask: u32,
    pub group_mask: u32,
    pub everyone_mask: u32,
    pub next_owner_mask: u32,
    pub group_owned: bool,
    pub asset_id: UUID,
    pub asset_type: i8,
    pub inv_type: i8,
    pub flags: u32,
    pub sale_type: u8,
    pub sale_price: i32,
    pub name: String,
    pub description: String,
    pub creation_date: i32,
    pub crc: u32,
}

/// The reply from a `FetchInventoryDescendents` request.
#[derive(Debug, Clone)]
pub struct InventoryDescendents {
    pub agent_id: UUID,
    pub folder_id: UUID,
    pub owner_id: UUID,
    pub version: i32,
    pub descendents: i32,
    pub folders: Vec<InventoryFolder>,
    pub items: Vec<InventoryItem>,
}

/// Request the contents of an inventory folder.
pub async fn fetch_folder(
    network: &Arc<NetworkManager>,
    simulator: &Arc<Mutex<Option<Simulator>>>,
    folder_id: UUID,
    owner_id: UUID,
    sort_order: i32,
    fetch_folders: bool,
    fetch_items: bool,
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
    let mut packet = FetchInventoryDescendentsPacket {
        header: Header {
            frequency: PacketFrequency::Low,
            id: 277,
            reliable: true,
            sequence: seq,
            ..Default::default()
        },
        agent_data: FetchInventoryDescendentsAgentDataBlock {
            agent_i_d: agent_id,
            session_i_d: session_id,
        },
        inventory_data: FetchInventoryDescendentsInventoryDataBlock {
            folder_i_d: folder_id,
            owner_i_d: owner_id,
            sort_order,
            fetch_folders,
            fetch_items,
        },
    };

    network.send_packet(&mut packet).await
}

/// Parse an `InventoryDescendents` packet into typed data.
pub fn parse_inventory_descendents(packet: &WrappedPacket) -> Option<InventoryDescendents> {
    let WrappedPacket::InventoryDescendents(desc) = packet else {
        return None;
    };

    let agent = &desc.agent_data;

    let folders: Vec<InventoryFolder> = desc
        .folder_data
        .iter()
        .map(|f: &InventoryDescendentsFolderDataBlock| InventoryFolder {
            folder_id: f.folder_i_d,
            parent_id: f.parent_i_d,
            folder_type: f.r#type,
            name: String::from_utf8_lossy(&f.name).to_string(),
        })
        .collect();

    let items: Vec<InventoryItem> = desc
        .item_data
        .iter()
        .map(|i: &InventoryDescendentsItemDataBlock| InventoryItem {
            item_id: i.item_i_d,
            folder_id: i.folder_i_d,
            creator_id: i.creator_i_d,
            owner_id: i.owner_i_d,
            group_id: i.group_i_d,
            base_mask: i.base_mask,
            owner_mask: i.owner_mask,
            group_mask: i.group_mask,
            everyone_mask: i.everyone_mask,
            next_owner_mask: i.next_owner_mask,
            group_owned: i.group_owned,
            asset_id: i.asset_i_d,
            asset_type: i.r#type,
            inv_type: i.inv_type,
            flags: i.flags,
            sale_type: i.sale_type,
            sale_price: i.sale_price,
            name: String::from_utf8_lossy(&i.name).to_string(),
            description: String::from_utf8_lossy(&i.description).to_string(),
            creation_date: i.creation_date,
            crc: i.c_r_c,
        })
        .collect();

    Some(InventoryDescendents {
        agent_id: agent.agent_i_d,
        folder_id: agent.folder_i_d,
        owner_id: agent.owner_i_d,
        version: agent.version,
        descendents: agent.descendents,
        folders,
        items,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inv_type_constants() {
        assert_eq!(INV_TYPE_TEXTURE, 0);
        assert_eq!(INV_TYPE_NOTECARD, 7);
        assert_eq!(INV_TYPE_LSL, 8);
        assert_eq!(INV_TYPE_ANIMATION, 19);
    }

    #[test]
    fn asset_type_constants() {
        assert_eq!(ASSET_TYPE_TEXTURE, 0);
        assert_eq!(ASSET_TYPE_OBJECT, 6);
        assert_eq!(ASSET_TYPE_ANIMATION, 19);
    }

    #[test]
    fn sort_order_constants() {
        assert_eq!(SORT_ORDER_BY_NAME, 0);
        assert_eq!(SORT_ORDER_BY_DATE, 1);
    }
}
