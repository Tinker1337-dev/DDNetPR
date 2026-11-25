#![warn(missing_docs)]
//! DDNet++ Ultimate Plugin Platform SDK (v10.0 LTS).

/// Typed DX primitives, errors, and prelude re-exports.
pub mod config;
pub use config::PluginConfig;
/// Trait-based event dispatcher.
pub mod handler;
pub use handler::EventHandler;
/// Spatial query helpers built on top of cached player states.
pub mod spatial;
/// Asynchronous file logger (`logs/<plugin>.log`).
pub mod log;
/// Typed DX primitives, errors, and prelude re-exports.
pub mod dx;

pub use dx::prelude;

pub use dx::{PlayerId, RayHit, RayMask, SdkError, Vec2, Weapon};

use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc;
use std::thread;

/// Returns the SDK API version string (`"11.0.0"`).
///
/// # Examples
/// ```
/// assert_eq!(ddnet_sdk::plugin_api_version(), "11.0.0");
/// ```
pub fn plugin_api_version() -> &'static str {
    "11.0.0"
}

/// Plugin execution priority hint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Priority {
    /// High priority (runs first).
    High,
    /// Normal default priority.
    Normal,
    /// Low priority (runs last).
    Low,
}

/// Action returned by intercepting event handlers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventAction {
    /// Pass event further to core and other plugins.
    Pass,
    /// Legacy v9.2 alias for Pass.
    Continue,
    /// Cancel event propagation.
    Cancel,
}

/// Cached player position and velocity snapshot.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerPos {
    /// World X coordinate in pixels.
    pub x: f32,
    /// World Y coordinate in pixels.
    pub y: f32,
    /// Horizontal velocity.
    pub vx: f32,
    /// Vertical velocity.
    pub vy: f32,
    /// Whether the player character is currently alive.
    pub alive: bool,
    /// Remaining freeze ticks.
    pub frozen: i32,
    /// Active weapon ID.
    pub weapon: i32,
}

/// Server and manager events delivered to plugins.
#[derive(Clone, Debug)]
pub enum Event {
    /// Player joined the server.
    Join {
        /// Player nickname.
        player: String,
    },
    /// Player left the server.
    Leave {
        /// Player nickname.
        player: String,
    },
    /// Player renamed.
    Rename {
        /// Previous nickname.
        old: String,
        /// New nickname.
        new: String,
        /// Legacy alias for new nickname.
        new_name: String,
    },
    /// Chat message or command received.
    Chat {
        /// Chat team channel (`0` = public).
        team: i32,
        /// Sender nickname.
        player: String,
        /// Raw message text.
        message: String,
    },
    /// BestClient handshake received.
    BestClient {
        /// Client ID.
        cid: i32,
        /// Client version string.
        version: String,
    },
    /// Player authenticated via account system.
    Auth {
        /// Client ID.
        cid: i32,
        /// Player nickname.
        player: String,
        /// Account username.
        account: String,
    },
    /// Role updated for an account.
    Role {
        /// Account username.
        account: String,
        /// Assigned role name.
        role: String,
    },
    /// Periodic 1-second server tick.
    Tick {
        /// Uptime in seconds.
        uptime_sec: u64,
    },
    /// High-frequency 100ms tick.
    FastTick {
        /// Uptime in milliseconds.
        uptime_ms: u64,
    },
    /// Scheduled timer callback fired.
    Scheduled {
        /// Plugin-defined timer identifier.
        callback_id: u64,
    },
    /// Inter-plugin message received.
    Ipc {
        /// Sender plugin name.
        from: String,
        /// Payload string.
        payload: String,
    },
    /// Key-value database response.
    DbValue {
        /// Request correlation identifier.
        req_id: String,
        /// Returned string value.
        value: String,
    },
    /// Key-value prefix list response.
    DbList {
        /// Request correlation identifier.
        req_id: String,
        /// Parsed `(key, value)` pairs.
        items: Vec<(String, String)>,
    },
    /// Player movement/action input snapshot.
    Input {
        /// Client ID.
        cid: i32,
        /// Horizontal direction (`-1`, `0`, `1`).
        dir: i32,
        /// Jump flag.
        jump: i32,
        /// Hook flag.
        hook: i32,
        /// Fire counter/flag.
        fire: i32,
        /// Aim target X.
        target_x: i32,
        /// Aim target Y.
        target_y: i32,
    },
    /// Legacy online list update.
    Online {
        /// Online player nicknames.
        players: Vec<String>,
    },
    /// Legacy DB result event.
    DbResult {
        /// Request ID.
        req_id: String,
        /// Value string.
        value: String,
    },
    /// Legacy detailed player state snapshot.
    PlayerState {
        /// Client ID.
        cid: i32,
        /// Player nickname.
        player: String,
        /// World X.
        x: f32,
        /// World Y.
        y: f32,
        /// Velocity X.
        vx: f32,
        /// Velocity Y.
        vy: f32,
        /// Freeze ticks.
        frozen: i32,
        /// Active weapon.
        weapon: i32,
    },
    /// Player state/position update.
    State {
        /// Client ID.
        cid: i32,
        /// Player snapshot.
        pos: PlayerPos,
    },
    /// Player kill event.
    Kill {
        /// Killer nickname.
        killer: String,
        /// Victim nickname.
        victim: String,
        /// Weapon ID.
        weapon: i32,
    },
    /// Player death event.
    Death {
        /// Client ID.
        cid: i32,
        /// Killer client ID (`-1` if world).
        killer: i32,
        /// Weapon ID.
        weapon: i32,
    },
    /// Player took damage.
    Damage {
        /// Victim client ID.
        cid: i32,
        /// Attacker client ID.
        attacker: i32,
        /// Damage amount.
        amount: i32,
        /// Weapon ID.
        weapon: i32,
    },
    /// Player finished a map/race.
    Finish {
        /// Client ID.
        cid: i32,
        /// Finish time in seconds.
        time: f32,
    },
    /// Player touched a special tile.
    Tile {
        /// Client ID.
        cid: i32,
        /// Tile index.
        tile: i32,
        /// Tile X coordinate.
        x: i32,
        /// Tile Y coordinate.
        y: i32,
    },
    /// Weapon hit event.
    WeaponHit {
        /// Attacker client ID.
        attacker: i32,
        /// Victim client ID.
        victim: i32,
        /// Weapon ID.
        weapon: i32,
    },
    /// Hammer hit event.
    HammerHit {
        /// Attacker nickname.
        attacker: String,
        /// Victim nickname.
        victim: String,
    },
    /// Pickup collected event.
    Pickup {
        /// Client ID.
        cid: i32,
        /// Pickup type.
        kind: i32,
    },
    /// Active weapon changed.
    WeaponChange {
        /// Client ID.
        cid: i32,
        /// Previous weapon ID.
        old: i32,
        /// New weapon ID.
        new: i32,
    },
    /// Player hook attached to a tile or another player.
    HookAttach {
        /// Client ID of the hooking player.
        cid: i32,
        /// Hooked target client ID, or None if attached to world geometry.
        hooked_cid: Option<i32>,
        /// World coordinates where the hook attached.
        pos: crate::dx::Vec2,
    },
    /// Player hook detached.
    HookDetach {
        /// Client ID of the player.
        cid: i32,
        /// Previously hooked player ID, if any.
        prev_hooked_cid: Option<i32>,
    },
}

