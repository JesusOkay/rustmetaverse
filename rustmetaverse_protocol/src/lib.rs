//! LLUDP wire format for rustmetaverse.
//!
//! This crate implements the low-level packet layer of the Linden Lab UDP
//! protocol used by Second Life and OpenSimulator:
//!
//! - [`header`]: the LLUDP packet header (flags, frequency, sequence,
//!   acknowledgement list) with serialize/deserialize.
//! - [`zerocoding`]: LLUDP zero-coding compression (encode and expand).
//! - [`safebuf`]: [`SafeBuf`], a bounds-checked reader that returns
//!   `io::Error` instead of panicking on truncated packets.
//! - [`packets`]: auto-generated packet definitions, the [`PacketType`]
//!   enum, the [`WrappedPacket`] dispatch enum, and [`decode_packet`].
//!
//! Packet definitions are generated from the LLUDP message template. The
//! [`Packet`] trait provides serialize/deserialize for every message; the
//! application-level handling lives in the `rustmetaverse` crate.

pub mod header;
pub mod packets;
pub mod safebuf;
pub mod zerocoding;

pub use header::*;
pub use packets::*;
pub use safebuf::SafeBuf;
pub use zerocoding::*;
