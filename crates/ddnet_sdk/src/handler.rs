use crate::{Event, EventAction, PlayerPos, ServerApi};

/// Диспетчер событий на основе трейта с пустыми обработчиками по умолчанию для каждого варианта Event.
#[allow(clippy::too_many_arguments)]
pub trait EventHandler {
    /// Вызывается при входе игрока на сервер.
    fn on_join(&mut self, _api: &mut ServerApi, _player: String) {}
    /// Вызывается при выходе игрока с сервера.
    fn on_leave(&mut self, _api: &mut ServerApi, _player: String) {}
    /// Вызывается при смене команды игрока (`0` = Red, `1` = Blue, `-1` = Наблюдатель).
    fn on_team_change(&mut self, _api: &mut ServerApi, _cid: i32, _player: String, _team: i32) {}
    /// Вызывается при смене ника игрока.
    fn on_rename(&mut self, _api: &mut ServerApi, _old: String, _new: String) {}
    /// Вызывается при сообщении в чат; верните EventAction::Cancel, чтобы заблокировать дальнейшую обработку.
    fn on_chat(&mut self, _api: &mut ServerApi, _team: i32, _player: String, _message: String) -> EventAction {
        EventAction::Pass
    }
    /// Вызывается при обнаружении рукопожатия BestClient.
    fn on_best_client(&mut self, _api: &mut ServerApi, _cid: i32, _version: String) {}
    /// Вызывается при входе игрока в аккаунт.
    fn on_auth(&mut self, _api: &mut ServerApi, _cid: i32, _player: String, _account: String) {}
    /// Вызывается при назначении или обновлении роли аккаунта.
    fn on_role(&mut self, _api: &mut ServerApi, _account: String, _role: String) {}
    /// Вызывается один раз в секунду.
    fn on_tick(&mut self, _api: &mut ServerApi, _uptime_sec: u64) {}
    /// Вызывается каждые 100 мс, если включён быстрый тик.
    fn on_fast_tick(&mut self, _api: &mut ServerApi, _uptime_ms: u64) {}
    /// Вызывается при срабатывании запланированного таймера.
    fn on_scheduled(&mut self, _api: &mut ServerApi, _callback_id: u64) {}
    /// Вызывается при получении межплагинного IPC-сообщения.
    fn on_ipc(&mut self, _api: &mut ServerApi, _from: String, _payload: String) {}
    /// Вызывается при получении значения по ключу из БД.
    fn on_db_value(&mut self, _api: &mut ServerApi, _req_id: String, _value: String) {}
    /// Вызывается при завершении поиска по префиксу в БД.
    fn on_db_list(&mut self, _api: &mut ServerApi, _req_id: String, _items: Vec<(String, String)>) {}
    /// Вызывается при получении снимка ввода игрока.
    fn on_input(&mut self, _api: &mut ServerApi, _cid: i32, _dir: i32, _jump: i32, _hook: i32, _fire: i32, _target_x: i32, _target_y: i32) {}
    /// Вызывается при устаревшем обновлении списка онлайн-игроков.
    fn on_online(&mut self, _api: &mut ServerApi, _players: Vec<String>) {}
    /// Вызывается при устаревшем событии результата БД.
    fn on_db_result(&mut self, _api: &mut ServerApi, _req_id: String, _value: String) {}
    /// Вызывается при устаревшем детальном снимке состояния игрока.
    fn on_player_state(&mut self, _api: &mut ServerApi, _cid: i32, _player: String, _x: f32, _y: f32, _vx: f32, _vy: f32, _frozen: i32, _weapon: i32) {}
    /// Вызывается при обновлении снимка состояния игрока.
    fn on_state(&mut self, _api: &mut ServerApi, _cid: i32, _pos: PlayerPos) {}
    /// Вызывается при убийстве одного игрока другим.
    fn on_kill(&mut self, _api: &mut ServerApi, _killer: String, _victim: String, _weapon: i32) {}
    /// Вызывается при смерти игрока.
    fn on_death(&mut self, _api: &mut ServerApi, _cid: i32, _killer: i32, _weapon: i32) {}
    /// Вызывается при получении урона игроком.
    fn on_damage(&mut self, _api: &mut ServerApi, _cid: i32, _attacker: i32, _amount: i32, _weapon: i32) {}
    /// Вызывается при финише игрока на карте/гонке.
    fn on_finish(&mut self, _api: &mut ServerApi, _cid: i32, _time: f32) {}
    /// Вызывается при касании игроком специального тайла.
    fn on_tile(&mut self, _api: &mut ServerApi, _cid: i32, _tile: i32, _x: i32, _y: i32) {}
    /// Вызывается при попадании оружия в игрока.
    fn on_weapon_hit(&mut self, _api: &mut ServerApi, _attacker: i32, _victim: i32, _weapon: i32) {}
    /// Вызывается при ударе молотом по игроку.
    fn on_hammer_hit(&mut self, _api: &mut ServerApi, _attacker: String, _victim: String) {}
    /// Вызывается при подборе предмета игроком.
    fn on_pickup(&mut self, _api: &mut ServerApi, _cid: i32, _kind: i32) {}
    /// Вызывается при переключении активного оружия игроком.
    fn on_weapon_change(&mut self, _api: &mut ServerApi, _cid: i32, _old: i32, _new: i32) {}
    /// Вызывается при зацепе крюка игрока за тайл или другого игрока.
    fn on_hook_attach(&mut self, _api: &mut ServerApi, _cid: i32, _hooked_cid: Option<i32>, _pos: crate::dx::Vec2) {}
    /// Вызывается при отцеплении крюка игрока.
    fn on_hook_detach(&mut self, _api: &mut ServerApi, _cid: i32, _prev_hooked_cid: Option<i32>) {}
    /// Перенаправляет сырое событие Event в соответствующий типизированный метод обработчика.
    fn dispatch(&mut self, api: &mut ServerApi, ev: Event) -> EventAction {
        match ev {
            Event::Join { player } => self.on_join(api, player),
            Event::Leave { player } => self.on_leave(api, player),
            Event::TeamChange { cid, player, team } => self.on_team_change(api, cid, player, team),
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
    /// Запускает цикл событий плагина с использованием реализации EventHandler.
    pub fn run_handler<H: EventHandler + 'static>(self, mut handler: H) {
        self.run_intercept(move |api, ev| handler.dispatch(api, ev));
    }
}
