use crate::networking::network_manager::NetworkManager;
use crate::simulator::Simulator;
use futures::future::BoxFuture;
use rustmetaverse_protocol::packets::{PacketType, WrappedPacket};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

// Single-box handler: Box<dyn Fn -> BoxFuture> instead of the previous
// Box<dyn Fn -> Pin<Box<dyn Future>>> which allocated twice per dispatch.
pub type PacketHandlerFn = Box<
    dyn Fn(
            WrappedPacket,
            Arc<NetworkManager>,
            Arc<Mutex<Option<Simulator>>>,
        ) -> BoxFuture<'static, ()>
        + Send
        + Sync,
>;

pub struct PacketDispatcher {
    handlers: HashMap<PacketType, Vec<PacketHandlerFn>>,
}

impl Default for PacketDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl PacketDispatcher {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    pub fn add_handler<F, Fut>(&mut self, packet_type: PacketType, handler: F)
    where
        F: Fn(WrappedPacket, Arc<NetworkManager>, Arc<Mutex<Option<Simulator>>>) -> Fut
            + Send
            + Sync
            + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let wrapped = Box::new(move |p, n, s| Box::pin(handler(p, n, s)) as BoxFuture<'static, ()>);
        self.handlers.entry(packet_type).or_default().push(wrapped);
    }

    pub async fn dispatch(
        &self,
        packet: WrappedPacket,
        network: Arc<NetworkManager>,
        simulator: Arc<Mutex<Option<Simulator>>>,
    ) {
        let packet_type = packet.packet_type();
        if let Some(handlers) = self.handlers.get(&packet_type) {
            match handlers.len() {
                0 => {}
                1 => {
                    handlers[0](packet, network, simulator).await;
                }
                n => {
                    // All handlers except the last get a clone.
                    for handler in &handlers[..n - 1] {
                        handler(packet.clone(), network.clone(), simulator.clone()).await;
                    }
                    // Last handler gets ownership.
                    handlers[n - 1](packet, network, simulator).await;
                }
            }
        }
    }
}
