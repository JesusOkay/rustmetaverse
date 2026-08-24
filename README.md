```
                    __                 __
   _______  _______/ /_____ ___  ___  / /_____ __   _____  _____________
  / ___/ / / / ___/ __/ __ `__ \/ _ \/ __/ __ `/ | / / _ \/ ___/ ___/ _ \
 / /  / /_/ (__  ) /_/ / / / / /  __/ /_/ /_/ /| |/ /  __/ /  (__  )  __/
/_/   \__,_/____/\__/_/ /_/ /_/\___/\__/\__,_/ |___/\___/_/  /____/\___/
```

# rustmetaverse

A modern Rust client library for building Second Life / OpenSimulator
virtual-world clients — written from scratch in safe, async Rust to modernize
the stack, eliminate runtime overhead, and bring memory safety and fearless
concurrency to the metaverse protocol layer.

[![Build status](https://github.com/JesusOkay/rustmetaverse/actions/workflows/ci.yml/badge.svg)](https://github.com/JesusOkay/rustmetaverse/actions/workflows/ci.yml)
[![License: BSD-3-Clause](https://img.shields.io/badge/license-BSD--3--Clause-blue.svg)](LICENSE)
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
┌─────────────────────────────────────────────────────────┐
│                     rustmetaverse                       │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────────┐  │
│  │ GridClient   │  │  PacketDisp. │  │  NetworkMgr    │  │
│  │ (login,      │──│  (async      │──│  (tokio UDP,   │  │
│  │  circuit,    │  │   handlers)  │  │   actor send)  │  │
│  │  handshake)  │  └──────────────┘  └────────────────┘  │
│  └─────────────┘                                        │
├─────────────────────────────────────────────────────────┤
│                  rustmetaverse_protocol                  │
│  ┌─────────┐  ┌────────────┐  ┌──────────┐  ┌─────────┐  │
│  │ Header  │  │ ZeroCoding │  │ SafeBuf  │  │ Packets │  │
│  │ (flags, │  │ (encode +  │  │ (bounds- │  │ (~470   │  │
│  │  freq)  │  │  expand)   │  │  checked)│  │  defs)  │  │
│  └─────────┘  └────────────┘  └──────────┘  └─────────┘  │
├─────────────────────────────────────────────────────────┤
│              rustmetaverse_structured_data                │
│  ┌────────────────┐  ┌────────────────────────────────┐  │
│  │  OSD enum       │  │  XML parser (LLSD <-> XML)    │  │
│  │  (Bool, Int,    │  │  + XML-RPC response parser    │  │
│  │   Real, Str,    │  │                               │  │
│  │   UUID, ...)    │  │                               │  │
│  └────────────────┘  └────────────────────────────────┘  │
├─────────────────────────────────────────────────────────┤
│                   rustmetaverse_types                    │
│  ┌────────┐  ┌──────────┐  ┌────────────┐  ┌───────────┐  │
│  │  UUID   │  │ Vector3  │  │ Quaternion │  │   utils   │  │
│  │ (v4,   │  │ (math:   │  │ (rotate,   │  │ (PI, clamp│  │
│  │  parse)│  │  dot,crs)│  │  normalize)│  │  consts)  │  │
│  └────────┘  └──────────┘  └────────────┘  └───────────┘  │
└─────────────────────────────────────────────────────────┘
```

| Crate | Responsibility | Status |
|-------|---------------|--------|
| `rustmetaverse_types` | Foundational types: `UUID`, `Vector3`, `Quaternion`, math helpers | ✅ Stable |
| `rustmetaverse_structured_data` | LLSD (Linden Lab Structured Data) XML serialization and parsing | ✅ Working |
| `rustmetaverse_protocol` | LLUDP wire format: packet header, zero-coding, safe buffer reads, packet definitions | ✅ Working |
| `rustmetaverse` | Client orchestration: login, networking, packet dispatch, session state | ⚠️ Early |

## Current status

### ✅ Working today
- XML-RPC login to Second Life / OpenSimulator grids
- UDP circuit establishment (`UseCircuitCode` with retry + acknowledgement)
- `CompleteAgentMovement` and `RegionHandshake` exchange
- Ping/pong keepalive handling
- Reliable packet acknowledgement tracking
- Async packet dispatcher with per-type handler registration
- LLUDP zero-coding (encode + expand, round-trip tested)
- Bounds-checked packet parsing — no panics on malformed data
- ~470 packet definitions generated from the LLUDP message template

### ❌ Missing / not yet implemented
- Most packet handlers beyond the login/handshake sequence
- Avatar movement and appearance
- Inventory operations
- Object manipulation (create, delete, modify, link)
- Group and messaging features
- Binary and notation LLSD formats (XML only)
- Resent/retry logic for reliable packets (only `UseCircuitCode` retries today)
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

[BSD 3-Clause](LICENSE)
