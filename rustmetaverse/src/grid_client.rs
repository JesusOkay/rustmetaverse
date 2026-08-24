use crate::core_handlers::CoreState;
use crate::login::{login, LoginParams};
use crate::networking::network_manager::NetworkManager;
use crate::packet_dispatcher::PacketDispatcher;
use crate::simulator::Simulator;
use bytes::Bytes;
use rustmetaverse_protocol::header::{Header, PacketFrequency};
use rustmetaverse_protocol::packets::{
    AgentHeightWidthAgentDataBlock, AgentHeightWidthHeightWidthBlockBlock, AgentHeightWidthPacket,
    CompleteAgentMovementAgentDataBlock, CompleteAgentMovementPacket, CompletePingCheckPacket,
    CompletePingCheckPingIDBlock, LogoutRequestAgentDataBlock, LogoutRequestPacket,
    PacketAckPacket, PacketAckPacketsBlock, PacketType, RegionHandshakeReplyAgentDataBlock,
    RegionHandshakeReplyPacket, RegionHandshakeReplyRegionInfoBlock,
    UseCircuitCodeCircuitCodeBlock, UseCircuitCodePacket, WrappedPacket,
};
use rustmetaverse_types::{Quaternion, Vector3, UUID};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};

/// Timeout for the UseCircuitCode acknowledgement. The reliable-resend layer
/// (250 ms poll interval, SRTT/RTTVAR-based RTO clamped to 250 ms–3 s, max 5
/// resends) handles retransmission automatically, so we only need a single
/// generous timeout to cover worst-case 5 resends + network latency.
const USE_CIRCUIT_CODE_TIMEOUT: tokio::time::Duration = tokio::time::Duration::from_secs(15);

/// AgentUpdate send interval (~10 Hz, matching the viewer cadence).
const MOVEMENT_TICK: tokio::time::Duration = tokio::time::Duration::from_millis(100);

pub struct GridClient {
    pub network: Arc<NetworkManager>,
    pub simulator: Arc<Mutex<Option<Simulator>>>,
    pub dispatcher: Arc<PacketDispatcher>,
    /// Set once the simulator has acknowledged the UDP circuit with a region handshake.
    pub region_ready: Arc<AtomicBool>,
    /// Shared state populated by core packet handlers.
    pub core_state: Arc<CoreState>,
    /// Shared movement state — set `control_flags` to drive the avatar.
    /// The movement loop reads this every tick and sends AgentUpdate at ~10 Hz.
    /// Set to `CONTROL_STOP` (0x00400000) to stop all movement.
    pub movement_flags: Arc<std::sync::atomic::AtomicU32>,
    /// Set to true to terminate the movement loop.
    movement_stop: Arc<AtomicBool>,
    reliable_acks: Arc<Mutex<HashMap<u32, oneshot::Sender<()>>>>,
    network_loop_started: AtomicBool,
    movement_loop_started: AtomicBool,
}

