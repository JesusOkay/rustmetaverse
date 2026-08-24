use crate::login::{login, LoginParams};
use crate::networking::network_manager::NetworkManager;
use crate::packet_dispatcher::PacketDispatcher;
use crate::simulator::Simulator;
use bytes::Bytes;
use rustmetaverse_protocol::header::{Header, PacketFrequency};
use rustmetaverse_protocol::packets::{
    AgentHeightWidthAgentDataBlock, AgentHeightWidthHeightWidthBlockBlock, AgentHeightWidthPacket,
    CompleteAgentMovementAgentDataBlock, CompleteAgentMovementPacket, CompletePingCheckPacket,
    CompletePingCheckPingIDBlock, PacketAckPacket, PacketAckPacketsBlock, PacketType,
    RegionHandshakeReplyAgentDataBlock, RegionHandshakeReplyPacket,
    RegionHandshakeReplyRegionInfoBlock, UseCircuitCodeCircuitCodeBlock, UseCircuitCodePacket,
    WrappedPacket,
};
use rustmetaverse_types::UUID;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex, RwLock};

const USE_CIRCUIT_CODE_RETRIES: usize = 3;
const USE_CIRCUIT_CODE_TIMEOUT: tokio::time::Duration = tokio::time::Duration::from_secs(5);

pub struct GridClient {
    pub network: Arc<NetworkManager>,
    pub simulator: Arc<Mutex<Option<Simulator>>>,
    pub dispatcher: Arc<RwLock<PacketDispatcher>>,
    /// Set once the simulator has acknowledged the UDP circuit with a region handshake.
    pub region_ready: Arc<AtomicBool>,
    reliable_acks: Arc<Mutex<HashMap<u32, oneshot::Sender<()>>>>,
    network_loop_started: AtomicBool,
}

