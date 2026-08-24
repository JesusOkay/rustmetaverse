use rustmetaverse_types::UUID;
use std::net::SocketAddr;

#[derive(Debug)]
pub struct Simulator {
    pub client: UUID, // The agent ID connected
    pub ip: SocketAddr,
    pub name: String,
    pub region_id: UUID,
    pub circuit_code: u32,
    pub session_id: UUID,
    pub seed_capability: String,
}

impl Simulator {
    pub fn new(
        ip: SocketAddr,
        circuit_code: u32,
        session_id: UUID,
        client: UUID,
        seed_capability: String,
    ) -> Self {
        Self {
            client,
            ip,
            name: String::new(),
            region_id: UUID::default(),
            circuit_code,
            session_id,
            seed_capability,
        }
    }
}
