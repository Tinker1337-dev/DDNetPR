use ddnet_sdk::{Event, ServerApi};
use std::collections::HashMap;
use std::fs;

const ADMINS_PATH: &str = "build_admins.txt";

fn can_build(api: &ServerApi, player: &str) -> bool {
    let acc = match api.authed_accounts.get(player) {
        Some(a) if !a.is_empty() => a.as_str(),
        _ => return false,
    };
    if api.is_staff(player) {
        return true;
    }
    if fs::metadata(ADMINS_PATH).is_err() {
        let _ = fs::write(ADMINS_PATH, "Tinker1337\n");
    }
    fs::read_to_string(ADMINS_PATH)
        .map(|s| s.lines().any(|l| l.trim().eq_ignore_ascii_case(acc)))
        .unwrap_or(false)
}

fn main() {
    let mut api = ServerApi::new();
    api.register_admin_plugin("build", "!build [1|3|9|0|off|clear] — фан-строительство (по логину)");
    api.register_cmd("build", "[tile|off|clear]", "1=стена, 3=нехук, 9=фриз, 0=сломать, clear=сброс, off=выкл");

    let mut builders: HashMap<i32, i32> = HashMap::new();
    let mut modified_tiles: HashMap<(i32, i32), i32> = HashMap::new();

    api.run(move |api, ev| match ev {
        Event::Tick { .. } => {
            if !builders.is_empty() {
                api.track_positions(true);
            }
        }
        Event::BestClient { cid, version } => {
            api.send_custom_packet(cid, "build_ui", &format!("hello:{}", version));
            for (&(x, y), &tile) in &modified_tiles {
                api.send_custom_packet(cid, "set_tile", &format!("{}:{}:{}", x, y, tile));
            }
        }
        Event::Leave { .. } => {
            if let Some(cid) = api.last_leave_cid {
                builders.remove(&cid);
            }
            if !api.names.is_empty() {
                builders.retain(|k, _| api.names.contains_key(k));
            }
            api.track_positions(!builders.is_empty());
        }
        Event::Chat { player, message, .. } => {
            let msg = message.trim();
            if msg == "!build" || msg.starts_with("!build ") {
                let cid = api.player_cids.get(&player).or_else(|| api.cids.get(&player)).copied().unwrap_or(0);
                if !can_build(api, &player) {
                    api.broadcast_to(cid, "[Build] Войдите в аккаунт (/login) с правами строителя.");
                    return;
                }
                let arg = msg.split_whitespace().nth(1).unwrap_or("1");
                if arg == "off" {
                    builders.remove(&cid);
                    api.track_positions(!builders.is_empty());
                    api.send_custom_packet(cid, "build_ui", "off");
                    api.broadcast_to(cid, "[Build] Строитель выключен.");
                    return;
                }
                if arg == "clear" {
                    for &(x, y) in modified_tiles.keys() {
                        api.raw(&format!("bc_set_tile {} {} 0", x, y));
                    }
                    modified_tiles.clear();
                    api.broadcast_to(cid, "[Build] Все временные блоки очищены.");
                    return;
                }
                let tile: i32 = arg.parse().unwrap_or(1).clamp(0, 255);
                builders.insert(cid, tile);
                api.track_positions(true);
                api.send_custom_packet(cid, "build_ui", &format!("on:tile={}", tile));
                api.broadcast_to(cid, &format!("[Build] Тайл {} активен! Стреляй ЛКМ чтобы строить.", tile));
            }
        }
        Event::Input { cid, fire, target_x, target_y, .. } => {
            let tile = match builders.get(&cid).copied() {
                Some(t) if fire != 0 => t,
                _ => return,
            };
            if let Some(me) = api.states.get(&cid) {
                let cx = ((me.x + target_x as f32) / 32.0).floor() as i32;
                let cy = ((me.y + target_y as f32) / 32.0).floor() as i32;
                modified_tiles.insert((cx, cy), tile);
                api.raw(&format!("bc_set_tile {} {} {}", cx, cy, tile));
                api.play_sound(cid, 24);
            }
        }
        _ => {}
    });
}
