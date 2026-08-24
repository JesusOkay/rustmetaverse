//! Reliable packet delivery — resend unacknowledged reliable packets.
//!
//! LLUDP implements a selective-repeat ARQ protocol. Every reliable packet
//! is stored with a retransmission timer. If no `PacketAck` arrives before
//! the RTO expires, the packet is resent with the `MSG_RESENT` flag. The RTO
//! is calculated from smoothed round-trip time (SRTT) and RTT variance
//! (RTTVAR), clamped between 250 ms and 3 000 ms.
//!
//! This mirrors the design used by libopenmetaverse / libremetaverse
//! (`UnackedPacketCollection`) and OpenSimulator's LLUDP stack.

use bytes::BytesMut;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Minimum retransmission timeout (250 ms), matching OpenSimulator.
pub const MIN_RTO: Duration = Duration::from_millis(250);

/// Maximum retransmission timeout (3 s), matching OpenSimulator.
pub const MAX_RTO: Duration = Duration::from_millis(3000);

/// Initial RTO before we have RTT measurements.
const INITIAL_RTO: Duration = Duration::from_secs(1);

/// Maximum resend attempts before giving up.
const MAX_RESEND_COUNT: u8 = 5;

/// A packet awaiting acknowledgement.
#[derive(Clone)]
pub struct UnackedPacket {
    /// The serialized packet bytes, ready to resend.
    pub data: BytesMut,
    /// The sequence number from the header.
    pub sequence: u32,
    /// When the packet was last sent.
    pub last_sent: Instant,
    /// How many times we have resent it.
    pub resend_count: u8,
}

/// Round-trip time statistics used to compute the RTO.
#[derive(Debug, Clone, Copy, Default)]
pub struct RttStats {
    /// Smoothed round-trip time (SRTT).
    pub srtt: f64,
    /// Round-trip time variance (RTTVAR).
    pub rttvar: f64,
}

impl RttStats {
    /// Update with a new RTT sample (RFC 6298 algorithm, simplified).
    pub fn update(&mut self, rtt: Duration) {
        let r = rtt.as_secs_f64();
        if self.srtt == 0.0 {
            // First measurement
            self.srtt = r;
            self.rttvar = r / 2.0;
        } else {
            // SRTT ← ⅞·SRTT + ⅛·R
            self.srtt = 0.875 * self.srtt + 0.125 * r;
            // RTTVAR ← ¾·RTTVAR + ¼·|SRTT − R|
            let diff = (self.srtt - r).abs();
            self.rttvar = 0.75 * self.rttvar + 0.25 * diff;
        }
    }

    /// Compute the current RTO from SRTT and RTTVAR, clamped to [MIN_RTO, MAX_RTO].
    pub fn rto(&self) -> Duration {
        if self.srtt == 0.0 {
            return INITIAL_RTO;
        }
        let rto_secs = self.srtt + 4.0 * self.rttvar;
        let rto = Duration::from_secs_f64(rto_secs);
        rto.clamp(MIN_RTO, MAX_RTO)
    }
}

/// Tracks reliable packets that have been sent but not yet acknowledged.
pub struct NeedAcks {
    packets: HashMap<u32, UnackedPacket>,
    rtt_stats: RttStats,
}

impl NeedAcks {
    pub fn new() -> Self {
        Self {
            packets: HashMap::new(),
            rtt_stats: RttStats::default(),
        }
    }

    /// Record a newly sent reliable packet.
    pub fn add(&mut self, sequence: u32, data: BytesMut) {
        self.packets.insert(
            sequence,
            UnackedPacket {
                data,
                sequence,
                last_sent: Instant::now(),
                resend_count: 0,
            },
        );
    }

    /// Remove an acknowledged packet and update RTT stats.
    pub fn ack(&mut self, sequence: u32) -> Option<UnackedPacket> {
        if let Some(pkt) = self.packets.remove(&sequence) {
            let rtt = pkt.last_sent.elapsed();
            self.rtt_stats.update(rtt);
            Some(pkt)
        } else {
            None
        }
    }

    /// Acknowledge multiple sequences (from a PacketAck or appended acks).
    pub fn ack_many(&mut self, sequences: &[u32]) {
        for seq in sequences {
            self.ack(*seq);
        }
    }

    /// Return packets whose RTO has expired and should be resent.
    /// Removes packets that have exceeded MAX_RESEND_COUNT.
    pub fn get_resends(&mut self) -> Vec<UnackedPacket> {
        let rto = self.rtt_stats.rto();
        let now = Instant::now();
        let mut resends = Vec::new();

        self.packets.retain(|_, pkt| {
            if pkt.resend_count >= MAX_RESEND_COUNT {
                log::warn!(
                    "Reliable packet seq {} exceeded {} resends, dropping",
                    pkt.sequence,
                    MAX_RESEND_COUNT
                );
                return false;
            }
            if now.duration_since(pkt.last_sent) >= rto {
                let mut p = pkt.clone();
                p.resend_count += 1;
                p.last_sent = now;
                resends.push(p);
                // Keep tracking it (with updated count/timestamp)
                pkt.resend_count += 1;
                pkt.last_sent = now;
            }
            true
        });

        resends
    }

    /// Current RTO based on measured RTT.
    pub fn rto(&self) -> Duration {
        self.rtt_stats.rto()
    }

    /// Number of packets awaiting acknowledgement.
    pub fn len(&self) -> usize {
        self.packets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.packets.is_empty()
    }
}

impl Default for NeedAcks {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared reliable delivery state, protected by a tokio Mutex.
pub type SharedNeedAcks = Arc<Mutex<NeedAcks>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rtt_stats_initial_rto() {
        let stats = RttStats::default();
        assert_eq!(stats.rto(), INITIAL_RTO);
    }

    #[test]
    fn rtt_stats_after_measurement() {
        let mut stats = RttStats::default();
        stats.update(Duration::from_millis(100));
        let rto = stats.rto();
        assert!(rto >= MIN_RTO, "RTO should be at least MIN_RTO");
        assert!(rto <= MAX_RTO, "RTO should be at most MAX_RTO");
    }

    #[test]
    fn rtt_stats_clamped() {
        let mut stats = RttStats::default();
        // Very large RTT → RTO clamped to MAX
        stats.update(Duration::from_secs(100));
        assert_eq!(stats.rto(), MAX_RTO);

        // Feed many small RTTs to drive SRTT down → RTO clamped to MIN
        for _ in 0..100 {
            stats.update(Duration::from_micros(1));
        }
        assert_eq!(stats.rto(), MIN_RTO);
    }

    #[test]
    fn need_acks_add_and_ack() {
        let mut need_acks = NeedAcks::new();
        need_acks.add(1, BytesMut::new());
        assert_eq!(need_acks.len(), 1);
        let acked = need_acks.ack(1);
        assert!(acked.is_some());
        assert!(need_acks.is_empty());
    }

    #[test]
    fn need_acks_ack_unknown() {
        let mut need_acks = NeedAcks::new();
        let acked = need_acks.ack(42);
        assert!(acked.is_none());
    }

    #[test]
    fn need_acks_get_resends_empty() {
        let mut need_acks = NeedAcks::new();
        let resends = need_acks.get_resends();
        assert!(resends.is_empty());
    }
}
