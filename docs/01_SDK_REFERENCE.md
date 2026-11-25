# Справочник `ddnet_sdk` v11.7.0 LTS (Rust Edition 2024)

## 1. Минимальный шаблон плагина
```rust
use ddnet_sdk::prelude::*;

ddnet_sdk::plugin! {
    name: "my_plugin",
    desc: "Описание режима",
    admin: false,
    priority: Priority::Normal,
    cmds: [
        ("!hello", "", "Приветствие игрока"),
    ],
    state: {
        counter: u64 = 0,
    },
    handle(api, ev) {
        match ev {
            Event::Chat { player, message, .. } if message.trim() == "!hello" => {
                *counter += 1;
                api.whisper(&player, &format!("Привет, {}! Вызов #{}", player, *counter));
            }
            _ => {}
        }
    }
}
```

## 2. Основные события `Event`
- **Сессия и чат:** `Join { player }`, `Leave { player }`, `Rename { old_name, new_name }`, `Chat { team, player, message }`, `Auth { cid, player, account }`, `Role { account, role }`, `BestClient { cid, version }`
- **Тики и таймеры:** `Tick { uptime_sec }`, `FastTick { uptime_ms }` (включать только на время активности через `api.enable_fast_tick(true)`), `Scheduled { callback_id }`
- **Позиции и ввод:** `api.track_positions(true)` -> координаты в `api.states: HashMap<i32, PlayerPos>` (`x, y, vx, vy, alive`); `Input { cid, fire, jump, hook, dir, target_x, target_y }`
- **Бой и карта:** `Kill`, `HammerHit`, `WeaponHit`, `Damage`, `Death`, `Pickup`, `WeaponChange`, `Finish`, `Tile`, `TileCheck`, `HookAttach { cid, pos, hooked_cid }`, `HookDetach { cid, prev_hooked_cid }`
- **IPC и БД:** `Ipc { from, payload }`, `DbValue { req_id, value }`, `DbList { req_id, items }`

## 3. Ключевые методы `ServerApi`
- **Чат и HUD:** `say`, `whisper`, `whisper_id`, `broadcast_to`, `server_alert`, `mod_alert`, `set_hud_line(PlayerId, 0..4, text)`, `clear_hud_line`
- **Физика и оружие:** `set_pos`, `set_vel`, `apply_impulse`, `set_freeze`, `set_invincible`, `set_endless_jump`, `set_endless_hook`, `set_solo`, `set_phys`, `give_weapon`, `set_ammo`, `remove_weapon`, `set_weapon_lock`, `force_kill`, `kill_by`, `reset_hook`, `play_sound`
- **Скины и цвета:** `set_skin`, `set_skin_to(viewer, target, skin)`, `clear_skin_to`, `set_clan`, `set_color`, `set_vip_plus`
- **Визуалы `OnSnap` (0 FX/сек, изолированы по плагинам):**
  - `spawn_visual_laser` / `remove_visual_laser` / `clear_visual_lasers` (пул `0..127`)
  - `spawn_visual_pickup` / `spawn_visual_pickup_follow` / `remove_visual_pickup` (пул `0..127`)
  - `spawn_visual_tee` / `remove_visual_tee` / `clear_visual_tees` (пул `0..63`)
  - `spawn_visual_text` / `remove_visual_text` / `clear_visual_texts` (пул `0..31`, 3x5 шрифт до 24 символов)
- **Временные FX (`110 FX/сек`):** `spawn_fx`, `spawn_laser`, `spawn_laser_ex`, `spawn_projectile`, `despawn_projectile`, `send_progress_bar`
- **БД `plugins_kv.db`:** `db_set`, `db_add`, `db_get`, `db_prefix`, `db_del`, `db_del_prefix`, `db_add_by_account`, `db_transfer_by_account`

### Режимы игры, скорборд и правила боя (`set_gamemode`, `set_freeze_autokill`, `set_laser_freeze`)
* `api.set_gamemode(mode)` — переключает режим таблицы очков (`0` = Race/Time, `1` = Points FFA, `2` = Team Points Red/Blue, `3` = FNG Team Points).
* `api.set_player_score(cid, score)` / `api.set_player_score_name(name, score)` — задаёт целые очки игрока в Tab (работает по `cid` или по нику).
* `api.set_player_team(cid, team)` / `api.set_team_score(red, blue)` — переводит игрока в команду (`0`=Red, `1`=Blue, `-1`=Spec) и задаёт командный счёт.
* `api.set_freeze_autokill(sec)` — включает автокилл в тайле фриза (`TILE_FREEZE`) через `sec` секунд (`0` = выкл) с персональным звуковым и визуальным отсчётом.
* `api.set_laser_freeze(sec)` — лазер замораживает врагов на `sec` секунд без моргания предсказания (`m_FreezeEnd = -1`), со звуком и партиклами при разморозке (`0` = выкл).
* `api.set_hammer_team_unfreeze(on)` — молот размораживает только тиммейтов и отбрасывает замороженных врагов.
* `api.set_fng_weapons(on)` — выдача при спавне и всем живым игрокам строго молота и лазера (без пистолета).