impl GridClient {
    pub async fn new() -> Result<Self, std::io::Error> {
        let network = NetworkManager::new("0.0.0.0:0").await?;
        let mut dispatcher = PacketDispatcher::new();
        let region_ready = Arc::new(AtomicBool::new(false));
        let reliable_acks: Arc<Mutex<HashMap<u32, oneshot::Sender<()>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        // Create the simulator handle up front so handlers can capture it.
        let simulator: Arc<Mutex<Option<Simulator>>> = Arc::new(Mutex::new(None));

        // Firestorm's message system tracks the acknowledgements for every
        // reliable message. The login sequence specifically waits for the
        // UseCircuitCode acknowledgement before sending CompleteAgentMovement.
        let reliable_acks_handler = reliable_acks.clone();
        dispatcher.add_handler(PacketType::PacketAck, move |packet, network, _simulator| {
            let reliable_acks = reliable_acks_handler.clone();
            async move {
                let WrappedPacket::PacketAck(packet_ack) = packet else {
                    return;
                };

                // Remove acknowledged packets from the resend queue.
                let seqs: Vec<u32> = packet_ack.packets.iter().map(|p| p.i_d).collect();
                network.need_acks.lock().await.ack_many(&seqs);

                // Notify any callers waiting for a specific ack.
                let mut pending = reliable_acks.lock().await;
                for acknowledged in packet_ack.packets {
                    if let Some(sender) = pending.remove(&acknowledged.i_d) {
                        let _ = sender.send(());
                    }
                }
            }
        });

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
                                log::error!("Failed to send RegionHandshakeReply: {}", e);
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
                                    log::error!("Failed to send AgentHeightWidth: {}", e);
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

        // ── Core packet handlers ────────────────────────────────────────

        let core_state = CoreState::new();

        // AgentMovementComplete — avatar position after entering a region
        let mc_state = core_state.clone();
        let mc_simulator = simulator.clone();
        dispatcher.add_handler(
            PacketType::AgentMovementComplete,
            move |packet, _network, _simulator| {
                let state = mc_state.clone();
                let sim = mc_simulator.clone();
                async move {
                    crate::core_handlers::handle_agent_movement_complete(&packet, state, sim).await;
                }
            },
        );

        // ChatFromSimulator — local chat
        dispatcher.add_handler(
            PacketType::ChatFromSimulator,
            move |packet, _network, _simulator| async move {
                crate::core_handlers::handle_chat_from_simulator(&packet).await;
            },
        );

        // HealthMessage — avatar health
        let health_state = core_state.clone();
        dispatcher.add_handler(
            PacketType::HealthMessage,
            move |packet, _network, _simulator| {
                let state = health_state.clone();
                async move {
                    crate::core_handlers::handle_health_message(&packet, state).await;
                }
            },
        );

        // LogoutReply — logout confirmation
        let logout_state = core_state.clone();
        dispatcher.add_handler(
            PacketType::LogoutReply,
            move |packet, _network, _simulator| {
                let state = logout_state.clone();
                async move {
                    crate::core_handlers::handle_logout_reply(&packet, state).await;
                }
            },
        );

        // DisableSimulator — simulator shutting down
        let disable_state = core_state.clone();
        dispatcher.add_handler(
            PacketType::DisableSimulator,
            move |_packet, _network, _simulator| {
                let state = disable_state.clone();
                async move {
                    crate::core_handlers::handle_disable_simulator(state).await;
                }
            },
        );

        // UUIDNameReply — display name resolution
        let name_state = core_state.clone();
        dispatcher.add_handler(
            PacketType::UUIDNameReply,
            move |packet, _network, _simulator| {
                let state = name_state.clone();
                async move {
                    crate::core_handlers::handle_uuid_name_reply(&packet, state).await;
                }
            },
        );

        Ok(Self {
            network: Arc::new(network),
            simulator,
            dispatcher: Arc::new(dispatcher),
            region_ready,
            core_state,
            movement_flags: Arc::new(std::sync::atomic::AtomicU32::new(
                crate::movement::CONTROL_STOP,
            )),
            movement_stop: Arc::new(AtomicBool::new(false)),
            reliable_acks,
            network_loop_started: AtomicBool::new(false),
            movement_loop_started: AtomicBool::new(false),
        })
    }

