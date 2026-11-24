# DDNetPR — Server-First Plugin Platform for DDNet++ (v11.7.0 LTS)

**DDNetPR** is a high-performance, server-first plugin platform and Rust SDK built on top of the **DDNet++ (C++20)** game server. It enables developers to write custom game modes, mini-games, economy systems, and real-time `OnSnap` visual effects in **Rust (Edition 2024)** with **hot-reload**, process isolation, and **zero client modifications** (works out of the box with standard DDNet 0.6 / 20.x clients).

---

## Architecture Overview

Instead of embedding scripting VMs inside the single-threaded C++ tick loop or recompiling the server binary for every gameplay change, DDNetPR decouples gameplay logic into isolated OS processes managed by an asynchronous Rust IPC router:

```text
┌─────────────────────────┐         FIFO / Log IPC         ┌─────────────────────────┐
│   DDNet-Server (C++20)  │ ◄────────────────────────────► │  ddnet_plugins (Rust)   │
│  - Physics & Netcode    │   server.fifo / server.log     │  - Async Event Router   │
│  - OnSnap Entity Pools  │                                │  - Visual Pool Isolator │
│  - Custom Tile/Ray Hooks│                                │  - Rate-Limiter & WD    │
└─────────────────────────┘                                └───────────┬─────────────┘
                                                                       │ stdin/stdout IPC
                          ┌────────────────────────────────────────────┼────────────────────────────────┐
                          ▼                                            ▼                                ▼
            ┌──────────────────────────┐                 ┌──────────────────────────┐     ┌──────────────────────────┐
            │   plugin_city (Rust)     │                 │  plugin_potato (Rust)    │     │   plugin_hub (Rust)      │
            │  Economy, Jobs & Roles   │                 │  Hot Potato Mini-Game    │     │  Lobby, Stats & Coins    │
            └──────────────────────────┘                 └──────────────────────────┘     └──────────────────────────┘
```

### Key Engineering Features
- **Zero-Modification Client Support:** Renders custom HUD lines, floating world text, laser polygons, pickups, and dummy tees on vanilla DDNet clients via server-side `OnSnap` entity virtualization.
- **Strict Visual Pool Isolation:** `ddnet_plugins` automatically partitions entity IDs across plugins (`pickups 0..128`, `lasers 0..128`, `tees 0..64`, `texts 0..32`), preventing ID collisions when multiple plugins render simultaneously.
- **Hot-Reload & Crash Isolation:** Each plugin runs as an independent Rust binary compiled against `ddnet_sdk`. Updating or restarting a plugin takes milliseconds without dropping connected players.
- **Built-in Guardrails:** Per-plugin FX rate-limiting (`110 FX/sec`), command throttling (`240 cmd/sec`), and a `12s` process watchdog with automatic recovery.
- **Shared Key-Value Persistence:** Built-in asynchronous SQLite KV store (`plugins_kv.db`) with atomic balance transfers (`db_add_by_account`, `db_transfer_by_account`) shared safely across plugins.

---

## Repository Structure

```text
DDNetPR/
├── core/
│   └── ddnetpr_core_v11.7_lts.patch   # C++20 patch for DDNetPP (commit 4e458fa81)
├── crates/
│   ├── ddnet_sdk/                     # Ergonomic Rust SDK v11.0.0 (macros, spatial queries, HUD, IPC)
│   ├── ddnet_plugins/                 # Core IPC router, visual pool isolator & process supervisor
│   └── ddnet-plugin-template/         # Starter template & generator script for new plugins
├── plugins/                           # 12 production-ready plugins built with ddnet_sdk
│   ├── plugin_hub/                    # Central lobby, player profiles, coin economy & leaderboards
│   ├── plugin_city/                   # City RP mode (jobs, salaries, roles, custom commands)
│   ├── plugin_potato/                 # Hot Potato mini-game with real-time timers & visual indicators
│   ├── plugin_mod/                    # Staff moderation suite (ban, mute, freeze, vanish, role sync)
│   ├── plugin_super/                  # Extended admin/super powers & sandbox tools
│   ├── plugin_brush/                  # In-game map/entity brush tool for builders
│   ├── plugin_build/                  # Extended builder protocol integration
│   ├── plugin_build_tee/              # Vanilla-compatible builder tool using virtualized tees
│   ├── plugin_sunstrike/              # Custom AoE spell & raycast combat mechanics
│   ├── plugin_perf/                   # Real-time tickrate, IPC latency & resource profiler
│   ├── plugin_reboot/                 # Graceful scheduled restarts with countdown alerts
│   └── plugin_v10_test/               # Backward-compatibility verification suite (API v10 -> v11)
├── docs/                              # Architecture guides, SDK reference & AI plugin writer prompt
├── scripts/
│   └── setup.sh                       # Interactive server configuration wizard
└── .github/workflows/                 # CI build checks & automated Linux x86_64 release packaging
```

---

## Quick Start

### 1. Using Prebuilt Release (Linux x86_64)
Download the latest `ddnetpr-v11.7.0-lts-linux-x86_64.tar.gz` from [GitHub Releases](../../releases), extract it, and run the interactive setup wizard:

```bash
tar -xzf ddnetpr-v11.7.0-lts-linux-x86_64.tar.gz
cd ddnetpr-v11.7.0-lts-linux-x86_64
./setup.sh
./start.sh
```

### 2. Building from Source

#### Build the Rust SDK, Manager & Plugins
```bash
cargo build --release --workspace
```

#### Patch & Build the C++20 Server Core
```bash
git clone https://github.com
cd DDNetPP
git checkout 4e458fa81
git apply ../core/ddnetpr_core_v11.7_lts.patch
cmake -S . -B build -DCLIENT=OFF -DMYSQL=OFF
cmake --build build -j\$(nproc)
```

---

## Writing a Plugin with `ddnet_sdk`

```rust
use ddnet_sdk::{plugin, Event, ServerApi};

plugin! {
    name: "hello_ddnet",
    desc: "Minimal DDNetPR plugin example",
    admin: false,
    priority: 10,
    cmds: [
        ("!hello", "", "Greet the server")
    ],
    state: {
        greetings: u64 = 0,
    },
    handle(api, ev) {
        if let Event::Chat { player, message, .. } = ev {
            if message.trim() == "!hello" {
                state.greetings += 1;
                api.whisper(player, &format!("Hello! Total greetings: {}", state.greetings));
            }
        }
    }
}
```

See [`docs/01_SDK_REFERENCE.md`](docs/01_SDK_REFERENCE.md) for the complete API documentation.

---

## License

Licensed under the [MIT License](LICENSE). Based on [DDNet++](https://github.com) and [DDraceNetwork](https://ddnet.org).
