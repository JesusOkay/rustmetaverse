# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-08-24

Initial public release.

### Added

- **rustmetaverse_types** — foundational types: `UUID` (v4 generation, parse,
  format), `Vector3` (add/sub/mul/div, dot, cross, length, distance),
  `Quaternion` (rotate, normalize, slerp), and math helpers (`PI`, `TWO_PI`,
  `HALF_PI`, `clamp`).
- **rustmetaverse_structured_data** — LLSD serialization in three formats:
  XML (`to_xml` / `parse_xml`), binary (`to_binary` / `from_binary`),
  and notation (`to_notation` / `from_notation`). Full `OSD` enum with
  Boolean, Integer, Real, String, UUID, Date, Array, Map (IndexMap), and
  Binary variants.
- **rustmetaverse_protocol** — LLUDP wire format: packet header (flags,
  frequency, ack sequence), zero-coding (encode + expand, round-trip
  tested), `SafeBuf` bounds-checked reader, and ~470 auto-generated packet
  definitions with `PacketType` enum and `decode_packet`.
- **rustmetaverse** — client orchestration:
  - XML-RPC login (`login`, `login_silent`, `LoginParams`)
  - UDP circuit establishment (`UseCircuitCode`, `CompleteAgentMovement`,
    `RegionHandshake`)
  - Reliable packet resend — selective-repeat ARQ with SRTT/RTTVAR-based
    adaptive RTO (250 ms–3 s), automatic resend with `MSG_RESENT` flag,
    max 5 retries
  - Async packet dispatcher with per-type handler registration
  - Avatar movement — `AgentUpdate` with control flags, continuous ~10 Hz
    movement loop via `start_movement_loop` / `stop_movement_loop`
  - Local chat — `say`, `shout`, `whisper` on any channel;
    `ChatFromSimulator` parsing
  - Instant messaging — `send_im` with full dialog type constants
  - Avatar appearance — `rebake`, `AvatarAppearance` parsing
  - Inventory — `fetch_inventory_folder` with sort/fetch flags
  - Object manipulation — create (`ObjectAdd`), delete, link/delink,
    name/description
  - Group operations — join, leave, profile parsing
  - Core handlers — `AgentMovementComplete`, `ChatFromSimulator`,
    `HealthMessage`, `LogoutReply`, `DisableSimulator`, `UUIDNameReply`
- **Examples** — `connection_test` (login → circuit → handshake → logout),
  `bot_scout` (nearby-avatar detection), `full_test` (all 24 public API
  functions exercised live)
- CI workflow — fmt check, clippy (`-D warnings`), build, test, doc
- Dual license: MIT OR Apache-2.0
