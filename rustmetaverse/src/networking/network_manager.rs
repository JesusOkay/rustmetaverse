use bytes::BytesMut;
use rustmetaverse_protocol::packets::Packet;
use std::io;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

pub struct NetworkManager {
    pub socket: Arc<UdpSocket>,
    sequence: AtomicU32,
    tx_sender: mpsc::Sender<BytesMut>,
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
                    eprintln!("Error sending packet: {}", e);
                }
            }
        });

        Ok(Self {
            socket,
            sequence: AtomicU32::new(0),
            tx_sender: tx,
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
        let mut buf = BytesMut::with_capacity(4096);
        packet.serialize(&mut buf);

        // Send to the channel instead of locking the socket
        self.tx_sender
            .send(buf)
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "Send channel closed"))?;
        Ok(())
    }
}
