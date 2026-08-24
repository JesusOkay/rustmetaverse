use bytes::BytesMut;
use rustmetaverse_protocol::header::Header;
use rustmetaverse_protocol::packets::Packet;
use std::io;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

use super::reliability::{NeedAcks, SharedNeedAcks};

pub struct NetworkManager {
    pub socket: Arc<UdpSocket>,
    sequence: AtomicU32,
    tx_sender: mpsc::Sender<BytesMut>,
    /// Reliable packets awaiting acknowledgement, shared with the resend loop.
    pub need_acks: SharedNeedAcks,
}

impl NetworkManager {
    pub async fn new(bind_addr: &str) -> Result<Self, io::Error> {
        let socket = Arc::new(UdpSocket::bind(bind_addr).await?);

        // Create a channel for outgoing packets (Actor pattern)
        // Capacity 1000 to absorb bursts without blocking the game logic
        let (tx, mut rx) = mpsc::channel::<BytesMut>(1000);

        let socket_clone = socket.clone();
        // Spawn the Sender Task (The "Write Half")
        // This task owns the write responsibility, eliminating lock contention on the socket
        tokio::spawn(async move {
            while let Some(data) = rx.recv().await {
                if let Err(e) = socket_clone.send(&data).await {
                    log::error!("Error sending packet: {}", e);
                }
            }
        });

        let need_acks: SharedNeedAcks = Arc::new(tokio::sync::Mutex::new(NeedAcks::new()));

        // Spawn the resend loop. Every 250 ms it checks for expired RTOs
        // and resends reliable packets that haven't been acknowledged.
        let resend_need_acks = need_acks.clone();
        let resend_socket = socket.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(tokio::time::Duration::from_millis(250));
            ticker.tick().await; // skip first immediate tick
            loop {
                ticker.tick().await;
                let resends = {
                    let mut guard = resend_need_acks.lock().await;
                    guard.get_resends()
                };
                for pkt in resends {
                    // Set the RESENT flag on the serialized packet.
                    // The flags byte is at offset 0; MSG_RESENT = 0x20.
                    let mut data = pkt.data.clone();
                    if !data.is_empty() {
                        data[0] |= Header::MSG_RESENT;
                    }
                    log::debug!(
                        "Resending reliable packet seq {} (attempt {})",
                        pkt.sequence,
                        pkt.resend_count
                    );
                    if let Err(e) = resend_socket.send(&data).await {
                        log::error!("Error resending packet: {}", e);
                    }
                }
            }
        });

        Ok(Self {
            socket,
            sequence: AtomicU32::new(0),
            tx_sender: tx,
            need_acks,
        })
    }

    pub async fn connect(&self, ip: &str, port: u16) -> Result<(), io::Error> {
        self.socket.connect(format!("{}:{}", ip, port)).await
    }

    pub fn get_next_sequence(&self) -> u32 {
        // Atomic increment - Zero lock contention
        self.sequence.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub async fn send_packet<P: Packet>(&self, packet: &mut P) -> Result<(), io::Error> {
        let mut buf = BytesMut::with_capacity(1024);
        packet.serialize(&mut buf);

        // Track reliable packets for resend. We read the flags byte (offset 0)
        // and sequence number (offset 1..5) from the serialized buffer to
        // avoid needing a header() accessor on the Packet trait.
        if buf.len() >= 5 && (buf[0] & Header::MSG_RELIABLE) != 0 {
            let sequence = u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]);
            self.need_acks.lock().await.add(sequence, buf.clone());
        }

        self.tx_sender
            .send(buf)
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "Send channel closed"))?;
        Ok(())
    }
}