    /// Register a packet handler. Must be called before `start_network_loop`.
    pub fn add_handler<F, Fut>(&self, packet_type: PacketType, handler: F)
    where
        F: Fn(WrappedPacket, Arc<NetworkManager>, Arc<Mutex<Option<Simulator>>>) -> Fut
            + Send
            + Sync
            + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        // SAFETY: This is only safe before start_network_loop is called,
        // because the dispatcher is not yet shared with the receive task.
        // After that, the Arc is shared and get_mut returns None.
        if let Some(dispatcher) = Arc::get_mut(&mut Arc::clone(&self.dispatcher)) {
            dispatcher.add_handler(packet_type, handler);
        } else {
            log::error!("Cannot add handler after network loop has started");
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
                        if len < 7 {
                            log::debug!("Skipping truncated packet: {} bytes", len);
                            continue;
                        }
                        let data = Bytes::copy_from_slice(&buf[..len]);

                        let mut header_data = data.clone();
                        let header = match Header::deserialize(&mut header_data) {
                            Ok(h) => h,
                            Err(e) => {
                                log::debug!(
                                    "Malformed packet header: {} (first bytes: {:02x?})",
                                    e,
                                    data.iter().take(10).collect::<Vec<_>>()
                                );
                                continue;
                            }
                        };

                        if !header.acks.is_empty() {
                            // Remove from the resend queue
                            network.need_acks.lock().await.ack_many(&header.acks);

                            // Notify any callers waiting for a specific ack
                            let mut pending = reliable_acks.lock().await;
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
                                    sequence: network.get_next_sequence(),
                                    ..Default::default()
                                },
                                packets: vec![PacketAckPacketsBlock {
                                    i_d: header.sequence,
                                }],
                            };
                            if let Err(error) = network.send_packet(&mut acknowledgement).await {
                                log::debug!("Could not acknowledge reliable packet: {error}");
                            }
                        }

                        // decode_packet takes Bytes by value (no internal clone
                        // on the non-zerocoded path). We pass a clone because
                        // we still need the original data below for error logging.
                        match rustmetaverse_protocol::packets::decode_packet(data.clone()) {
                            Ok(packet) => {
                                log::trace!(
                                    "Decoded packet type: {:?} (seq {})",
                                    packet.packet_type(),
                                    header.sequence
                                );
                                dispatcher
                                    .dispatch(packet, network.clone(), simulator.clone())
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
                    }
                    Err(e) => {
                        log::error!("Network error: {}", e);
                        break;
                    }
                }
            }
        });
    }

    /// Start the continuous movement loop — sends `AgentUpdate` at ~10 Hz,
    /// using the avatar position from `AgentMovementComplete` as the camera
    /// center and the shared `movement_flags` for control flags.
    ///
    /// This is started automatically by [`login()`](Self::login). Call
    /// [`stop_movement_loop()`](Self::stop_movement_loop) to terminate it.
    ///
    /// The loop is idempotent — calling it twice is a no-op.
    pub async fn start_movement_loop(&self) {
        if self.movement_loop_started.swap(true, Ordering::AcqRel) {
            return;
        }

        let network = self.network.clone();
        let simulator = self.simulator.clone();
        let core_state = self.core_state.clone();
        let movement_flags = self.movement_flags.clone();
        let movement_stop = self.movement_stop.clone();

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(MOVEMENT_TICK);
            ticker.tick().await; // skip first immediate tick
            loop {
                if movement_stop.load(Ordering::Acquire) {
                    break;
                }
                ticker.tick().await;

                // Skip if no simulator is connected.
                {
                    let sim = simulator.lock().await;
                    if sim.is_none() {
                        continue;
                    }
                }

                // Use the latest known avatar position from
                // AgentMovementComplete as the camera center. If we haven't
                // received it yet, default to origin.
                let camera_center = {
                    let pos = core_state.avatar_position.read().await;
                    pos.position
                };
                let camera_at_axis = Vector3::new(1.0, 0.0, 0.0);

                let control_flags = movement_flags.load(Ordering::Acquire);

                // If control_flags is CONTROL_STOP, we still send AgentUpdate
                // with the STOP flag — this tells the simulator to halt the
                // avatar. The viewer does the same: it never stops sending
                // AgentUpdate, it just sends with CONTROL_STOP when idle.
                if let Err(e) = crate::movement::send_movement(
                    &network,
                    &simulator,
                    control_flags,
                    Quaternion::IDENTITY,
                    camera_center,
                    camera_at_axis,
                )
                .await
                {
                    log::debug!("Movement loop send error: {e}");
                }
            }
        });
    }

    /// Stop the movement loop (started by `start_movement_loop`).
    /// This does NOT send a final AgentUpdate with CONTROL_STOP — the
    /// simulator will time out the avatar on its own.
    pub fn stop_movement_loop(&self) {
        self.movement_stop.store(true, Ordering::Release);
    }

    /// Set the movement control flags. The movement loop picks this up on
    /// its next tick (~100 ms latency).
    ///
    /// Use `movement::CONTROL_AT_POS` for forward, `CONTROL_STOP` for idle, etc.
    /// Flags can be OR-ed together: `CONTROL_AT_POS | CONTROL_FAST_AT`.
    pub fn set_movement_flags(&self, flags: u32) {
        self.movement_flags.store(flags, Ordering::Release);
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

            // Send once — the reliable-resend layer (reliability.rs) handles
            // automatic retransmission with MSG_RESENT flag and adaptive RTO.
            self.network.send_packet(&mut use_circuit_code).await?;

            // Wait for the PacketAck. The resend loop will keep retrying
            // underneath us; we just need to observe the ack within the
            // timeout window.
            let circuit_confirmed =
                match tokio::time::timeout(USE_CIRCUIT_CODE_TIMEOUT, &mut ack_receiver).await {
                    Ok(Ok(())) => true,
                    Ok(Err(_)) => false,
                    Err(_) => false,
                };
            self.reliable_acks.lock().await.remove(&sequence);

            if !circuit_confirmed {
                return Err("The simulator did not acknowledge UseCircuitCode within 15s.".into());
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

            // Start the continuous movement loop. It sends AgentUpdate at
            // ~10 Hz using the avatar position from AgentMovementComplete as
            // the camera center. The initial control_flags is CONTROL_STOP
            // (idle); callers set movement via set_movement_flags().
            self.start_movement_loop().await;
        } else {
            return Err("Invalid login response format".into());
        }

        Ok(())
    }

    /// Disconnect from the simulator by sending `LogoutRequest`.
    ///
    /// After calling this, the `LogoutReply` handler sets
    /// `core_state.logout_confirmed` to `true`.
    pub async fn logout(&self) -> Result<(), std::io::Error> {
        // Stop the movement loop before logging out.
        self.stop_movement_loop();

        let (agent_id, session_id) = {
            let sim = self.simulator.lock().await;
            if let Some(s) = sim.as_ref() {
                (s.client, s.session_id)
            } else {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotConnected,
                    "No simulator connected",
                ));
            }
        };

        let seq = self.network.get_next_sequence();
        let mut packet = LogoutRequestPacket {
            header: Header {
                frequency: PacketFrequency::Low,
                id: 252,
                reliable: true,
                sequence: seq,
                ..Default::default()
            },
            agent_data: LogoutRequestAgentDataBlock {
                agent_i_d: agent_id,
                session_i_d: session_id,
            },
        };

        self.network.send_packet(&mut packet).await?;
        log::info!("Sent LogoutRequest");
        Ok(())
    }

    /// Send local chat on channel 0.
    pub async fn say(&self, message: &str) -> Result<(), std::io::Error> {
        crate::chat::say(&self.network, &self.simulator, message).await
    }

    /// Send local chat on channel 0 with shout volume.
    pub async fn shout(&self, message: &str) -> Result<(), std::io::Error> {
        crate::chat::shout(&self.network, &self.simulator, message).await
    }

    /// Send a private instant message to another agent.
    pub async fn send_im(&self, target_id: UUID, message: &str) -> Result<(), std::io::Error> {
        crate::messaging::send_private_im(&self.network, &self.simulator, target_id, message).await
    }

    /// Request the simulator to rebake avatar textures.
    pub async fn rebake(&self, texture_id: UUID) -> Result<(), std::io::Error> {
        crate::appearance::request_rebake(&self.network, &self.simulator, texture_id).await
    }

    /// Fetch the contents of an inventory folder.
    pub async fn fetch_inventory_folder(
        &self,
        folder_id: UUID,
        owner_id: UUID,
    ) -> Result<(), std::io::Error> {
        crate::inventory::fetch_folder(
            &self.network,
            &self.simulator,
            folder_id,
            owner_id,
            crate::inventory::SORT_ORDER_BY_NAME,
            true,
            true,
        )
        .await
    }

    /// Join a group by its UUID.
    pub async fn join_group(&self, group_id: UUID) -> Result<(), std::io::Error> {
        crate::groups::join_group(&self.network, &self.simulator, group_id).await
    }

    /// Leave a group by its UUID.
    pub async fn leave_group(&self, group_id: UUID) -> Result<(), std::io::Error> {
        crate::groups::leave_group(&self.network, &self.simulator, group_id).await
    }
}
