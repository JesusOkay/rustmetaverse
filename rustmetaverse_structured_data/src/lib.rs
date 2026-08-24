//! LLSD (Linden Lab Structured Data) serialization for rustmetaverse.
//!
//! The [`OSD`] enum models the LLSD type system: boolean, integer, real,
//! string, UUID, date, URI, binary, map, array, and undef. Values can be
//! serialized to and from three formats:
//!
//! - **XML** via [`OSD::to_xml`] and [`xml_parser::parse_xml`]
//! - **Binary** via [`OSD::to_binary`] and [`OSD::from_binary`]
//! - **Notation** via [`OSD::to_notation`] and [`OSD::from_notation`]
//!
//! LLSD is the data format used by Second Life / OpenSimulator capability
//! (CAPS) endpoints and some login-response fields.

pub mod binary_parser;
pub mod notation_parser;
pub mod osd;
pub mod xml_parser;

pub use binary_parser::{format_binary, parse_binary, write_binary};
pub use notation_parser::{format_notation, parse_notation, write_notation};
pub use osd::OSD;
