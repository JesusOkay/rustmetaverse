use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::io;

/// The first six bytes are the LLUDP packet header and must remain unencoded.
const LLUDP_PACKET_HEADER_SIZE: usize = 6;

pub fn expand_zerocoded(data: &mut Bytes) -> Result<Bytes, io::Error> {
    let mut output = BytesMut::with_capacity(data.len() * 2); // Heuristic

    while data.has_remaining() {
        let byte = data.get_u8();
        if byte == 0x00 {
            if !data.has_remaining() {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "ZeroCoding: missing count byte",
                ));
            }
            let count = data.get_u8();
            if count == 0 {
                // 0x00 0x00 is not a valid sequence in standard ZeroCoding usually,
                // but if it means 0 zeros, we just write nothing?
                // In some implementations 0x00 0x00 might be an escape for something else,
                // but usually it means "insert 0 zeros" which is a no-op, or it's invalid.
                // Let's assume it inserts 'count' zeros.
            }
            output.put_bytes(0x00, count as usize);
        } else {
            output.put_u8(byte);
        }
    }

    Ok(output.freeze())
}

/// Applies LLUDP zero coding when it makes the message smaller.
///
/// The message template marks `DirFindQuery` as `Zerocoded`. Firestorm encodes
/// runs of zero bytes after the fixed packet header as `0x00, count`, then sets
/// the zerocoded bit in the header. Templates also permit sending an unencoded
/// packet if this transformation would not save space.
pub fn zero_encode(data: &[u8]) -> BytesMut {
    if data.len() <= LLUDP_PACKET_HEADER_SIZE {
        return BytesMut::from(data);
    }

    let mut encoded = BytesMut::with_capacity(data.len());
    encoded.extend_from_slice(&data[..LLUDP_PACKET_HEADER_SIZE]);

    let mut zero_count = 0u8;
    for byte in &data[LLUDP_PACKET_HEADER_SIZE..] {
        if *byte == 0 {
            if zero_count == 0 {
                encoded.put_u8(0);
                zero_count = 1;
            } else if zero_count == 254 {
                // The current zero is the 255th byte in this run.
                encoded.put_u8(255);
                zero_count = 0;
            } else {
                zero_count += 1;
            }
        } else {
            if zero_count != 0 {
                encoded.put_u8(zero_count);
                zero_count = 0;
            }
            encoded.put_u8(*byte);
        }
    }

    if zero_count != 0 {
        encoded.put_u8(zero_count);
    }

    if encoded.len() < data.len() {
        encoded[0] |= 0x80; // LL_ZERO_CODE_FLAG
        encoded
    } else {
        BytesMut::from(data)
    }
}

#[cfg(test)]
mod tests {
    use super::{expand_zerocoded, zero_encode, LLUDP_PACKET_HEADER_SIZE};
    use bytes::Bytes;

    #[test]
    fn zero_coding_round_trips_the_packet_body() {
        let packet = [
            0x40, 0, 0, 0, 1, 0, // fixed header
            0xff, 0xff, 0, 31, // low-frequency message ID
            7, 0, 0, 0, 0, 0, 0, 0, 9,
        ];

        let encoded = zero_encode(&packet);
        assert!(encoded.len() < packet.len());
        assert_ne!(encoded[0] & 0x80, 0);

        let mut encoded_body = Bytes::copy_from_slice(&encoded[LLUDP_PACKET_HEADER_SIZE..]);
        let decoded_body = expand_zerocoded(&mut encoded_body).unwrap();
        assert_eq!(&decoded_body[..], &packet[LLUDP_PACKET_HEADER_SIZE..]);
    }
}