type LifecycleHook = Box<dyn FnMut(&mut ServerApi)>;

/// Main SDK handle passed to plugin event callbacks.
pub struct ServerApi {
    /// Currently online player nicknames.
    pub online: HashSet<String>,
    /// Client IDs verified as BestClient.
    pub best_clients: HashSet<i32>,
    /// Map from nickname to client ID (`i32`).
    pub cids: HashMap<String, i32>,
    /// Legacy v9.2 alias for `cids`.
    pub player_cids: HashMap<String, i32>,
    /// Map from client ID (`i32`) to nickname.
    pub names: HashMap<i32, String>,
    /// Map from nickname to client IP address.
    pub ips: HashMap<String, String>,
    /// Legacy v9.2 alias for `ips`.
    pub player_ips: HashMap<String, String>,
    /// Map from nickname to authenticated account name.
    pub accounts: HashMap<String, String>,
    /// Legacy v9.2 alias for `accounts`.
    pub authed_accounts: HashMap<String, String>,
    /// Map from account name to assigned role.
    pub roles: HashMap<String, String>,
    /// Cached player positions keyed by client ID.
    pub states: HashMap<i32, PlayerPos>,
    /// Client ID of the player who disconnected in the most recent `Event::Leave`.
    pub last_leave_cid: Option<i32>,
    permissions: HashSet<String>,
    on_load_cb: Option<LifecycleHook>,
    on_reload_cb: Option<LifecycleHook>,
    on_unload_cb: Option<LifecycleHook>,
}

impl Default for ServerApi {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerApi {
    /// Creates a new `ServerApi` instance and announces SDK version to manager.
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// assert!(api.online.is_empty());
    /// ```
    pub fn new() -> Self {
        let api = Self {
            online: HashSet::new(),
            best_clients: HashSet::new(),
            cids: HashMap::new(),
            player_cids: HashMap::new(),
            names: HashMap::new(),
            ips: HashMap::new(),
            player_ips: HashMap::new(),
            accounts: HashMap::new(),
            authed_accounts: HashMap::new(),
            roles: HashMap::new(),
            states: HashMap::new(),
            last_leave_cid: None,
            permissions: HashSet::new(),
            on_load_cb: None,
            on_reload_cb: None,
            on_unload_cb: None,
        };
        api.raw(&format!("@PM SDK_VER|{}", plugin_api_version()));
        api
    }

    /// Sends a raw command line to the plugin manager over stdout.
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.raw("@PONG");
    /// ```
    pub fn raw(&self, cmd: &str) {
        let mut out = io::stdout().lock();
        let _ = writeln!(out, "{}", cmd);
        let _ = out.flush();
    }

    /// Declares plugin API version and required capability permissions.
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let mut api = ServerApi::new();
    /// api.set_manifest("10.0", &["tile_edit"]);
    /// assert!(api.has_permission("tile_edit"));
    /// ```
    pub fn set_manifest(&mut self, api_ver: &str, perms: &[&str]) {
        self.permissions.clear();
        for p in perms {
            self.permissions.insert((*p).to_string());
        }
        self.raw(&format!("@PM MANIFEST|{}|{}", api_ver, perms.join(",")));
    }

    /// Returns `true` if the plugin declared the given permission in its manifest.
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let mut api = ServerApi::new();
    /// api.set_manifest("10.0", &["noclip"]);
    /// assert!(api.has_permission("noclip"));
    /// assert!(!api.has_permission("damage"));
    /// ```
    pub fn has_permission(&self, perm: &str) -> bool {
        self.permissions.contains(perm)
    }

    /// Loads or creates plugin .cfg file.
    pub fn load_config<C: PluginConfig>(&self, n: &str) -> C { C::load_or_create(n) }

    /// Registers a chat command and help description.
    pub fn register_plugin(&mut self, cmd: &str, desc: &str) {
        crate::log::init(cmd);
        self.raw(&format!("@REG_CMD {}|{}", cmd, desc));
    }

    /// Sets plugin execution priority (`High`, `Normal`, `Low`).
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.set_priority(Priority::High);
    /// ```
    pub fn set_priority(&self, prio: Priority) {
        let tag = match prio {
            Priority::High => "HIGH",
            Priority::Normal => "NORMAL",
            Priority::Low => "LOW",
        };
        self.raw(&format!("@PM PRIORITY|{}", tag));
    }

    /// Enables or disables 100ms `Event::FastTick` delivery.
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.enable_fast_tick(true);
    /// ```
    pub fn enable_fast_tick(&self, enabled: bool) {
        self.raw(&format!("@PM FASTTICK|{}", if enabled { 1 } else { 0 }));
    }

    /// Enables or disables periodic player position polling into `api.states`.
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.track_positions(true);
    /// ```
    pub fn track_positions(&self, enabled: bool) {
        self.raw(&format!("@TRACK_POS {}", if enabled { 1 } else { 0 }));
    }

    /// Registers a callback invoked on initial plugin load.
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let mut api = ServerApi::new();
    /// api.on_load(|_api| {});
    /// ```
    pub fn on_load<F: FnMut(&mut ServerApi) + 'static>(&mut self, cb: F) {
        self.on_load_cb = Some(Box::new(cb));
    }

