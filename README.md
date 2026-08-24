```
▗▄▄▖ ▗▖ ▗▖ ▗▄▄▖▗▄▄▄▖▗▖  ▗▖▗▄▄▄▖▗▄▄▄▖▗▄▖ ▗▖  ▗▖▗▄▄▄▖▗▄▄▖  ▗▄▄▖▗▄▄▄▖
▐▌ ▐▌▐▌ ▐▌▐▌     █  ▐▛▚▞▜▌▐▌     █ ▐▌ ▐▌▐▌  ▐▌▐▌   ▐▌ ▐▌▐▌   ▐▌
▐▛▀▚▖▐▌ ▐▌ ▝▀▚▖  █  ▐▌  ▐▌▐▛▀▀▘  █ ▐▛▀▜▌▐▌  ▐▌▐▛▀▀▘▐▛▀▚▖ ▝▀▚▖▐▛▀▀▘
▐▌ ▐▌▝▚▄▞▘▗▄▄▞▘  █  ▐▌  ▐▌▐▙▄▄▖  █ ▐▌ ▐▌ ▝▚▞▘ ▐▙▄▄▖▐▌ ▐▌▗▄▄▞▘▐▙▄▄▖
```

# rustmetaverse

A modern Rust client library for building Second Life / OpenSimulator
virtual-world clients — written from scratch in safe, async Rust to modernize
the stack, eliminate runtime overhead, and bring memory safety and fearless
concurrency to the metaverse protocol layer.

