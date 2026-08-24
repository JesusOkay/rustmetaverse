use bytes::{Buf, BufMut, Bytes, BytesMut};
use serde::{Deserialize, Serialize};
use std::io;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PacketFrequency {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Header {
    pub reliable: bool,
    pub resent: bool,
    pub zerocoded: bool,
    pub appended_acks: bool,
    pub sequence: u32,
    pub id: u16,
    pub frequency: PacketFrequency,
    pub acks: Vec<u32>,
}

impl Default for Header {
    fn default() -> Self {
        Header {
            reliable: false,
            resent: false,
            zerocoded: false,
            appended_acks: false,
            sequence: 0,
            id: 0,
            frequency: PacketFrequency::Low,
            acks: Vec::new(),
        }
    }
}

impl Header {
    pub const MSG_RELIABLE: u8 = 0x40;
    pub const MSG_RESENT: u8 = 0x20;
    pub const MSG_ZEROCODED: u8 = 0x80;
    pub const MSG_APPENDED_ACKS: u8 = 0x10;

    pub fn serialize(&self, buf: &mut BytesMut) {
        let mut flags = 0u8;
        if self.reliable {
            flags |= Self::MSG_RELIABLE;
        }
        if self.resent {
            flags |= Self::MSG_RESENT;
        }
        if self.zerocoded {
            flags |= Self::MSG_ZEROCODED;
        }
        if self.appended_acks {
            flags |= Self::MSG_APPENDED_ACKS;
        }

        buf.put_u8(flags);
        buf.put_u32(self.sequence); // Big Endian
        buf.put_u8(0); // Extra byte (padding)

        match self.frequency {
            PacketFrequency::Low => {
                buf.put_u8(0xFF);
                buf.put_u8(0xFF);
                buf.put_u16(self.id); // Big Endian
            }
            PacketFrequency::Medium => {
                buf.put_u8(0xFF);
                buf.put_u8(self.id as u8);
            }
            PacketFrequency::High => {
                buf.put_u8(self.id as u8);
            }
        }
    }

    pub fn write_acks(&self, buf: &mut BytesMut) {
        if self.appended_acks && !self.acks.is_empty() {
            for ack in &self.acks {
                buf.put_u32(*ack); // Big Endian
            }
            buf.put_u8(self.acks.len() as u8);
        }
    }

    pub fn deserialize(buf: &mut Bytes) -> Result<Self, io::Error> {
        if buf.len() < 6 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Packet too short",
            ));
        }

        let flags = buf[0];
        let reliable = (flags & Self::MSG_RELIABLE) != 0;
        let resent = (flags & Self::MSG_RESENT) != 0;
        let zerocoded = (flags & Self::MSG_ZEROCODED) != 0;
        let appended_acks = (flags & Self::MSG_APPENDED_ACKS) != 0;

        let sequence = u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]);

        // buf[5] is extra byte

        let mut pos = 6;
        // Check bounds for ID
        if buf.len() < pos + 1 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Packet too short for ID",
            ));
        }

        let (frequency, id) = if buf[pos] == 0xFF {
            if buf.len() < pos + 2 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Packet too short for Medium ID",
                ));
            }
            if buf[pos + 1] == 0xFF {
                // Low
                if buf.len() < pos + 4 {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "Packet too short for Low ID",
                    ));
                }
                let id = u16::from_be_bytes([buf[pos + 2], buf[pos + 3]]);
                pos += 4;
                (PacketFrequency::Low, id)
            } else {
                // Medium
                let id = buf[pos + 1] as u16;
                pos += 2;
                (PacketFrequency::Medium, id)
            }
        } else {
            // High
            let id = buf[pos] as u16;
            pos += 1;
            (PacketFrequency::High, id)
        };

        buf.advance(pos);

        let mut acks = Vec::new();
        if appended_acks && !buf.is_empty() {
            let count = buf[buf.len() - 1] as usize;
            let acks_size = count * 4 + 1;
            if buf.len() >= acks_size {
                let split_idx = buf.len() - acks_size;
                let acks_buf = buf.split_off(split_idx);
                // acks_buf contains [Ack1][Ack2]...[Count]

                for i in 0..count {
                    let start = i * 4;
                    let ack = u32::from_be_bytes([
                        acks_buf[start],
                        acks_buf[start + 1],
                        acks_buf[start + 2],
                        acks_buf[start + 3],
                    ]);
                    acks.push(ack);
                }
            }
        }

        Ok(Header {
            reliable,
            resent,
            zerocoded,
            appended_acks,
            sequence,
            id,
            frequency,
            acks,
        })
    }
}