impl GridClient {
    pub async fn new() -> Self {
        let network = NetworkManager::new("0.0.0.0:0").await.unwrap();
        let mut dispatcher = PacketDispatcher::new();
        let region_ready = Arc::new(AtomicBool::new(false));
        let reliable_acks: Arc<Mutex<HashMap<u32, oneshot::Sender<()>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // Firestorm's message system tracks the acknowledgements for every
        // reliable message. The login sequence specifically waits for the
        // UseCircuitCode acknowledgement before sending CompleteAgentMovement.
        let reliable_acks_handler = reliable_acks.clone();
        dispatcher.add_handler(
            PacketType::PacketAck,
            move |packet, _network, _simulator| {
                let reliable_acks = reliable_acks_handler.clone();
                async move {
                    let WrappedPacket::PacketAck(packet_ack) = packet else {
                        return;
                    };

                    let mut pending = reliable_acks.lock().await;
                    for acknowledged in packet_ack.packets {
                        if let Some(sender) = pending.remove(&acknowledged.i_d) {
                            let _ = sender.send(());
                        }
                    }
                }
            },
        );

        dispatcher.add_handler(
            PacketType::OpenCircuit,
            |_packet, _network, _simulator| async move {
                // Silently handle OpenCircuit
            },
        );

        dispatcher.add_handler(
            PacketType::StartPingCheck,
            |packet, network, _simulator| async move {
                if let WrappedPacket::StartPingCheck(ping) = packet {
                    let ping_id = ping.ping_i_d.ping_i_d;
                    let seq_to_ack = ping.header.sequence;
                    let is_reliable = ping.header.reliable;
                    // Silently handle ping

                    let seq = network.get_next_sequence();

                    let mut acks = Vec::new();
                    if is_reliable {
                        acks.push(seq_to_ack);
                    }

                    let mut reply = CompletePingCheckPacket {
                        header: Header {
                            frequency: PacketFrequency::High,
                            id: 2,
                            reliable: false,
                            sequence: seq,
                            appended_acks: !acks.is_empty(),
                            acks,
                            ..Default::default()
                        },
                        ping_i_d: CompletePingCheckPingIDBlock { ping_i_d: ping_id },
                    };

                    if let Err(_e) = network.send_packet(&mut reply).await {
                        // Silently ignore ping errors
                    }
                }
            },
        );

        let region_ready_handler = region_ready.clone();
        dispatcher.add_handler(
            PacketType::RegionHandshake,
            move |packet, network, simulator| {
                let region_ready = region_ready_handler.clone();
                async move {
                    if let WrappedPacket::RegionHandshake(handshake) = packet {
                        let region_name =
                            String::from_utf8_lossy(&handshake.region_info.sim_name).to_string();
                        let region_id = handshake.region_info2.region_i_d;
                        log::debug!(
                            "Received RegionHandshake for region '{}' ({})",
                            region_name.trim_end_matches('\0'),
                            region_id
                        );

                        // Persist the region details on the simulator so callers
                        // can read them after the handshake completes.
                        {
                            let mut sim_guard = simulator.lock().await;
                            if let Some(sim) = sim_guard.as_mut() {
                                sim.name = region_name.clone();
                                sim.region_id = region_id;
                            }
                        }

                        let (agent_id, session_id) = {
                            let sim_guard = simulator.lock().await;
                            if let Some(sim) = sim_guard.as_ref() {
                                (sim.client, sim.session_id)
                            } else {
                                (UUID::default(), UUID::default())
                            }
                        };

                        if !agent_id.is_zero() {
                            let seq = network.get_next_sequence();

                            let mut reply = RegionHandshakeReplyPacket {
                                header: Header {
                                    frequency: PacketFrequency::Low,
                                    id: 149,
                                    reliable: true,
                                    sequence: seq,
                                    ..Default::default()
                                },
                                agent_data: RegionHandshakeReplyAgentDataBlock {
                                    agent_i_d: agent_id,
                                    session_i_d: session_id,
                                },
                                region_info: RegionHandshakeReplyRegionInfoBlock {
                                    flags: 0, // TODO: Set flags
                                },
                            };

                            if let Err(e) = network.send_packet(&mut reply).await {
                                eprintln!("Failed to send RegionHandshakeReply: {}", e);
                            } else {
                                log::debug!("Sent RegionHandshakeReply");

                                // Send AgentHeightWidth
                                let seq = network.get_next_sequence();
                                let (circuit_code, _) = {
                                    let sim_guard = simulator.lock().await;
                                    if let Some(sim) = sim_guard.as_ref() {
                                        (sim.circuit_code, ())
                                    } else {
                                        (0, ())
                                    }
                                };

                                let mut height_width = AgentHeightWidthPacket {
                                    header: Header {
                                        frequency: PacketFrequency::Low,
                                        // AgentHeightWidth is Low 83 in the same message
                                        // template used by Firestorm. Low 150 is an
                                        // unrelated SimulatorViewerTimeMessage.
                                        id: 83,
                                        reliable: true,
                                        sequence: seq,
                                        ..Default::default()
                                    },
                                    agent_data: AgentHeightWidthAgentDataBlock {
                                        agent_i_d: agent_id,
                                        session_i_d: session_id,
                                        circuit_code,
                                    },
                                    height_width_block: AgentHeightWidthHeightWidthBlockBlock {
                                        gen_counter: 0,
                                        height: 169, // Standard height
                                        width: 45,   // Standard width
                                    },
                                };

                                if let Err(e) = network.send_packet(&mut height_width).await {
                                    eprintln!("Failed to send AgentHeightWidth: {}", e);
                                } else {
                                    log::debug!("Sent AgentHeightWidth");
                                    // Firestorm is now able to use the circuit. Do not
                                    // expose it as ready before its handshake reply has
                                    // been sent.
                                    region_ready.store(true, Ordering::Release);
                                }
                            }
                        }
                    }
                }
            },
        );

        Self {
            network: Arc::new(network),
            simulator: Arc::new(Mutex::new(None)),
            dispatcher: Arc::new(RwLock::new(dispatcher)),
            region_ready,
            reliable_acks,
            network_loop_started: AtomicBool::new(false),
        }
    }

