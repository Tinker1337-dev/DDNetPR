# DDNetPR — Server-First Plugin Platform for DDNet++ (v11.7.0 LTS)

![Core](https://shields.io)
![Rust](https://shields.io)
![Release](https://shields.io)
![License](https://shields.io)

**DDNetPR** turns a **DDNet++ (C++20)** game server into a modular **Rust** platform. Write custom game modes, mini-games, economy systems, and real-time `OnSnap` visuals in **Rust (Edition 2024)** with **zero-downtime hot-reload**, process isolation, and **zero client modifications** — players join from any standard **DDNet 0.6 / 20.x** client.

---

## Why DDNetPR?

Normally, adding a new game mode or mechanic to Teeworlds / DDNet means editing C++ internals, recompiling the entire server, and kicking everyone online on every tweak.

DDNetPR separates the heavy 50 Hz physics & netcode from your gameplay logic:

```text
┌─────────────────────────┐         FIFO / Log IPC         ┌─────────────────────────┐
│   DDNet-Server (C++20)  │ ◄────────────────────────────► │  ddnet_plugins (Rust)   │
│  - 50Hz Physics & Snap  │   server.fifo / server.log     │  - Async Event Router   │
│  - Virtual Entity Pools │                                │  - Visual Pool Isolator │
│  - Custom Combat Rules  │                                │  - Rate-Limiter & WD    │
└─────────────────────────┘                                └───────────┬─────────────┘
                                                                       │ stdin/stdout IPC
                          ┌────────────────────────────────────────────┼────────────────────────────────┐
                          ▼                                            ▼                                ▼
            ┌──────────────────────────┐                 ┌──────────────────────────┐     ┌──────────────────────────┐
            │  plugin_gamemode (Rust)  │                 │   plugin_city (Rust)     │     │   plugin_hub (Rust)      │
            │  Race / Block / FNG / TDM│                 │  Economy, Jobs & Roles   │     │  Lobby, Stats & Coins    │
            └──────────────────────────┘                 └──────────────────────────┘     └──────────────────────────┘
```

- **Hot-Reload Without Kicking Players:** Update or restart any single plugin in ~1 second while players stay in-game.
- **Crash Isolation:** If a plugin panics, the supervisor restarts it automatically — the C++ server never crashes.
- **No `OnSnap` Collisions:** Each plugin gets isolated pools for virtual pickups, lasers, tees, and floating world texts.
- **Vanilla Client Compatible:** Custom scoreboards (time vs. integer points vs. Red/Blue teams), HUD lines, laser freeze, and floating cosmetics work out of the box on stock DDNet clients.

---

## What's Included Out of the Box

- **Dynamic Game Modes & Map Rotation (`plugin_gamemode`):** Switch live between **Race**, **Block**, **FNG**, and **TDM** via `!mode <race|block|fng|tdm> [nomap]`. Automatically rotates maps, switches Tab scoreboard modes, manages Red/Blue teams (`!team`), and configures freeze/weapon rules.
- **City RP & Lobby Economy (`plugin_city`, `plugin_hub`):** Persistent SQLite key-value storage (`plugins_kv.db`), jobs, salaries, coins, and player stats.
- **Mini-Games & Spells (`plugin_potato`, `plugin_sunstrike`):** Hot Potato with countdown indicators and custom raycast AoE abilities.
- **Staff & Builder Suite (`plugin_mod`, `plugin_super`, `plugin_brush`, `plugin_build`, `plugin_build_tee`, `plugin_katana`):** Role-based moderation, floating staff katana cosmetics, and vanilla-compatible map building tools.
- **Operations (`plugin_perf`, `plugin_reboot`):** Real-time tick/IPC profiler and scheduled restarts with chat alerts.

---

## Repository Structure

```text
DDNetPR/
├── core/
│   └── ddnetpr_core_v11.7_lts.patch   # C++20 patch for DDNetPP (commit 4e458fa81)
├── crates/
│   ├── ddnet_sdk/                     # Ergonomic Rust SDK v11.0.0 (macros, spatial, HUD, IPC)
│   ├── ddnet_plugins/                 # Async IPC router, visual pool isolator & supervisor
│   └── ddnet-plugin-template/         # Starter template & generator script for new plugins
├── plugins/                           # 14 ready-to-use Rust plugins
│   ├── plugin_gamemode/               # Live mode switcher (Race / Block / FNG / TDM) + map rotation
│   ├── plugin_hub/                    # Central lobby, player profiles, coins & leaderboards
│   ├── plugin_city/                   # City RP mode (jobs, salaries, roles, custom commands)
│   ├── plugin_potato/                 # Hot Potato mini-game with timers & visual indicators
│   ├── plugin_katana/                 # Floating staff katana cosmetic using OnSnap follow pickups
│   ├── plugin_mod/                    # Staff moderation suite (ban, mute, freeze, vanish, roles)
│   ├── plugin_super/                  # Extended admin/super powers & sandbox tools
│   ├── plugin_brush/                  # In-game map/entity brush tool for builders
│   ├── plugin_build/                  # Extended builder protocol integration
│   ├── plugin_build_tee/              # Vanilla-compatible builder tool using virtualized tees
│   ├── plugin_sunstrike/              # Custom AoE spell & raycast combat mechanics
│   ├── plugin_perf/                   # Real-time tickrate, IPC latency & resource profiler
│   ├── plugin_reboot/                 # Graceful scheduled restarts with countdown alerts
│   └── plugin_v10_test/               # Backward-compatibility verification suite (API v10 -> v11)
├── docs/                              # Installation guide, SDK reference & AI plugin prompt
├── scripts/
│   └── setup.sh                       # Interactive server configuration wizard
└── .github/workflows/                 # CI build checks & automated Linux x86_64 release packaging
```

---

## Quick Start

### 1. Prebuilt Linux x86_64 Bundle
Download `ddnetpr-v11.7.0-lts-linux-x86_64.tar.gz` from [GitHub Releases](../../releases), extract it, and run the setup wizard:

```bash
tar -xzf ddnetpr-v11.7.0-lts-linux-x86_64.tar.gz
cd ddnetpr-v11.7.0-lts-linux-x86_64
./setup.sh
./start.sh
```

### 2. Build from Source

#### Build the Rust SDK, Manager & Plugins
```bash
cargo build --release --workspace
```

#### Patch & Build the C++20 Server Core
```bash
git clone https://github.com
cd DDNetPP
git checkout 4e458fa81
git submodule update --init --recursive
git apply ../core/ddnetpr_core_v11.7_lts.patch
cmake -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=Release -DCLIENT=OFF -DSERVER=ON
cmake --build build -j$(nproc)
```

---

## Writing a Plugin with `ddnet_sdk`

Every plugin is a standalone Rust binary using the `ddnet_sdk::plugin!` macro:

```rust
use ddnet_sdk::prelude::*;

ddnet_sdk::plugin! {
    name: "hello_ddnet",
    desc: "Minimal DDNetPR plugin example",
    admin: false,
    priority: Priority::Normal,
    cmds: [
        ("hello", "", "Greet the server"),
    ],
    state: {
        greetings: u64 = 0,
    },
    handle(api, ev) {
        match ev {
            Event::Chat { player, message, .. } if message.trim() == "!hello" => {
                greetings += 1;
                api.whisper(&player, &format!("Hello! Total greetings: {}", greetings));
            }
            Event::TeamChange { player, team, .. } => {
                api.say(&format!("{} switched to team {}", player, team));
            }
            _ => {}
        }
    }
}
```

---

## Documentation

- [`docs/00_INSTALL_FROM_SCRATCH.md`](docs/00_INSTALL_FROM_SCRATCH.md) — Step-by-step VPS deployment guide
- [`docs/01_SDK_REFERENCE.md`](docs/01_SDK_REFERENCE.md) — Full `Event` & `ServerApi` reference
- [`docs/02_ADMIN_AND_PLUGINS.md`](docs/02_ADMIN_AND_PLUGINS.md) — Managing plugins, configs, and roles
- [`docs/PluginWriterPrompt.md`](docs/PluginWriterPrompt.md) — Ready-to-use prompt for generating new plugins with AI

---

## License

Licensed under the [MIT License](LICENSE). Built on top of [DDNet++](https://github.com/DDNetPP/DDNetPP) and [DDraceNetwork](https://ddnet.org).
