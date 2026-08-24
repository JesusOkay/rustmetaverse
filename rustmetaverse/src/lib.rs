//! A native Rust implementation of the Second Life / OpenSimulator
//! virtual-world protocol.
//!
//! This crate provides the client orchestration layer:
//!
//! - [`GridClient`]: the main entry point. Owns the network manager,
//!   packet dispatcher, and simulator session state. Drives login,
//!   circuit establishment, and the receive loop.
//! - [`login()`]: XML-RPC login to a grid's login endpoint.
//! - [`NetworkManager`]: the tokio-based UDP socket with an actor-pattern
//!   sender and atomic sequence numbering.
//! - [`PacketDispatcher`]: an async handler registry keyed by packet type.
//! - [`Simulator`]: session state for the connected region (agent ID,
//!   session ID, circuit code, seed capability).
//!
//! ## Subsystems
//!
//! - [`chat`]: local chat (ChatFromViewer / ChatFromSimulator)
//! - [`messaging`]: instant messaging (ImprovedInstantMessage)
//! - [`movement`]: avatar movement (AgentUpdate with control flags)
//! - [`appearance`]: avatar appearance (rebake, AvatarAppearance parsing)
//! - [`inventory`]: inventory operations (FetchInventoryDescendents)
//! - [`objects`]: object manipulation (create, delete, link, rename)
//! - [`groups`]: group operations (join, leave, profile)
//!
//! ## Example
//!
//! ```no_run
//! use rustmetaverse::{GridClient, LoginParams};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//! let client = GridClient::new().await?;
//! let params = LoginParams {
//!     first_name: "BotFirst".to_string(),
//!     last_name: "BotLast".to_string(),
//!     password: "password".to_string(),
//!     start: "last".to_string(),
//!     ..Default::default()
//! };
//! client.login(&params, "https://login.agni.lindenlab.com/cgi-bin/login.cgi").await?;
//! client.start_network_loop().await;
//! # Ok(())
//! # }
//! ```

pub mod appearance;
pub mod chat;
pub mod core_handlers;
pub mod grid_client;
pub mod groups;
pub mod inventory;
pub mod login;
pub mod messaging;
pub mod movement;
pub mod networking;
pub mod objects;
pub mod packet_dispatcher;
pub mod simulator;

pub use grid_client::GridClient;
pub use login::{login, LoginParams};
pub use networking::network_manager::NetworkManager;
pub use packet_dispatcher::PacketDispatcher;
pub use simulator::Simulator;