    pub async fn start_network_loop(&self) {
        if self.network_loop_started.swap(true, Ordering::AcqRel) {
            return;
        }

        let network = self.network.clone();
        let simulator = self.simulator.clone();
        let dispatcher = self.dispatcher.clone();
        let reliable_acks = self.reliable_acks.clone();
        let socket = network.socket.clone();

        tokio::spawn(async move {
            let mut buf = vec![0u8; 65536];
            loop {
                match socket.recv(&mut buf).await {
                    Ok(len) => {
                        log::trace!("Received {} network bytes", len);
                        // Minimum packet size: header is at least 7 bytes. Skip truncated packets.
                        if len < 7 {
                            log::debug!("Skipping truncated packet: {} bytes", len);
                            continue;
                        }
                        let data = Bytes::copy_from_slice(&buf[..len]);
                        let network_clone = network.clone();
                        let simulator_clone = simulator.clone();
                        let dispatcher_clone = dispatcher.clone();
                        let reliable_acks_clone = reliable_acks.clone();

                        tokio::spawn(async move {
                            // LLUDP reliable packets must be acknowledged. Firestorm's
                            // message system handles this automatically; do it before
                            // dispatching so directory and handshake replies are retained.
                            //
                            // Header::deserialize and decode_packet both return Result
                            // and perform bounds-checked reads, so a malformed or
                            // truncated packet produces a structured error rather than
                            // a panic.
                            let mut header_data = data.clone();
                            let header = match Header::deserialize(&mut header_data) {
                                Ok(h) => h,
                                Err(e) => {
                                    log::debug!(
                                        "Malformed packet header: {} (first bytes: {:02x?})",
                                        e,
                                        data.iter().take(10).collect::<Vec<_>>()
                                    );
                                    return;
                                }
                            };

                            // An acknowledgement can be appended to any incoming
                            // message, not just PacketAck.
                            if !header.acks.is_empty() {
                                let mut pending = reliable_acks_clone.lock().await;
                                for sequence in &header.acks {
                                    if let Some(sender) = pending.remove(sequence) {
                                        let _ = sender.send(());
                                    }
                                }
                            }

                            if header.reliable {
                                let mut acknowledgement = PacketAckPacket {
                                    header: Header {
                                        frequency: PacketFrequency::Low,
                                        id: 65531,
                                        sequence: network_clone.get_next_sequence(),
                                        ..Default::default()
                                    },
                                    packets: vec![PacketAckPacketsBlock {
                                        i_d: header.sequence,
                                    }],
                                };
                                if let Err(error) =
                                    network_clone.send_packet(&mut acknowledgement).await
                                {
                                    log::debug!("Could not acknowledge reliable packet: {error}");
                                }
                            }

                            let mut data_mut = data.clone();
                            match rustmetaverse_protocol::packets::decode_packet(&mut data_mut) {
                                Ok(packet) => {
                                    log::trace!(
                                        "Decoded packet type: {:?} (seq {})",
                                        packet.packet_type(),
                                        header.sequence
                                    );
                                    let dispatch = dispatcher_clone.read().await;
                                    dispatch
                                        .dispatch(packet, network_clone, simulator_clone)
                                        .await;
                                }
                                Err(e) => {
                                    log::debug!(
                                        "Error decoding packet (seq {}): {} (first bytes: {:02x?})",
                                        header.sequence,
                                        e,
                                        data.iter().take(10).collect::<Vec<_>>()
                                    );
                                }
                            }
                        });
                    }
                    Err(e) => {
                        eprintln!("Network error: {}", e);
                        break;
                    }
                }
            }
        });
    }

    pub async fn login(
        &self,
        params: &LoginParams,
        login_uri: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.login_internal(params, login_uri, false).await
    }

    pub async fn login_silent(
        &self,
        params: &LoginParams,
        login_uri: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.login_internal(params, login_uri, true).await
    }