![CI failing](https://img.shields.io/badge/CI-failing-red)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
![Rust](https://img.shields.io/badge/rust-stable-orange.svg)

---

## What it is

rustmetaverse is a native Rust implementation of the Second Life / OpenSimulator
virtual-world protocol (LLUDP). It implements the login flow, UDP circuit
handshake, LLUDP packet framing, zero-coding, and an async packet dispatcher —
everything needed to build automated clients, bots, and tools that connect to
SL or OpenSim grids.

## What it is not

This is **not** a port or fork of any existing library. It is a
clean-room implementation in Rust with independent source code. The
Second Life / OpenSimulator protocol is a public wire format; this project
implements it directly.

## Why Rust?

The Second Life / OpenSimulator protocol has been implemented in managed
languages for over two decades. rustmetaverse rewrites the same protocol layer
in Rust to gain:

- **Memory safety** — no GC pauses, no null-pointer dereferences, no buffer
  overflows. The protocol parser uses bounds-checked reads throughout.
- **Async I/O** — tokio-based networking with zero-copy packet handling and
  lock-free sequence numbering.
- **Single binary** — no runtime dependency, no VM, no SDK version drift.
  Cross-compile to any target Rust supports.
- **Modern ergonomics** — strong typing over the wire format, `Result`-based
  error handling, derive macros for serialization.

## Architecture

```
┌──────────────────────────────────────────────────────────────────────────┐
│                      rustmetaverse (client)                              │
│                                                                          │
│  GridClient           PacketDispatcher       NetworkManager              │
│  + login flow         + async handlers       + tokio UdpSocket           │
│  + circuit code       + per-type registry    + actor pattern (mpsc)      │
│  + handshake          + BoxFuture dispatch   + sequence tracking         │
│  + session state      + route by PacketType + reliable ack queue         │
│  + logout + chat + IM                                                      │
│                                                                          │
│  Subsystems:                                                             │
│  chat: ChatFromViewer/Simulator  messaging: ImprovedInstantMessage       │
│  movement: AgentUpdate           appearance: RebakeAvatarTextures       │
│  inventory: Fetch/Descendents    objects: Add/Delete/Link/Name          │
│  groups: Join/Leave/Profile       core_handlers: Health/Movement/Logout  │
│                                                                          │
│  reliability layer: NeedAcks + SRTT/RTTVAR RTO + resend loop (250ms)     │
│                                                                          │
│  Simulator: region | session_id | circuit_code | seed_capability         │
├──────────────────────────────────────────────────────────────────────────┤
│                    rustmetaverse_protocol (wire)                         │
│                                                                          │
│  Header               ZeroCoding             SafeBuf                     │
│  + flags (app/ack)    + encode (compress)    + bounds-checked reads      │
│  + frequency (low/    + expand (decompress)  + no panic on EOF           │
│    med/high)          + round-trip tested    + returns io::Error         │
│  + ack sequence no.                                                      │
│                                                                          │
│  Packets (~470 defs, auto-generated)                                     │
│  + serialize/deserialize + typed blocks + PacketType enum                │
├──────────────────────────────────────────────────────────────────────────┤
│                 rustmetaverse_structured_data (LLSD)                     │
│                                                                          │
│  OSD enum                XML parser          Binary parser    Notation     │
│  + Boolean + Integer     + LLSD <-> XML      + LLSD binary    + LLSD notation│
│  + Real + String         + XML-RPC response  + round-trip     + round-trip │
│  + UUID + Date + Array   + base64 decode     + big-endian     + text fmt   │
│  + Map(IndexMap) + Binary                                                  │
├──────────────────────────────────────────────────────────────────────────┤
│                    rustmetaverse_types (math)                            │
│                                                                          │
│  UUID               Vector3            Quaternion         utils          │
│  + v4 generation    + add/sub/mul/div  + rotate            + PI / TWO_PI │
│  + parse + format   + dot / cross      + normalize         + HALF_PI     │
│  + ZERO constant    + length + dist    + slerp            + clamp()     │
└──────────────────────────────────────────────────────────────────────────┘
```

| Crate | Responsibility | Status |
|-------|---------------|--------|
| `rustmetaverse_types` | Foundational types: `UUID`, `Vector3`, `Quaternion`, math helpers | ✅ Stable |
| `rustmetaverse_structured_data` | LLSD (Linden Lab Structured Data) XML, binary, and notation serialization | ✅ Working |
| `rustmetaverse_protocol` | LLUDP wire format: packet header, zero-coding, safe buffer reads, packet definitions | ✅ Working |
| `rustmetaverse` | Client orchestration: login, networking, reliable resend, movement, chat, IM, inventory, objects, groups, appearance, packet dispatch, session state | ✅ Working |

## Current status

### ✅ Working today
- XML-RPC login to Second Life / OpenSimulator grids
- UDP circuit establishment (`UseCircuitCode` with retry + acknowledgement)
- `CompleteAgentMovement` and `RegionHandshake` exchange
- Ping/pong keepalive handling
- Reliable packet acknowledgement tracking
- **Reliable packet resend** — selective-repeat ARQ with SRTT/RTTVAR-based
  adaptive RTO (clamped 250 ms–3 s), automatic resend with `MSG_RESENT` flag,
  max 5 retries
- Async packet dispatcher with per-type handler registration
- LLUDP zero-coding (encode + expand, round-trip tested)
- Bounds-checked packet parsing — no panics on malformed data
- ~470 packet definitions generated from the LLUDP message template
- **LLSD binary serialization** — `OSD::to_binary()` / `OSD::from_binary()`,
  round-trip tested
- **LLSD notation serialization** — `OSD::to_notation()` /
  `OSD::from_notation()`, round-trip tested
- **Avatar movement** — `AgentUpdate` with control flags (forward, backward,
  strafe, turn, up/down, fly), camera vectors, body rotation
- **Local chat** — `ChatFromViewer` (say, shout, whisper) on any channel;
  `ChatFromSimulator` parsing for incoming messages
- **Instant messaging** — `ImprovedInstantMessage` for private IMs, teleport
  lures, friendship offers; full dialog type constants
- **Avatar appearance** — `RebakeAvatarTextures` request; `AvatarAppearance`
  packet parsing (sender, texture entry, visual params, attachments)
- **Inventory operations** — `FetchInventoryDescendents` request with
  sort/fetch flags; `InventoryDescendents` parsing into typed folders and
  items; item/asset type constants
- **Object manipulation** — create prims (`ObjectAdd` with P-code, material,
  raycast), delete (`ObjectDelete` with force flag), link/delink
  (`ObjectLink`/`ObjectDelink`), set name/description (`ObjectName`/
  `ObjectDescription`)
- **Group operations** — join (`JoinGroupRequest`), leave
  (`LeaveGroupRequest`), parse replies (`JoinGroupReply`,
  `LeaveGroupReply`), profile parsing (`GroupProfileReply`)
- **Core packet handlers** — `AgentMovementComplete` (avatar position),
  `ChatFromSimulator` (local chat), `HealthMessage` (avatar health),
  `LogoutReply` (logout confirmation), `DisableSimulator` (region shutdown),
  `UUIDNameReply` (display name cache)
- **GridClient API** — `logout()`, `say()`, `shout()`, `send_im()`,
  `rebake()`, `fetch_inventory_folder()`, `join_group()`, `leave_group()`

### ⚠️ Partially implemented
- Reliable resend covers all reliable packets; `UseCircuitCode` still has its
  own dedicated retry loop for backward compatibility
- Movement sends `AgentUpdate` packets; continuous movement loop not yet
  wired into `GridClient` (caller must drive `send_agent_update` ~10×/s)
- `AgentMovementComplete` is captured into `core_state.avatar_position` but
  not yet used to drive automatic movement

### ❌ Missing / not yet implemented
- No published crates.io release yet

## Quick start

### Build

```sh
cargo build --workspace
```

### Run the connection test

The `connection_test` example performs a full login → circuit → handshake →
logout cycle against a live grid:

```sh
cargo run --release --example connection_test
```

It will prompt for first name, last name, password, region, and login URI.
Defaults to the Second Life Agni grid. For a local OpenSim instance, use
`http://localhost:8002` as the login URI.

### Bot scout

The `bot_scout` example connects to a region, waits for the nearby-avatar
snapshot, resolves avatar names, and reports whether a target resident is
present:

```sh
cargo run --release --example bot_scout -- \
    BotFirst BotLast BotPassword "Region Name" TargetFirst TargetLast
```

### Minimal usage

```rust
use rustmetaverse::{GridClient, LoginParams};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = GridClient::new().await;

    let params = LoginParams {
        first_name: "BotFirst".to_string(),
        last_name: "BotLast".to_string(),
        password: "password".to_string(),
        start: "last".to_string(),
        ..Default::default()
    };

    client
        .login(&params, "https://login.agni.lindenlab.com/cgi-bin/login.cgi")
        .await?;
    client.start_network_loop().await;

    // Wait for region handshake, then interact with the grid...
    Ok(())
}
```

## Crate structure

```
rustmetaverse/
├── rustmetaverse_types/            # UUID, Vector3, Quaternion, math
├── rustmetaverse_structured_data/  # LLSD XML serialization
├── rustmetaverse_protocol/         # LLUDP framing, packets, zero-coding
└── rustmetaverse/                   # GridClient, login, networking, dispatch
```

## Acknowledgements

The Second Life / OpenSimulator protocol is a public wire format documented
by the community over many years. The [Firestorm](https://www.firestormviewer.org/)
viewer's behavior was used to verify protocol details. This project is an
independent implementation that shares no source code with any other project.

## License

Dual-licensed under MIT OR Apache-2.0.

- [MIT License](LICENSE-MIT)
- [Apache License 2.0](LICENSE-APACHE)