    /// Registers a callback invoked when the plugin is hot-reloaded.
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let mut api = ServerApi::new();
    /// api.on_reload(|_api| {});
    /// ```
    pub fn on_reload<F: FnMut(&mut ServerApi) + 'static>(&mut self, cb: F) {
        self.on_reload_cb = Some(Box::new(cb));
    }

    /// Registers a callback invoked after state rollback on plugin unload.
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let mut api = ServerApi::new();
    /// api.on_unload(|_api| {});
    /// ```
    pub fn on_unload<F: FnMut(&mut ServerApi) + 'static>(&mut self, cb: F) {
        self.on_unload_cb = Some(Box::new(cb));
    }

    /// Resolves a player's nickname to a typed `PlayerId`.
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let mut api = ServerApi::new();
    /// api.cids.insert("nameless".into(), 3);
    /// assert_eq!(api.player_id("nameless"), Some(PlayerId(3)));
    /// ```
    pub fn player_id(&self, nick: &str) -> Option<PlayerId> {
        self.cids.get(nick).copied().map(PlayerId)
    }

    /// Broadcasts a public chat message to all players.
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.say("Hello server!");
    /// ```
    pub fn say(&self, msg: &str) {
        self.raw(&format!("say \"{}\"", msg.replace('"', "'")));
    }

    /// Sends a private whisper message to a player by nickname.
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.whisper("nameless", "Hi!");
    /// ```
    pub fn whisper(&self, player: &str, msg: &str) {
        self.raw(&format!(
            "whisper \"{}\" \"{}\"",
            player.replace('"', "'"),
            msg.replace('"', "'")
        ));
    }

    /// Sends a private whisper message to a typed `PlayerId`.
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.whisper_id(PlayerId(0), "Hi!");
    /// ```
    pub fn whisper_id(&self, cid: PlayerId, msg: &str) {
        if let Some(nick) = self.names.get(&cid.0) {
            self.whisper(nick, msg);
        }
    }

    /// Sends a direct message over inter-plugin IPC to `target` plugin.
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.send_to("plugin_hub", "PING");
    /// ```
    pub fn send_to(&self, target: &str, payload: &str) {
        self.raw(&format!("@TO {}|{}", target, payload));
    }

    /// Broadcasts a payload over the global inter-plugin bus.
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.broadcast_bus("SYNC");
    /// ```
    pub fn broadcast_bus(&self, payload: &str) {
        self.raw(&format!("@BUS {}", payload));
    }

    /// Sets a raw key-value entry in `plugins_kv.db`.
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.db_set("demo:key", "1");
    /// ```
    pub fn db_set(&self, key: &str, val: &str) {
        self.raw(&format!("@DB SET|{}|{}", key, val));
    }

    /// Requests a key-value entry from `plugins_kv.db` (delivered via `Event::DbValue`).
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.db_get("req1", "demo:key");
    /// ```
    pub fn db_get(&self, req_id: &str, key: &str) {
        self.raw(&format!("@DB GET|{}|{}", req_id, key));
    }

    /// Atomically adds `delta` to an integer key in `plugins_kv.db`.
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.db_add("demo:count", 5);
    /// ```
    pub fn db_add(&self, key: &str, delta: i64) {
        self.raw(&format!("@DB ADD|{}|{}", key, delta));
    }

    /// Deletes a key from `plugins_kv.db`.
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.db_del("demo:key");
    /// ```
    pub fn db_del(&self, key: &str) {
        self.raw(&format!("@DB DEL|{}", key));
    }

    /// Deletes all keys matching `prefix` from `plugins_kv.db`.
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.db_del_prefix("demo:");
    /// ```
    pub fn db_del_prefix(&self, prefix: &str) {
        self.raw(&format!("@DB DEL_PREFIX|{}", prefix));
    }

    /// Queries all keys starting with `prefix` (delivered via `Event::DbList`).
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.db_prefix("req_p", "demo:");
    /// ```
    pub fn db_prefix(&self, req_id: &str, prefix: &str) {
        self.raw(&format!("@DB PREFIX|{}|{}", req_id, prefix));
    }

    /// Sets a persistent HUD line in slot `1..=5` for a player.
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// assert!(api.set_hud_line(PlayerId(0), 1, "Wave 1").is_ok());
    /// ```
    /// Returns true if client verified as BestClient.
    pub fn is_best_client(&self, cid: i32) -> bool {
        self.best_clients.contains(&cid)
    }

    /// Sends custom system packet (MsgId 100) to a verified BestClient.
    pub fn send_custom_packet(&self, cid: i32, kind: &str, payload: &str) {
        if self.best_clients.contains(&cid) {
            let clean_k: String = kind.chars().filter(|c| !c.is_whitespace() && *c != ';').collect();
            let clean_p: String = payload.chars().filter(|c| *c != '\n' && *c != '\r' && *c != ';').collect();
            if !clean_k.is_empty() {
                self.raw(&format!("bc_packet {} {} {}", cid, clean_k, clean_p));
            }
        }
    }

    /// Updates a custom HUD slot line for the given player.
    pub fn set_hud_line(&self, cid: PlayerId, slot: u8, text: &str) -> Result<(), SdkError> {
        if !cid.is_valid() {
            return Err(SdkError::InvalidCid(cid));
        }
        if slot > 4 || text.chars().count() > 200 {
            return Err(SdkError::InvalidArg {
                name: "slot/text",
                reason: "slot must be 0..=4 and text <= 200 chars",
            });
        }
        let clean = text.replace(['\n', '\r'], " ").replace('"', "'");
        self.raw(&format!("set_hud_line {} {} \"{}\"", cid.0, slot, clean));
        Ok(())
    }

    /// Clears a HUD line slot `1..=5` for a player.
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// assert!(api.clear_hud_line(PlayerId(0), 1).is_ok());
    /// ```
    pub fn clear_hud_line(&self, cid: PlayerId, slot: u8) -> Result<(), SdkError> {
        if !cid.is_valid() {
            return Err(SdkError::InvalidCid(cid));
        }
        if slot > 4 {
            return Err(SdkError::InvalidArg {
                name: "slot",
                reason: "slot must be 0..=4",
            });
        }
        self.raw(&format!("clear_hud_line {} {}", cid.0, slot));
        Ok(())
    }

    /// Renders a UTF-8 progress bar (`█`/`░` + percentage) into HUD slot 4.
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// assert!(api.send_progress_bar(PlayerId(0), "HP", 50, 100, 10).is_ok());
    /// ```
    pub fn send_progress_bar(
        &self,
        cid: PlayerId,
        title: &str,
        current: u32,
        max: u32,
        width: u8,
    ) -> Result<(), SdkError> {
        let w = (width.clamp(4, 30)) as usize;
        let denom = max.max(1);
        let clamped = current.min(denom);
        let filled = ((clamped as u64 * w as u64) / denom as u64) as usize;
        let pct = (clamped as u64 * 100) / denom as u64;
        let bar = format!(
            "{} [{}{}] {}%",
            title,
            "█".repeat(filled),
            "░".repeat(w.saturating_sub(filled)),
            pct
        );
        self.set_hud_line(cid, 4, &bar)
    }

    /// Spawns a custom server projectile tracked in the state registry.
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// assert!(api.spawn_projectile(1, PlayerId(0), Vec2::new(100.0, 100.0), Vec2::new(10.0, 0.0), 0.5, 2000).is_ok());
    /// ```
    pub fn spawn_projectile(
        &self,
        id: i32,
        owner_cid: PlayerId,
        pos: Vec2,
        vel: Vec2,
        gravity: f32,
        ttl_ms: u32,
    ) -> Result<(), SdkError> {
        self.raw(&format!(
            "spawn_proj {} {} {:.1} {:.1} {:.2} {:.2} {:.3} {}",
            id, owner_cid.0, pos.x, pos.y, vel.x, vel.y, gravity, ttl_ms
        ));
        Ok(())
    }

    /// Despawns a previously spawned custom projectile by identifier.
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.despawn_projectile(1);
    /// ```
    pub fn despawn_projectile(&self, id: i32) {
        self.raw(&format!("despawn_proj {}", id));
    }

    /// Overrides player movement inputs for the next tick (`force_input` capability).
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let mut api = ServerApi::new();
    /// api.set_manifest("10.0", &["force_input"]);
    /// assert!(api.force_input(PlayerId(0), 1, 1, 0, 0).is_ok());
    /// ```
    pub fn force_input(
        &self,
        cid: PlayerId,
        dir: i32,
        jump: i32,
        hook: i32,
        fire: i32,
    ) -> Result<(), SdkError> {
        if !self.has_permission("force_input") {
            return Err(SdkError::PermissionDenied {
                needed: "force_input",
            });
        }
        if !cid.is_valid() {
            return Err(SdkError::InvalidCid(cid));
        }
        self.raw(&format!(
            "force_input {} {} {} {} {}",
            cid.0,
            dir.clamp(-1, 1),
            if jump != 0 { 1 } else { 0 },
            if hook != 0 { 1 } else { 0 },
            if fire != 0 { 1 } else { 0 }
        ));
        Ok(())
    }

    /// Clears forced movement input for a player (`force_input` capability).
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let mut api = ServerApi::new();
    /// api.set_manifest("10.0", &["force_input"]);
    /// assert!(api.clear_force_input(PlayerId(0)).is_ok());
    /// ```
    pub fn clear_force_input(&self, cid: PlayerId) -> Result<(), SdkError> {
        if !self.has_permission("force_input") {
            return Err(SdkError::PermissionDenied {
                needed: "force_input",
            });
        }
        if !cid.is_valid() {
            return Err(SdkError::InvalidCid(cid));
        }
        self.raw(&format!("clear_force_input {}", cid.0));
        Ok(())
    }

    /// Atomically adds `delta` to an account-scoped numeric key (`account:key`).
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.db_add_by_account("alice", "coins", 100);
    /// ```
    pub fn db_add_by_account(&self, account: &str, key: &str, delta: i64) {
        self.raw(&format!("@DB ADD_ACC|{}|{}|{}", account, key, delta));
    }

    /// Atomically transfers `amount` between two accounts (status via `Event::DbValue`).
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.db_transfer_by_account("tx1", "alice", "bob", "coins", 25);
    /// ```
    pub fn db_transfer_by_account(
        &self,
        req_id: &str,
        from_acc: &str,
        to_acc: &str,
        key: &str,
        amount: i64,
    ) {
        self.raw(&format!(
            "@DB XFER|{}|{}|{}|{}|{}",
            req_id, from_acc, to_acc, key, amount
        ));
    }

    /// Persists a plugin-namespaced state value (`pstate:<plugin>:<key>`).
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.state_set("wave", "3");
    /// ```
    pub fn state_set(&self, key: &str, val: &str) {
        self.raw(&format!("@PSTATE SET|{}|{}", key, val));
    }

    /// Reads a plugin-namespaced state value (delivered via `Event::DbValue`).
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.state_get("st1", "wave");
    /// ```
    pub fn state_get(&self, req_id: &str, key: &str) {
        self.raw(&format!("@PSTATE GET|{}|{}", req_id, key));
    }

    /// Schedules a one-shot `Event::Scheduled { callback_id }` after `delay_ms`.
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.schedule_after(1000, 42);
    /// ```
    pub fn schedule_after(&self, delay_ms: u32, callback_id: u64) {
        self.raw(&format!("@SCHED AFTER|{}|{}", delay_ms, callback_id));
    }

    /// Schedules a repeating `Event::Scheduled { callback_id }` every `interval_ms`.
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.schedule_every(500, 7);
    /// ```
    pub fn schedule_every(&self, interval_ms: u32, callback_id: u64) {
        self.raw(&format!("@SCHED EVERY|{}|{}", interval_ms, callback_id));
    }

    /// Cancels a previously scheduled timer by `callback_id`.
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.cancel_schedule(42);
    /// ```
    pub fn cancel_schedule(&self, callback_id: u64) {
        self.raw(&format!("@SCHED CANCEL|{}", callback_id));
    }

    /// Runs the main plugin event loop with panic isolation.
    ///
    /// # Examples
    /// ```no_run
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.run(|_api, _ev| {});
    /// ```
    pub fn run<F: FnMut(&mut ServerApi, Event)>(self, mut handler: F) {
        self.run_intercept(move |api, ev| {
            handler(api, ev);
            EventAction::Pass
        });
    }

    /// Runs the main plugin event loop allowing event cancellation and panic isolation.
    ///
    /// # Examples
    /// ```no_run
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.run_intercept(|_api, _ev| EventAction::Pass);
    /// ```
    pub fn run_intercept<F: FnMut(&mut ServerApi, Event) -> EventAction>(mut self, mut handler: F) {
        let (tx, rx) = mpsc::channel::<String>();
        thread::spawn(move || {
            let stdin = io::stdin();
            for line in stdin.lock().lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });

        for raw_line in rx {
            let line = raw_line.trim();
            if line.is_empty() {
                continue;
            }
            if line == "PING" {
                self.raw("@PONG");
                continue;
            }
            if let Some(lc) = line.strip_prefix("LIFECYCLE|") {
                match lc {
                    "LOAD" => {
                        if let Some(mut cb) = self.on_load_cb.take() {
                            let res = catch_unwind(AssertUnwindSafe(|| cb(&mut self)));
                            self.on_load_cb = Some(cb);
                            if res.is_err() {
                                self.raw("@PM PANIC|on_load");
                                break;
                            }
                        }
                    }
                    "RELOAD" => {
                        if let Some(mut cb) = self.on_reload_cb.take() {
                            let res = catch_unwind(AssertUnwindSafe(|| cb(&mut self)));
                            self.on_reload_cb = Some(cb);
                            if res.is_err() {
                                self.raw("@PM PANIC|on_reload");
                                break;
                            }
                        }
                    }
                    "UNLOAD" => {
                        if let Some(mut cb) = self.on_unload_cb.take() {
                            let _ = catch_unwind(AssertUnwindSafe(|| cb(&mut self)));
                            self.on_unload_cb = Some(cb);
                        }
                        self.raw("@PM UNLOAD_ACK");
                        break;
                    }
                    _ => {}
                }
                continue;
            }
            if let Some(rest) = line.strip_prefix("SYNC_PLAYER|") {
                let p: Vec<&str> = rest.splitn(4, '|').collect();
                if let (Some(nick), Some(cid_s), Some(ip), Some(acc)) =
                    (p.first(), p.get(1), p.get(2), p.get(3))
                {
                    let n = (*nick).to_string();
                    self.online.insert(n.clone());
                    if let Ok(cid) = cid_s.parse::<i32>()
                        && cid >= 0
                    {
                        self.cids.insert(n.clone(), cid);
                        self.names.insert(cid, n.clone());
                    }
                    if !ip.is_empty() {
                        self.ips.insert(n.clone(), (*ip).to_string());
                    }
                    if !acc.is_empty() {
                        self.accounts.insert(n.clone(), (*acc).to_string());
                        self.authed_accounts.insert(n.clone(), (*acc).to_string());
                    }
                    if let Ok(cid) = cid_s.parse::<i32>() && cid >= 0 {
                        self.player_cids.insert(n, cid);
                    }
                }
                continue;
            }

            let mut ev_id: Option<u64> = None;
            let body = if let Some(rest) = line.strip_prefix("EV|") {
                if let Some((id_s, tail)) = rest.split_once('|') {
                    ev_id = id_s.parse::<u64>().ok();
                    tail
                } else {
                    line
                }
            } else {
                line
            };

            if let Some(ev) = self.parse_event(body) {
                let res = catch_unwind(AssertUnwindSafe(|| handler(&mut self, ev)));
                match res {
                    Ok(act) => {
                        if let Some(id) = ev_id {
                            let tag = if act == EventAction::Cancel {
                                "CANCEL"
                            } else {
                                "PASS"
                            };
                            self.raw(&format!("@ACK {}|{}", id, tag));
                        }
                    }
                    Err(_) => {
                        self.raw("@PM PANIC|event_handler");
                        break;
                    }
                }
            } else if let Some(id) = ev_id {
                self.raw(&format!("@ACK {}|PASS", id));
            }
        }
    }

    /// Sends a raw server console command (legacy v9.2 alias).
    pub fn cmd(&self, c: &str) {
        self.raw(c);
    }
    /// Registers an admin-only plugin command (legacy v9.2 alias).
    pub fn register_admin_plugin(&mut self, cmd: &str, desc: &str) {
        crate::log::init(cmd);
        self.raw(&format!("@REG_ADMIN {}|{}", cmd, desc));
    }
    /// Registers a subcommand description (legacy v9.2 alias).
    pub fn register_cmd(&mut self, cmd: &str, args: &str, desc: &str) {
        self.raw(&format!("@REG_SUB {}|{}|{}", cmd, args, desc));
    }
    /// Checks whether a player holds a specific role.
    pub fn has_role(&self, player: &str, role: &str) -> bool {
        self.accounts
            .get(player)
            .and_then(|a| self.roles.get(a))
            .map(|r| r.eq_ignore_ascii_case(role))
            .unwrap_or(false)
    }
    /// Checks whether a player is staff (`owner`, `admin`, `mod`, `helper`).
    pub fn is_staff(&self, player: &str) -> bool {
        ["owner", "admin", "mod", "helper"]
            .iter()
            .any(|r| self.has_role(player, r))
    }
    /// Broadcasts payload over global IPC bus (legacy v9.2 alias).
    pub fn broadcast_ipc(&self, payload: &str) {
        self.broadcast_bus(payload);
    }
    /// Sends a broadcast banner to a single client ID.
    pub fn broadcast_to(&self, cid: i32, msg: &str) {
        self.raw(&format!("broadcast_to {} {}", cid, msg));
    }
    /// Queries keys starting with prefix (legacy v9.2 alias).
    pub fn db_get_prefix(&self, req_id: &str, prefix: &str) {
        self.db_prefix(req_id, prefix);
    }
    /// Requests immediate player state snapshot.
    pub fn request_state(&self, cid: i32) {
        self.raw(&format!("get_state {}", cid));
    }
    /// Resets player hook.
    pub fn reset_hook(&self, cid: i32) {
        self.raw(&format!("reset_hook {}", cid));
    }
    /// Plays a sound effect for a client ID (`-1` for all).
    pub fn play_sound(&self, cid: i32, id: i32) {
        self.raw(&format!("play_sound {} {}", cid, id));
    }
    /// Freezes or unfreezes a player for `secs` seconds.
    pub fn set_freeze(&self, cid: i32, secs: i32) {
        self.raw(&format!("set_freeze {} {}", cid, secs));
    }
    /// Sets player velocity.
    pub fn set_vel(&self, cid: i32, vx: f32, vy: f32) {
        self.raw(&format!("set_vel {} {:.2} {:.2}", cid, vx, vy));
    }
    /// Adds relative impulse (vx, vy) to player current velocity.
    pub fn apply_impulse(&self, cid: i32, vx: f32, vy: f32) {
        self.raw(&format!("plugin_apply_impulse {} {:.2} {:.2}", cid, vx, vy));
    }
    /// Sets player world position.
    pub fn set_pos(&self, cid: i32, x: f32, y: f32) {
        self.raw(&format!("set_pos {} {:.1} {:.1}", cid, x, y));
    }

    /// Gives a weapon ID (`0..=5`) to a player.
    pub fn give_weapon(&self, cid: i32, w: i32) {
        self.raw(&format!("give_weapon {} {}", cid, w));
    }
    /// Sets weapon ammo for a player.
    pub fn set_ammo(&self, cid: i32, w: i32, ammo: i32) {
        self.raw(&format!("set_ammo {} {} {}", cid, w, ammo));
    }
    /// Toggles player invincibility.
    pub fn set_invincible(&self, cid: i32, on: bool) {
        self.raw(&format!(
            "plugin_set_invincible {} {}",
            cid,
            if on { 1 } else { 0 }
        ));
    }
    /// Toggles endless jump for a player.
    pub fn set_endless_jump(&self, cid: i32, on: bool) {
        self.raw(&format!(
            "plugin_set_endless_jump {} {}",
            cid,
            if on { 1 } else { 0 }
        ));
    }
    /// Toggles endless hook for a player.
    pub fn set_endless_hook(&self, cid: i32, on: bool) {
        self.raw(&format!(
            "plugin_set_endless_hook {} {}",
            cid,
            if on { 1 } else { 0 }
        ));
    }
    /// Sets a named player physics flag (`jetpack`, `collision`, `hammer_hit`, `hook_hit`).
    pub fn set_phys(&self, cid: i32, key: &str, on: bool) {
        self.raw(&format!(
            "set_phys {} {} {}",
            cid,
            key,
            if on { 1 } else { 0 }
        ));
    }
    /// Sets custom tee body/feet color.
    pub fn set_color(&self, cid: i32, custom: bool, body: i32, feet: i32) {
        self.raw(&format!(
            "plugin_set_color {} {} {} {}",
            cid,
            if custom { 1 } else { 0 },
            body,
            feet
        ));
    }
    /// Overrides `target` player's visible skin specifically for `viewer` (pass `""` to reset).
    pub fn set_skin_to(&self, viewer: i32, target: i32, skin: &str) {
        let clean: String = skin
            .chars()
            .filter(|c| !c.is_control() && *c != '"' && *c != '\\' && !c.is_whitespace())
            .take(23)
            .collect();
        if clean.is_empty() {
            self.raw(&format!("plugin_set_skin_to {} {}", viewer, target));
        } else {
            self.raw(&format!("plugin_set_skin_to {} {} {}", viewer, target, clean));
        }
    }
    /// Resets `target` player's visible skin for `viewer` back to their real skin.
    pub fn clear_skin_to(&self, viewer: i32, target: i32) {
        self.set_skin_to(viewer, target, "");
    }
    /// Toggles VIP+ status for a player.
    pub fn set_vip_plus(&self, cid: i32, on: bool) {
        self.raw(&format!("set_vip_plus {} {}", cid, if on { 1 } else { 0 }));
    }
    /// Force-kills a player character.
    pub fn force_kill(&self, cid: i32) {
        self.raw(&format!("force_kill {}", cid));
    }
    /// Kills `cid` credited to `killer` with `weapon`.
    pub fn kill_by(&self, cid: i32, killer: i32, w: i32) {
        self.raw(&format!("kill_by {} {} {}", cid, killer, w));
    }
    /// Spawns a visual particle effect (`explosion`, `hammer`, `damage`).
    pub fn spawn_fx(&self, kind: &str, x: f32, y: f32) {
        self.raw(&format!("spawn_fx {} {:.1} {:.1}", kind, x, y));
    }
    /// Spawns a standard laser beam.
    pub fn spawn_laser(&self, x1: f32, y1: f32, x2: f32, y2: f32) {
        self.raw(&format!(
            "spawn_laser {:.1} {:.1} {:.1} {:.1}",
            x1, y1, x2, y2
        ));
    }
    /// Spawns a styled laser beam.
    pub fn spawn_laser_ex(&self, x1: f32, y1: f32, x2: f32, y2: f32, kind: i32) {
        self.raw(&format!(
            "spawn_laser_ex {:.1} {:.1} {:.1} {:.1} {}",
            x1, y1, x2, y2, kind
        ));
    }
    /// Spawns a persistent visual laser with ID.
    pub fn spawn_visual_laser(&self, id: i32, x1: f32, y1: f32, x2: f32, y2: f32, kind: i32) {
        self.raw(&format!(
            "spawn_visual_laser {} {:.1} {:.1} {:.1} {:.1} {}",
            id, x1, y1, x2, y2, kind
        ));
    }
    /// Clears all persistent visual lasers.
    /// Removes a persistent visual laser by ID.
    pub fn remove_visual_laser(&self, id: i32) {
        self.raw(&format!("remove_visual_laser {}", id));
    }
    /// Clears all persistent visual lasers spawned by this plugin.
    pub fn clear_visual_lasers(&self) {
        self.raw("clear_visual_lasers");
    }
    /// Spawns a persistent visual pickup with ID.
    pub fn spawn_visual_pickup(&self, id: i32, kind: i32, sub: i32, x: f32, y: f32) {
        self.raw(&format!(
            "spawn_visual_pickup {} {} {} {:.1} {:.1}",
            id, kind, sub, x, y
        ));
    }
    /// Spawns a persistent visual pickup attached to a player CID.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_visual_pickup_follow(
        &self,
        id: i32,
        kind: i32,
        sub: i32,
        cid: i32,
        off_x: f32,
        off_y: f32,
        bob_amp: f32,
    ) {
        self.raw(&format!(
            "spawn_visual_pickup_follow {} {} {} {} {:.1} {:.1} {:.1}",
            id, kind, sub, cid, off_x, off_y, bob_amp
        ));
    }
    /// Sets scoreboard mode (`0` = Race/Time, `1` = Points, `2` = Team Points).
    pub fn set_gamemode(&self, mode: i32) { self.raw(&format!("set_gamemode {}", mode)); }
    /// Sets a player's integer scoreboard points.
    pub fn set_player_score(&self, cid: i32, score: i32) { self.raw(&format!("set_player_score {} {}", cid, score)); }
    /// Sets a player's team (`0` = Red, `1` = Blue, `-1` = Spectators).
    pub fn set_player_team(&self, cid: i32, team: i32) { self.raw(&format!("set_player_team {} {}", cid, team)); }
    /// Sets Red and Blue team scores.
    pub fn set_team_score(&self, red: i32, blue: i32) { self.raw(&format!("set_team_score {} {}", red, blue)); }
    /// Removes a persistent visual pickup by ID.
    pub fn remove_visual_pickup(&self, id: i32) {
        self.raw(&format!("remove_visual_pickup {}", id));
    }
    /// Spawns a persistent visual tee (ID 0..63) in OnSnap.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_visual_tee(&self, id: i32, x: f32, y: f32, skin: &str, body: i32, feet: i32, freeze: i32) {
        self.raw(&format!(
            "spawn_visual_tee {} {:.1} {:.1} \"{}\" {} {} {}",
            id, x, y, skin.replace('"', ""), body, feet, freeze
        ));
    }
    /// Removes a persistent visual tee by ID.
    pub fn remove_visual_tee(&self, id: i32) {
        self.raw(&format!("remove_visual_tee {}", id));
    }
    /// Clears all persistent visual tees.
    pub fn clear_visual_tees(&self) {
        self.raw("clear_visual_tees");
    }
    /// Spawns persistent vector laser text in OnSnap (ID 0..31, 0 FX/s).
    pub fn spawn_visual_text(&self, id: i32, x: f32, y: f32, scale: f32, kind: i32, text: &str) {
        let clean = text.replace('"', "");
        self.raw(&format!(
            "spawn_visual_text {} {:.1} {:.1} {:.2} {} \"{}\"",
            id, x, y, scale, kind, clean
        ));
    }
    /// Removes a persistent visual text by ID.
    pub fn remove_visual_text(&self, id: i32) {
        self.raw(&format!("remove_visual_text {}", id));
    }
    /// Clears all persistent visual texts of this plugin.
    pub fn clear_visual_texts(&self) {
        self.raw("clear_visual_texts");
    }

    fn parse_event(&mut self, body: &str) -> Option<Event> {
        if let Some(rest) = body.strip_prefix("ONLINE|") {
            self.online = rest
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToString::to_string)
                .collect();
            return None;
        }
        if let Some(s) = body.strip_prefix("TICK|") {
            let sec: u64 = s.parse().ok()?;
            return Some(Event::Tick { uptime_sec: sec });
        }
        if let Some(s) = body.strip_prefix("FTICK|") {
            let ms: u64 = s.parse().ok()?;
            return Some(Event::FastTick { uptime_ms: ms });
        }
        if let Some(s) = body.strip_prefix("SCHED|") {
            let callback_id: u64 = s.parse().ok()?;
            return Some(Event::Scheduled { callback_id });
        }
        if let Some(rest) = body.strip_prefix("IPC|") {
            let (from, payload) = rest.split_once('|')?;
            return Some(Event::Ipc {
                from: from.to_string(),
                payload: payload.to_string(),
            });
        }
        if let Some(rest) = body.strip_prefix("DB_VAL|") {
            let (req_id, value) = rest.split_once('|')?;
            return Some(Event::DbValue {
                req_id: req_id.to_string(),
                value: value.to_string(),
            });
        }
        if let Some(rest) = body.strip_prefix("DB_LIST|") {
            let (req_id, raw_items) = rest.split_once('|')?;
            let items = raw_items
                .split(';')
                .filter(|s| !s.is_empty())
                .filter_map(|kv| {
                    kv.split_once('=')
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                })
                .collect();
            return Some(Event::DbList {
                req_id: req_id.to_string(),
                items,
            });
        }
        if let Some(rest) = body.strip_prefix("JOIN|") {
            let p: Vec<&str> = rest.splitn(3, '|').collect();
            let player = p.first()?.to_string();
            let cid = p.get(1).and_then(|s| s.parse().ok()).unwrap_or(-1);
            let ip = p.get(2).copied().unwrap_or("").to_string();
            self.online.insert(player.clone());
            if cid >= 0 {
                self.cids.insert(player.clone(), cid);
                self.player_cids.insert(player.clone(), cid);
                self.names.insert(cid, player.clone());
            }
            if !ip.is_empty() {
                self.ips.insert(player.clone(), ip.clone());
                self.player_ips.insert(player.clone(), ip.clone());
            }
            return Some(Event::Join { player });
        }
        if let Some(nick) = body.strip_prefix("LEAVE|") {
            let player = nick.to_string();
            self.online.remove(&player);
            let cid_opt = self.cids.remove(&player).or_else(|| self.player_cids.remove(&player));
            self.player_cids.remove(&player);
            self.last_leave_cid = cid_opt;
            if let Some(cid) = cid_opt {
                self.names.remove(&cid);
                self.states.remove(&cid);
                self.best_clients.remove(&cid);
            }
            self.ips.remove(&player);
            self.player_ips.remove(&player);
            self.accounts.remove(&player);
            self.authed_accounts.remove(&player);
            return Some(Event::Leave { player });
        }
        if let Some(rest) = body.strip_prefix("RENAME|") {
            let (old, new) = rest.split_once('|')?;
            self.online.remove(old);
            self.online.insert(new.to_string());
            if let Some(cid) = self.cids.remove(old).or_else(|| self.player_cids.remove(old)) {
                self.player_cids.remove(old);
                self.cids.insert(new.to_string(), cid);
                self.player_cids.insert(new.to_string(), cid);
                self.names.insert(cid, new.to_string());
            }
            if let Some(ip) = self.ips.remove(old).or_else(|| self.player_ips.remove(old)) {
                self.player_ips.remove(old);
                self.ips.insert(new.to_string(), ip.clone());
                self.player_ips.insert(new.to_string(), ip);
            }
            if let Some(acc) = self.accounts.remove(old).or_else(|| self.authed_accounts.remove(old)) {
                self.authed_accounts.remove(old);
                self.accounts.insert(new.to_string(), acc.clone());
                self.authed_accounts.insert(new.to_string(), acc);
            }
            return Some(Event::Rename {
                old: old.to_string(),
                new: new.to_string(),
                new_name: new.to_string(),
            });
        }
        if let Some(rest) = body.strip_prefix("CHAT|") {
            let p: Vec<&str> = rest.splitn(3, '|').collect();
            return Some(Event::Chat {
                team: p.first()?.parse().ok()?,
                player: (*p.get(1)?).to_string(),
                message: (*p.get(2)?).to_string(),
            });
        }
        if let Some(rest) = body.strip_prefix("BESTCLIENT|") {
            let (cid_s, ver) = rest.split_once('|')?;
            let cid = cid_s.parse().ok()?;
            self.best_clients.insert(cid);
            return Some(Event::BestClient { cid, version: ver.to_string() });
        }
        if let Some(rest) = body.strip_prefix("AUTH|") {
            let p: Vec<&str> = rest.splitn(3, '|').collect();
            let cid = p.first()?.parse().ok()?;
            let player = (*p.get(1)?).to_string();
            let account = (*p.get(2)?).to_string();
            self.accounts.insert(player.clone(), account.clone());
            self.authed_accounts.insert(player.clone(), account.clone());
            return Some(Event::Auth {
                cid,
                player,
                account,
            });
        }
        if let Some(rest) = body.strip_prefix("ROLE|") {
            let (acc, role) = rest.split_once('|')?;
            if role.is_empty() {
                self.roles.remove(acc);
            } else {
                self.roles.insert(acc.to_string(), role.to_string());
            }
            return Some(Event::Role {
                account: acc.to_string(),
                role: role.to_string(),
            });
        }
        if let Some(rest) = body.strip_prefix("KILL|") {
            let p: Vec<&str> = rest.splitn(3, '|').collect();
            return Some(Event::Kill {
                killer: (*p.first()?).to_string(),
                victim: (*p.get(1)?).to_string(),
                weapon: p.get(2)?.parse().ok()?,
            });
        }
        if let Some(rest) = body.strip_prefix("STATE|") {
            let p: Vec<&str> = if rest.contains(':') {
                rest.split(':').collect()
            } else {
                rest.split_whitespace().collect()
            };
            let off = if p.len() >= 7 && p[1].parse::<f32>().is_err() { 1 } else { 0 };
            if p.len() >= 6 + off {
                let cid = p[0].parse().ok()?;
                let pos = PlayerPos {
                    x: p[1 + off].parse().ok()?,
                    y: p[2 + off].parse().ok()?,
                    vx: p[3 + off].parse().ok()?,
                    vy: p[4 + off].parse().ok()?,
                    frozen: p[5 + off].parse().ok().unwrap_or(0),
                    alive: p.get(6 + off).map(|v| *v == "1").unwrap_or(true),
                    weapon: p.get(7 + off).and_then(|v| v.parse().ok()).unwrap_or(0),
                };
                self.states.insert(cid, pos);
                return Some(Event::State { cid, pos });
            }
        }
        if let Some(rest) = body.strip_prefix("INPUT|") {
            let is_colon = rest.contains(':');
            let p: Vec<&str> = if is_colon {
                rest.split(':').collect()
            } else {
                rest.split_whitespace().collect()
            };
            if p.len() >= 5 {
                let (dir_i, jump_i, hook_i, fire_i) = if is_colon { (4, 2, 3, 1) } else { (1, 2, 3, 4) };
                return Some(Event::Input {
                    cid: p.first()?.parse().ok()?,
                    dir: p.get(dir_i)?.parse().ok()?,
                    jump: p.get(jump_i)?.parse().ok()?,
                    hook: p.get(hook_i)?.parse().ok()?,
                    fire: p.get(fire_i)?.parse().ok()?,
                    target_x: p.get(5).and_then(|v| v.parse().ok()).unwrap_or(0),
                    target_y: p.get(6).and_then(|v| v.parse().ok()).unwrap_or(0),
                });
            }
        }
        if let Some(rest) = body.strip_prefix("DEATH|") {
            let p: Vec<&str> = rest.split_whitespace().collect();
            if p.len() >= 3 {
                return Some(Event::Death {
                    cid: p.first()?.parse().ok()?,
                    killer: p.get(1)?.parse().ok()?,
                    weapon: p.get(2)?.parse().ok()?,
                });
            }
        }
        if let Some(rest) = body.strip_prefix("DAMAGE|") {
            let p: Vec<&str> = rest.split_whitespace().collect();
            if p.len() >= 4 {
                return Some(Event::Damage {
                    cid: p.first()?.parse().ok()?,
                    attacker: p.get(1)?.parse().ok()?,
                    amount: p.get(2)?.parse().ok()?,
                    weapon: p.get(3)?.parse().ok()?,
                });
            }
        }
        if let Some(rest) = body.strip_prefix("FINISH|") {
            let p: Vec<&str> = rest.split_whitespace().collect();
            if p.len() >= 2 {
                return Some(Event::Finish {
                    cid: p.first()?.parse().ok()?,
                    time: p.get(1)?.parse().ok()?,
                });
            }
        }
        if let Some(rest) = body.strip_prefix("TILE|") {
            let p: Vec<&str> = rest.split_whitespace().collect();
            if p.len() >= 4 {
                return Some(Event::Tile {
                    cid: p.first()?.parse().ok()?,
                    tile: p.get(1)?.parse().ok()?,
                    x: p.get(2)?.parse().ok()?,
                    y: p.get(3)?.parse().ok()?,
                });
            }
        }
        if let Some(rest) = body.strip_prefix("WHIT|") {
            let p: Vec<&str> = rest.split_whitespace().collect();
            if p.len() >= 3 {
                return Some(Event::WeaponHit {
                    attacker: p.first()?.parse().ok()?,
                    victim: p.get(1)?.parse().ok()?,
                    weapon: p.get(2)?.parse().ok()?,
                });
            }
        }
        if let Some(rest) = body.strip_prefix("HAMMER|")
            && let Some((a, v)) = rest.split_once('|')
        {
            return Some(Event::HammerHit {
                attacker: a.to_string(),
                victim: v.to_string(),
            });
        }
        if let Some(rest) = body.strip_prefix("PICKUP|") {
            let p: Vec<&str> = rest.split_whitespace().collect();
            if p.len() >= 2 {
                return Some(Event::Pickup {
                    cid: p.first()?.parse().ok()?,
                    kind: p.get(1)?.parse().ok()?,
                });
            }
        }
        if let Some(rest) = body.strip_prefix("WPN_CHG|") {
            let p: Vec<&str> = rest.split(|c: char| c == ':' || c == '|' || c.is_whitespace()).filter(|s| !s.is_empty()).collect();
            if p.len() >= 3 {
                return Some(Event::WeaponChange {
                    cid: p.first()?.parse().ok()?,
                    old: p.get(1)?.parse().ok()?,
                    new: p.get(2)?.parse().ok()?,
                });
            }
        }
        if let Some(rest) = body.strip_prefix("HOOK_ATTACH|") {
            let p: Vec<&str> = rest.split(':').collect();
            if p.len() >= 4 {
                let cid: i32 = p[0].parse().ok()?;
                let raw_hooked: i32 = p[1].parse().ok()?;
                let x: f32 = p[2].parse().ok()?;
                let y: f32 = p[3].parse().ok()?;
                return Some(Event::HookAttach {
                    cid,
                    hooked_cid: if raw_hooked >= 0 { Some(raw_hooked) } else { None },
                    pos: crate::dx::Vec2::new(x, y),
                });
            }
        }
        if let Some(rest) = body.strip_prefix("HOOK_DETACH|") {
            let p: Vec<&str> = rest.split(':').collect();
            if p.len() >= 2 {
                let cid: i32 = p[0].parse().ok()?;
                let raw_prev: i32 = p[1].parse().ok()?;
                return Some(Event::HookDetach {
                    cid,
                    prev_hooked_cid: if raw_prev >= 0 { Some(raw_prev) } else { None },
                });
            }
        }
        None
    }
}