    async fn login_internal(
        &self,
        params: &LoginParams,
        login_uri: &str,
        silent: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let response = login(params, login_uri).await.map_err(|e| {
            let err: Box<dyn std::error::Error + Send + Sync> =
                Box::new(std::io::Error::other(e.to_string()));
            err
        })?;

        if let Some(map) = response.as_map() {
            if let Some(login_true) = map.get("login").and_then(|v| v.as_boolean()) {
                if !login_true {
                    let msg = map
                        .get("message")
                        .and_then(|v| v.as_string())
                        .unwrap_or("Login failed".to_string());
                    return Err(msg.into());
                }
            } else {
                return Err("Invalid login response".into());
            }

            if !silent {
                println!("Login successful!");
            }

            let circuit_code = map
                .get("circuit_code")
                .and_then(|v| v.as_integer())
                .unwrap_or(0) as u32;
            let session_id_str = map
                .get("session_id")
                .and_then(|v| v.as_string())
                .unwrap_or_default();
            let agent_id_str = map
                .get("agent_id")
                .and_then(|v| v.as_string())
                .unwrap_or_default();
            let sim_ip = map
                .get("sim_ip")
                .and_then(|v| v.as_string())
                .unwrap_or_default();
            let sim_port = map
                .get("sim_port")
                .and_then(|v| v.as_integer())
                .unwrap_or(0) as u16;
            let seed_capability = map
                .get("seed_capability")
                .and_then(|v| v.as_string())
                .unwrap_or_default();

            if !silent {
                println!("Circuit Code: {}", circuit_code);
                println!("Session ID: {}", session_id_str);
                println!("Agent ID: {}", agent_id_str);
                println!("Sim IP: {}", sim_ip);
                println!("Sim Port: {}", sim_port);
                println!("Seed Capability: {}", seed_capability);
            }

            let session_id = UUID::parse(&session_id_str).unwrap_or(UUID::ZERO);
            let agent_id = UUID::parse(&agent_id_str).unwrap_or(UUID::ZERO);

            {
                let ip_addr: std::net::IpAddr =
                    sim_ip.parse().map_err(|e: std::net::AddrParseError| {
                        let err: Box<dyn std::error::Error + Send + Sync> = Box::new(e);
                        err
                    })?;
                let socket_addr = SocketAddr::new(ip_addr, sim_port);

                let mut sim_guard = self.simulator.lock().await;
                *sim_guard = Some(Simulator::new(
                    socket_addr,
                    circuit_code,
                    session_id,
                    agent_id,
                    seed_capability.to_string(),
                ));
            }

            self.region_ready.store(false, Ordering::Release);
            self.network.connect(&sim_ip, sim_port).await.map_err(|e| {
                let err: Box<dyn std::error::Error + Send + Sync> = Box::new(e);
                err
            })?;

            // Start receiving before UseCircuitCode. The old flow only started
            // this loop after CompleteAgentMovement, so it could never observe
            // the acknowledgement Firestorm waits for.
            self.start_network_loop().await;

            let sequence = self.network.get_next_sequence();
            let mut use_circuit_code = UseCircuitCodePacket {
                header: Header {
                    frequency: PacketFrequency::Low,
                    id: 3,
                    reliable: true,
                    sequence,
                    ..Default::default()
                },
                circuit_code: UseCircuitCodeCircuitCodeBlock {
                    code: circuit_code,
                    session_i_d: session_id,
                    i_d: agent_id,
                },
            };
            let (ack_sender, mut ack_receiver) = oneshot::channel();
            self.reliable_acks.lock().await.insert(sequence, ack_sender);

            let mut circuit_confirmed = false;
            for attempt in 0..=USE_CIRCUIT_CODE_RETRIES {
                use_circuit_code.header.resent = attempt > 0;
                self.network.send_packet(&mut use_circuit_code).await?;

                match tokio::time::timeout(USE_CIRCUIT_CODE_TIMEOUT, &mut ack_receiver).await {
                    Ok(Ok(())) => {
                        circuit_confirmed = true;
                        break;
                    }
                    Ok(Err(_)) => break,
                    Err(_) => {}
                }
            }
            self.reliable_acks.lock().await.remove(&sequence);

            if !circuit_confirmed {
                return Err(
                    "The simulator did not acknowledge UseCircuitCode after 4 attempts.".into(),
                );
            }

            // Firestorm performs this only after the circuit acknowledgement.
            let mut complete_movement = CompleteAgentMovementPacket {
                header: Header {
                    frequency: PacketFrequency::Low,
                    id: 249,
                    reliable: true,
                    sequence: self.network.get_next_sequence(),
                    ..Default::default()
                },
                agent_data: CompleteAgentMovementAgentDataBlock {
                    agent_i_d: agent_id,
                    session_i_d: session_id,
                    circuit_code,
                },
            };
            self.network.send_packet(&mut complete_movement).await?;
            if !silent {
                println!("Simulator circuit established.");
            }
        } else {
            return Err("Invalid login response format".into());
        }

        Ok(())
    }
}
