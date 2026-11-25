use ddnet_sdk::{Event, EventAction, ServerApi};
use std::collections::{HashMap, HashSet};

fn find_target(api: &ServerApi, query: &str) -> Option<(i32, String)> {
    let q = query.trim();
    if q.is_empty() { return None; }
    if let Ok(id) = q.parse::<i32>() {
        for (name, &cid) in &api.player_cids {
            if cid == id { return Some((cid, name.clone())); }
        }
    }
    let ql = q.to_lowercase();
    for (name, &cid) in &api.player_cids {
        if name.to_lowercase() == ql { return Some((cid, name.clone())); }
    }
    for (name, &cid) in &api.player_cids {
        if name.to_lowercase().contains(&ql) { return Some((cid, name.clone())); }
    }
    None
}

fn nearest_player(api: &ServerApi, skip_cid: i32, px: f32, py: f32, max_dist: f32) -> Option<(i32, f32, f32)> {
    let mut best = None;
    let mut best_d2 = max_dist * max_dist;
    for (&tcid, tpos) in &api.states {
        if tcid == skip_cid { continue; }
        let d2 = (tpos.x - px).powi(2) + (tpos.y - py).powi(2);
        if d2 < best_d2 { best_d2 = d2; best = Some((tcid, tpos.x, tpos.y)); }
    }
    best
}

fn ray_hit_player(api: &ServerApi, skip_cid: i32, x1: f32, y1: f32, dx: f32, dy: f32, max_len: f32) -> Option<(i32, f32, f32)> {
    let mut best = None;
    let mut best_t = max_len;
    for (&tcid, tpos) in &api.states {
        if tcid == skip_cid { continue; }
        let vx = tpos.x - x1;
        let vy = tpos.y - y1;
        let proj = vx * dx + vy * dy;
        if proj < 0.0 || proj > best_t { continue; }
        let perp2 = (vx - dx * proj).powi(2) + (vy - dy * proj).powi(2);
        if perp2 <= 56.0 * 56.0 {
            best_t = proj;
            best = Some((tcid, x1 + dx * proj, y1 + dy * proj));
        }
    }
    best
}

fn give_super_kit(api: &ServerApi, cid: i32) {
    for w in (0..=4).rev() {
        api.give_weapon(cid, w);
        api.set_ammo(cid, w, if w == 0 { 10 } else { 0 });
    }
}

fn toggle_super(api: &mut ServerApi, cid: i32, player: &str, supers: &mut HashSet<i32>, active_wpn: &mut HashMap<i32, i32>) {
    if supers.remove(&cid) {
        active_wpn.remove(&cid);
        api.set_invincible(cid, false); api.set_endless_jump(cid, false);
        api.set_endless_hook(cid, false); api.set_phys(cid, "jetpack", false);
        api.set_phys(cid, "hammer_hit", true);
        // Забираем выданные супер-оружки (слоты 1..=4), чтобы не оставались пустыми
        for w in 1..=4 {
            api.set_ammo(cid, w, if w == 1 { 10 } else { -1 });
        }
        api.set_color(cid, false, 0, 0); api.play_sound(cid, 30);
        api.whisper(player, "[SUPER] Режим бога ВЫКЛЮЧЕН.");
    } else {
        supers.insert(cid);
        api.set_invincible(cid, true); api.set_endless_jump(cid, true);
        api.set_endless_hook(cid, true); api.set_phys(cid, "jetpack", true);
        api.set_phys(cid, "hammer_hit", false);
        api.set_freeze(cid, 0); give_super_kit(api, cid);
        api.set_color(cid, true, 2556864, 2556864); api.play_sound(cid, 28);
        api.whisper(player, "[SUPER] ВКЛ! 1:Полёт 2:ТП 3:Притяг 4:Мега-взрыв 5:Фриз-луч");
    }
    api.track_positions(!supers.is_empty());
}