/// Declares a DDNet++ plugin entry point, registration, config, state, and event loop.
#[macro_export]
macro_rules! plugin {
    (name: $n:expr, desc: $d:expr, $(admin: $a:expr,)? $(priority: $p:expr,)? $(cmds: [ $( ($c:expr, $ca:expr, $cd:expr) ),* $(,)? ],)? handler: $h:expr $(,)?) => {
        fn main() {
            let mut api = $crate::ServerApi::new();
            let mut _adm = false; $( _adm = $a; )?
            if _adm { api.register_admin_plugin($n, $d); } else { api.register_plugin($n, $d); }
            $( api.set_priority($p); )? $( $( api.register_cmd($c, $ca, $cd); )* )?
            api.run_handler($h);
        }
    };
    (name: $n:expr, desc: $d:expr, $(admin: $a:expr,)? $(priority: $p:expr,)? $(cmds: [ $( ($c:expr, $ca:expr, $cd:expr) ),* $(,)? ],)? config: $ct:ty, $(state: { $( $sn:ident : $st:ty = $sv:expr ),* $(,)? },)? intercept($api:ident, $cfg:ident, $ev:ident) $b:block) => {
        fn main() {
            let mut $api = $crate::ServerApi::new();
            let mut _adm = false; $( _adm = $a; )?
            if _adm { $api.register_admin_plugin($n, $d); } else { $api.register_plugin($n, $d); }
            $( $api.set_priority($p); )? $( $( $api.register_cmd($c, $ca, $cd); )* )?
            let mut $cfg = <$ct as $crate::PluginConfig>::load_or_create($n);
            $( $( let mut $sn: $st = $sv; )* )?
            $api.run_intercept(move |$api, $ev| $b);
        }
    };
    (name: $n:expr, desc: $d:expr, $(admin: $a:expr,)? $(priority: $p:expr,)? $(cmds: [ $( ($c:expr, $ca:expr, $cd:expr) ),* $(,)? ],)? config: $ct:ty, $(state: { $( $sn:ident : $st:ty = $sv:expr ),* $(,)? },)? handle($api:ident, $cfg:ident, $ev:ident) $b:block) => {
        fn main() {
            let mut $api = $crate::ServerApi::new();
            let mut _adm = false; $( _adm = $a; )?
            if _adm { $api.register_admin_plugin($n, $d); } else { $api.register_plugin($n, $d); }
            $( $api.set_priority($p); )? $( $( $api.register_cmd($c, $ca, $cd); )* )?
            let mut $cfg = <$ct as $crate::PluginConfig>::load_or_create($n);
            $( $( let mut $sn: $st = $sv; )* )?
            $api.run(move |$api, $ev| $b);
        }
    };
    (name: $n:expr, desc: $d:expr, $(admin: $a:expr,)? $(priority: $p:expr,)? $(cmds: [ $( ($c:expr, $ca:expr, $cd:expr) ),* $(,)? ],)? $(state: { $( $sn:ident : $st:ty = $sv:expr ),* $(,)? },)? intercept($api:ident, $ev:ident) $b:block) => {
        fn main() {
            let mut $api = $crate::ServerApi::new();
            let mut _adm = false; $( _adm = $a; )?
            if _adm { $api.register_admin_plugin($n, $d); } else { $api.register_plugin($n, $d); }
            $( $api.set_priority($p); )? $( $( $api.register_cmd($c, $ca, $cd); )* )? $( $( let mut $sn: $st = $sv; )* )?
            $api.run_intercept(move |$api, $ev| $b);
        }
    };
    (name: $n:expr, desc: $d:expr, $(admin: $a:expr,)? $(priority: $p:expr,)? $(cmds: [ $( ($c:expr, $ca:expr, $cd:expr) ),* $(,)? ],)? $(state: { $( $sn:ident : $st:ty = $sv:expr ),* $(,)? },)? handle($api:ident, $ev:ident) $b:block) => {
        fn main() {
            let mut $api = $crate::ServerApi::new();
            let mut _adm = false; $( _adm = $a; )?
            if _adm { $api.register_admin_plugin($n, $d); } else { $api.register_plugin($n, $d); }
            $( $api.set_priority($p); )? $( $( $api.register_cmd($c, $ca, $cd); )* )? $( $( let mut $sn: $st = $sv; )* )?
            $api.run(move |$api, $ev| $b);
        }
    };
}
