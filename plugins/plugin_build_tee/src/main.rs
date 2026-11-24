use ddnet_sdk::{Event, ServerApi};
use std::collections::{HashMap, VecDeque};
use std::fs;

const ADMINS_PATH: &str = "build_admins.txt";
const MAX_BLOCKS: usize = 64;

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

fn spawn_block_tee(api: &ServerApi, slot: usize, cx: i32, cy: i32, tile: i32) {
    let x = cx as f32 * 32.0 + 16.0;
    let y = cy as f32 * 32.0 + 16.0;
    let (body, feet, freeze) = match tile {
        3 => (2031680, 2031680, 0),
        9 => (8454208, 8454208, 1),
        _ => (65536, 65536, 0),
    };
    api.spawn_visual_tee(slot as i32, x, y, "default", body, feet, freeze);
}

fn main() {
    let mut api = ServerApi::new();
    api.register_admin_plugin("build_tee", "!build [1|3|9|0|off|clear] — строительство блоками-тиишками");
    api.register_cmd("build", "[1|3|9|0|off|clear]", "1=стена, 3=нехук, 9=фриз, 0=сломать, clear=сброс, off=выкл");

    let mut builders: HashMap<i32, i32> = HashMap::new();
    let mut blocks: HashMap<(i32, i32), (usize, i32)> = HashMap::new();
    let mut free_slots: Vec<usize> = (0..MAX_BLOCKS).rev().collect();
    let mut order: VecDeque<(i32, i32)> = VecDeque::new();

    api.run(move |api, ev| match ev {
        Event::Tick { .. } => {
            if !builders.is_empty() {
                api.track_positions(true);
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
                    api.broadcast_to(cid, "[Build] Строитель выключен.");
                    return;
                }
                if arg == "clear" {
                    for &(x, y) in blocks.keys() {
                        api.raw(&format!("bc_set_tile {} {} 0", x, y));
                    }
                    api.clear_visual_tees();
                    blocks.clear();
                    order.clear();
                    free_slots = (0..MAX_BLOCKS).rev().collect();
                    api.broadcast_to(cid, "[Build] Все блоки-тиишки очищены.");
                    return;
                }
                let tile: i32 = match arg { "0" => 0, "3" => 3, "9" => 9, _ => 1 };
                builders.insert(cid, tile);
                api.track_positions(true);
                api.broadcast_to(cid, &format!("[Build] Режим {} активен! Стреляй ЛКМ.", tile));
            }
        }
        Event::Input { cid, fire, target_x, target_y, .. } => {
            let tile = match builders.get(&cid).copied() {
                Some(t) if (fire & 1) != 0 => t,
                _ => return,
            };
            if let Some(me) = api.states.get(&cid) {
                let cx = ((me.x + target_x as f32) / 32.0).floor() as i32;
                let cy = ((me.y + target_y as f32) / 32.0).floor() as i32;
                if tile == 0 {
                    if let Some((slot, _)) = blocks.remove(&(cx, cy)) {
                        order.retain(|&p| p != (cx, cy));
                        free_slots.push(slot);
                        api.raw(&format!("bc_set_tile {} {} 0", cx, cy));
                        api.remove_visual_tee(slot as i32);
                        api.play_sound(cid, 4);
                    }
                    return;
                }
                if let Some(&(slot, old_tile)) = blocks.get(&(cx, cy)) {
                    if old_tile != tile {
                        blocks.insert((cx, cy), (slot, tile));
                        api.raw(&format!("bc_set_tile {} {} {}", cx, cy, tile));
                        spawn_block_tee(api, slot, cx, cy, tile);
                        api.play_sound(cid, 24);
                    }
                    return;
                }
                let slot = match free_slots.pop() {
                    Some(s) => s,
                    None => match order.pop_front().and_then(|o| blocks.remove(&o).map(|(s, _)| (o, s))) {
                        Some((o, s)) => { api.raw(&format!("bc_set_tile {} {} 0", o.0, o.1)); s }
                        None => return,
                    },
                };
                blocks.insert((cx, cy), (slot, tile));
                order.push_back((cx, cy));
                api.raw(&format!("bc_set_tile {} {} {}", cx, cy, tile));
                spawn_block_tee(api, slot, cx, cy, tile);
                api.play_sound(cid, 24);
            }
        }
        _ => {}
    });
}
