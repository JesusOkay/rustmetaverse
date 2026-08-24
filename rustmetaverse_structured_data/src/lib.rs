//! LLSD (Linden Lab Structured Data) serialization for rustmetaverse.
//!
//! The [`OSD`] enum models the LLSD type system: boolean, integer, real,
//! string, UUID, date, URI, binary, map, array, and undef. Values can be
//! serialized to XML via [`OSD::to_xml`] and parsed back via
//! [`xml_parser::parse_xml`].
//!
//! LLSD is the data format used by Second Life / OpenSimulator capability
//! (CAPS) endpoints and some login-response fields. This crate currently
//! implements the XML serialization; binary and notation formats are not
//! yet supported.

pub mod osd;
pub mod xml_parser;

pub use osd::OSD;