fn main() {
    let mut api = ServerApi::new();
    api.register_admin_plugin("super", "Режим бога: !super, !tp, !bring, !vanish");
    let (mut supers, mut vanishes, mut fire_held, mut hook_held, mut active_wpn, mut fly_tick) =
        (HashSet::new(), HashSet::new(), HashSet::new(), HashSet::new(), HashMap::new(), 0u32);
    api.run_intercept(|api, ev| { match ev {
        Event::Chat { player, message, .. } => {
            if !api.is_staff(&player) { return EventAction::Continue; }
            let Some(&cid) = api.player_cids.get(&player) else { return EventAction::Continue };
            let mut parts = message.trim().splitn(2, " ");
            let (cmd, arg) = (parts.next().unwrap_or(""), parts.next().unwrap_or("").trim());
            match cmd {
                "!super" | "!god" => toggle_super(api, cid, &player, &mut supers, &mut active_wpn),
                "!vanish" => {
                    let st = !vanishes.remove(&cid); if st { vanishes.insert(cid); }
                    api.set_phys(cid, "collision", !st); api.set_phys(cid, "hammer_hit", !st); api.set_phys(cid, "hook_hit", !st); api.play_sound(cid, if st { 28 } else { 30 });
                    api.whisper(&player, if st { "[SUPER] Vanish ВКЛ." } else { "[SUPER] Vanish ВЫКЛ." });
                }
                "!tp" => if let Some((tid, tn)) = find_target(api, arg) {
                    if let Some(p) = api.states.get(&tid).copied() {
                        api.set_pos(cid, p.x, p.y - 32.0); api.set_vel(cid, 0.0, 0.0);
                        api.play_sound(cid, 29); api.whisper(&player, &format!("[SUPER] ТП к {tn}."));
                    } else { api.request_state(tid); }
                },
                "!bring" => if let Some((tid, tn)) = find_target(api, arg) {
                    if let Some(p) = api.states.get(&cid).copied() {
                        api.set_pos(tid, p.x, p.y - 32.0); api.set_vel(tid, 0.0, 0.0);
                        api.play_sound(tid, 29); api.whisper(&player, &format!("[SUPER] {tn} притянут."));
                    } else { api.request_state(cid); }
                },
                _ => {}
            }
        }
        Event::Tick { .. } => if !supers.is_empty() { api.track_positions(true); },
        Event::WeaponChange { cid, new, .. } => {
            active_wpn.insert(cid, new);
            if let Some(s) = api.states.get_mut(&cid) { s.weapon = new; }
        }
        Event::PlayerState { cid, frozen, weapon, .. } => if supers.contains(&cid) {
            if frozen != 0 { api.set_freeze(cid, 0); }
            if weapon >= 0 { active_wpn.insert(cid, weapon); } else { give_super_kit(api, cid); }
        },
        Event::Input { cid, fire, hook, target_x, target_y, .. } => {
            if fire == 0 { fire_held.remove(&cid); }
            if hook == 0 { hook_held.remove(&cid); }
            if !supers.contains(&cid) { return EventAction::Continue; }
            let Some(p) = api.states.get(&cid).copied() else { return EventAction::Continue };
            if p.frozen != 0 { api.set_freeze(cid, 0); }
            let l = ((target_x * target_x + target_y * target_y) as f32).sqrt().max(1.0);
            let (dx, dy) = (target_x as f32 / l, target_y as f32 / l);
            let (ax, ay) = (p.x + target_x as f32, p.y + target_y as f32);
            let wpn = active_wpn.get(&cid).copied().unwrap_or(p.weapon);
            if wpn == 4 && hook != 0 && hook_held.insert(cid) {
                let (mut bx, mut by) = (p.x + dx * 850.0, p.y + dy * 850.0);
                if let Some((tid, hx, hy)) = ray_hit_player(api, cid, p.x, p.y, dx, dy, 850.0) {
                    bx = hx; by = hy;
                    api.set_freeze(tid, 0);
                    api.play_sound(cid, 30);
                }
                api.spawn_laser_ex(p.x, p.y, bx, by, 0);
            }
            if fire == 0 { return EventAction::Continue; }
            if wpn == 0 {
                let first_click = fire_held.insert(cid);
                fly_tick = fly_tick.wrapping_add(1);
                if first_click || fly_tick % 2 == 0 {
                    api.set_vel(cid, dx * 22.0, dy * 22.0);
                }
            } else {
                if fire_held.insert(cid) {
                    match wpn {
                        1 => { api.set_pos(cid, ax, ay); api.set_vel(cid, 0.0, 0.0); api.play_sound(cid, 29); }
                        2 => if let Some((tid, tx, ty)) = nearest_player(api, cid, ax, ay, 280.0) {
                            api.spawn_laser_ex(p.x, p.y, tx, ty, 0);
                            api.set_pos(tid, p.x, p.y - 32.0); api.set_vel(tid, 0.0, 0.0); api.play_sound(cid, 21);
                        },
                        3 => {
                            api.spawn_fx("explosion", ax, ay);
                            api.spawn_fx("explosion", ax - 48.0, ay);
                            api.spawn_fx("explosion", ax + 48.0, ay);
                            api.spawn_fx("explosion", ax, ay - 48.0);
                            api.spawn_fx("explosion", ax, ay + 48.0);
                            api.play_sound(cid, 6);
                            for (&tcid, tpos) in &api.states {
                                if tcid == cid { continue; }
                                let bx = tpos.x - ax;
                                let by = tpos.y - ay;
                                let dist = (bx * bx + by * by).sqrt();
                                if dist < 380.0 {
                                    let scale = (1.0 - dist / 380.0).max(0.25);
                                    let (nx, ny) = if dist > 1.0 { (bx / dist, by / dist) } else { (0.0, -1.0) };
                                    api.set_vel(tcid, nx * 44.0 * scale, ny * 44.0 * scale - 10.0 * scale);
                                }
                            }
                        }
                        4 => {
                            let (mut bx, mut by) = (p.x + dx * 850.0, p.y + dy * 850.0);
                            if let Some((tid, hx, hy)) = ray_hit_player(api, cid, p.x, p.y, dx, dy, 850.0) {
                                bx = hx; by = hy;
                                api.set_freeze(tid, 5);
                                api.play_sound(cid, 38);
                            } else {
                                api.play_sound(cid, 12);
                            }
                            api.spawn_laser_ex(p.x, p.y, bx, by, 4);
                        }
                        _ => {}
                    }
                }
                return EventAction::Cancel;
            }
        }
        // Сбрасываем активный супер-режим и невидимость при смене карты
        Event::Ipc { from, payload } if from == "manager" && payload.starts_with("MAP_CHANGE|") => {
            supers.clear();
            vanishes.clear();
            fire_held.clear();
            hook_held.clear();
            active_wpn.clear();
            api.track_positions(false);
        }
        Event::Leave { .. } => {
            api.states.retain(|&c, _| api.player_cids.values().any(|&v| v == c));
            supers.retain(|&c| api.player_cids.values().any(|&v| v == c));
            vanishes.retain(|&c| api.player_cids.values().any(|&v| v == c));
            fire_held.retain(|&c| api.player_cids.values().any(|&v| v == c));
            active_wpn.retain(|&c, _| api.player_cids.values().any(|&v| v == c));
            api.track_positions(!supers.is_empty());
        }
        _ => {}
    } EventAction::Continue });
}
