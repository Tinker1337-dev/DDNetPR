use crate::{Event, EventAction, PlayerPos, ServerApi};

/// Trait-based event dispatcher with default no-op handlers for every Event variant.
#[allow(clippy::too_many_arguments)]
pub trait EventHandler {
    /// Called when a player joins the server.
    fn on_join(&mut self, _api: &mut ServerApi, _player: String) {}
    /// Called when a player leaves the server.
    fn on_leave(&mut self, _api: &mut ServerApi, _player: String) {}
    /// Called when a player changes nickname.
    fn on_rename(&mut self, _api: &mut ServerApi, _old: String, _new: String) {}
    /// Called on chat message; return EventAction::Cancel to block propagation.
    fn on_chat(&mut self, _api: &mut ServerApi, _team: i32, _player: String, _message: String) -> EventAction {
        EventAction::Pass
    }
    /// Called when BestClient handshake is detected.
    fn on_best_client(&mut self, _api: &mut ServerApi, _cid: i32, _version: String) {}
    /// Called when a player logs into an account.
    fn on_auth(&mut self, _api: &mut ServerApi, _cid: i32, _player: String, _account: String) {}
    /// Called when an account role is assigned or updated.
    fn on_role(&mut self, _api: &mut ServerApi, _account: String, _role: String) {}
    /// Called once per second.
    fn on_tick(&mut self, _api: &mut ServerApi, _uptime_sec: u64) {}
    /// Called every 100ms when fast tick is enabled.
    fn on_fast_tick(&mut self, _api: &mut ServerApi, _uptime_ms: u64) {}
    /// Called when a scheduled timer fires.
    fn on_scheduled(&mut self, _api: &mut ServerApi, _callback_id: u64) {}
    /// Called when an inter-plugin IPC message arrives.
    fn on_ipc(&mut self, _api: &mut ServerApi, _from: String, _payload: String) {}
    /// Called when a DB key-value lookup finishes.
    fn on_db_value(&mut self, _api: &mut ServerApi, _req_id: String, _value: String) {}
    /// Called when a DB prefix scan finishes.
    fn on_db_list(&mut self, _api: &mut ServerApi, _req_id: String, _items: Vec<(String, String)>) {}
    /// Called on player input snapshot.
    fn on_input(&mut self, _api: &mut ServerApi, _cid: i32, _dir: i32, _jump: i32, _hook: i32, _fire: i32, _target_x: i32, _target_y: i32) {}
    /// Called on legacy online list update.
    fn on_online(&mut self, _api: &mut ServerApi, _players: Vec<String>) {}
    /// Called on legacy DB result event.
    fn on_db_result(&mut self, _api: &mut ServerApi, _req_id: String, _value: String) {}
    /// Called on legacy detailed player state snapshot.
    fn on_player_state(&mut self, _api: &mut ServerApi, _cid: i32, _player: String, _x: f32, _y: f32, _vx: f32, _vy: f32, _frozen: i32, _weapon: i32) {}
    /// Called when player state snapshot updates.
    fn on_state(&mut self, _api: &mut ServerApi, _cid: i32, _pos: PlayerPos) {}
    /// Called when a player kills another player.
    fn on_kill(&mut self, _api: &mut ServerApi, _killer: String, _victim: String, _weapon: i32) {}
    /// Called when a player dies.
    fn on_death(&mut self, _api: &mut ServerApi, _cid: i32, _killer: i32, _weapon: i32) {}
    /// Called when a player takes damage.
    fn on_damage(&mut self, _api: &mut ServerApi, _cid: i32, _attacker: i32, _amount: i32, _weapon: i32) {}
    /// Called when a player finishes a map/race.
    fn on_finish(&mut self, _api: &mut ServerApi, _cid: i32, _time: f32) {}
    /// Called when a player touches a special tile.
    fn on_tile(&mut self, _api: &mut ServerApi, _cid: i32, _tile: i32, _x: i32, _y: i32) {}
    /// Called when a weapon hits a player.
    fn on_weapon_hit(&mut self, _api: &mut ServerApi, _attacker: i32, _victim: i32, _weapon: i32) {}
    /// Called when a hammer hits a player.
    fn on_hammer_hit(&mut self, _api: &mut ServerApi, _attacker: String, _victim: String) {}
    /// Called when a player collects a pickup.
    fn on_pickup(&mut self, _api: &mut ServerApi, _cid: i32, _kind: i32) {}
    /// Called when a player switches active weapon.
    fn on_weapon_change(&mut self, _api: &mut ServerApi, _cid: i32, _old: i32, _new: i32) {}
    /// Called when a player hook attaches to a tile or another player.
    fn on_hook_attach(&mut self, _api: &mut ServerApi, _cid: i32, _hooked_cid: Option<i32>, _pos: crate::dx::Vec2) {}
    /// Called when a player hook detaches.
    fn on_hook_detach(&mut self, _api: &mut ServerApi, _cid: i32, _prev_hooked_cid: Option<i32>) {}
    /// Dispatches a raw Event into the corresponding typed handler method.
    fn dispatch(&mut self, api: &mut ServerApi, ev: Event) -> EventAction {
        match ev {
            Event::Join { player } => self.on_join(api, player),
            Event::Leave { player } => self.on_leave(api, player),
            Event::Rename { old, new, .. } => self.on_rename(api, old, new),
            Event::Chat { team, player, message } => return self.on_chat(api, team, player, message),
            Event::BestClient { cid, version } => self.on_best_client(api, cid, version),
            Event::Auth { cid, player, account } => self.on_auth(api, cid, player, account),
            Event::Role { account, role } => self.on_role(api, account, role),
            Event::Tick { uptime_sec } => self.on_tick(api, uptime_sec),
            Event::FastTick { uptime_ms } => self.on_fast_tick(api, uptime_ms),
            Event::Scheduled { callback_id } => self.on_scheduled(api, callback_id),
            Event::Ipc { from, payload } => self.on_ipc(api, from, payload),
            Event::DbValue { req_id, value } => self.on_db_value(api, req_id, value),
            Event::DbList { req_id, items } => self.on_db_list(api, req_id, items),
            Event::Input { cid, dir, jump, hook, fire, target_x, target_y } => {
                self.on_input(api, cid, dir, jump, hook, fire, target_x, target_y);
            }
            Event::Online { players } => self.on_online(api, players),
            Event::DbResult { req_id, value } => self.on_db_result(api, req_id, value),
            Event::PlayerState { cid, player, x, y, vx, vy, frozen, weapon } => {
                self.on_player_state(api, cid, player, x, y, vx, vy, frozen, weapon);
            }
            Event::State { cid, pos } => self.on_state(api, cid, pos),
            Event::Kill { killer, victim, weapon } => self.on_kill(api, killer, victim, weapon),
            Event::Death { cid, killer, weapon } => self.on_death(api, cid, killer, weapon),
            Event::Damage { cid, attacker, amount, weapon } => self.on_damage(api, cid, attacker, amount, weapon),
            Event::Finish { cid, time } => self.on_finish(api, cid, time),
            Event::Tile { cid, tile, x, y } => self.on_tile(api, cid, tile, x, y),
            Event::WeaponHit { attacker, victim, weapon } => self.on_weapon_hit(api, attacker, victim, weapon),
            Event::HammerHit { attacker, victim } => self.on_hammer_hit(api, attacker, victim),
            Event::Pickup { cid, kind } => self.on_pickup(api, cid, kind),
            Event::WeaponChange { cid, old, new } => self.on_weapon_change(api, cid, old, new),
            Event::HookAttach { cid, hooked_cid, pos } => self.on_hook_attach(api, cid, hooked_cid, pos),
            Event::HookDetach { cid, prev_hooked_cid } => self.on_hook_detach(api, cid, prev_hooked_cid),
        }
        EventAction::Pass
    }
}

impl ServerApi {
    /// Runs the plugin event loop using a stateful EventHandler implementation.
    pub fn run_handler<H: EventHandler + 'static>(self, mut handler: H) {
        self.run_intercept(move |api, ev| handler.dispatch(api, ev));
    }
}
