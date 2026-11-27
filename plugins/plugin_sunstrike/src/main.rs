use ddnet_sdk::{Event, ServerApi};
use std::collections::{HashMap, HashSet};
use std::f32::consts::TAU;

struct Strike { x: f32, y: f32, target_cid: Option<i32>, start_ms: u64, owner_cid: i32, last_beep: u64, last_draw: u64 }

/// Проверяет доступ к !sunstrike по авторизованному аккаунту, роли стаффа или build_admins.txt.
fn can_use_sunstrike_with_list(api: &ServerApi, player: &str, admins_txt: &str) -> bool {
    let acc = match api.authed_accounts.get(player) {
        Some(a) if !a.is_empty() => a.as_str(),
        _ => return false,
    };
    if api.is_staff(player) {
        return true;
    }
    admins_txt.lines().map(str::trim).filter(|l| !l.is_empty()).any(|l| l.eq_ignore_ascii_case(acc))
}

fn can_use_sunstrike(api: &ServerApi, player: &str) -> bool {
    let txt = std::fs::read_to_string(ddnet_sdk::resolve_server_path("build_admins.txt")).unwrap_or_default();
    can_use_sunstrike_with_list(api, player, &txt)
}

fn main() {
    let mut api = ServerApi::new();
    api.register_plugin("sunstrike", "!sunstrike — орбитальный удар с самонаведением (Admin/Owner)");
    api.register_cmd("sunstrike", "", "Орбитальный солнечный удар с самонаведением (Admin/Owner)");

    let mut armed: HashSet<i32>= HashSet::new();
    let mut last_fire: HashMap<i32, i32>= HashMap::new();
    let mut strikes: Vec<Strike>= Vec::new();
    let mut uptime_ms: u64 = 0;

    api.run(move |api, ev| {
        match ev {
            Event::Chat { player, message, .. } => {
                if message.trim() == "!sunstrike" {
                    let cid = match api.player_cids.get(&player).or_else(|| api.cids.get(&player)).copied() { Some(c) => c, None => return }; if !can_use_sunstrike(api, &player) { api.broadcast_to(cid, "Только для Admin/Owner."); return; }
                    {
                        if armed.remove(&cid) { api.broadcast_to(cid, "☀ Режим SUNSTRIKE выключен."); }
                        else { armed.insert(cid); api.broadcast_to(cid, "☀ Режим SUNSTRIKE включен! Стреляй для удара."); }
                        let act = !armed.is_empty() || !strikes.is_empty();
                        api.track_positions(act);
                        api.enable_fast_tick(act);
                    }
                    return;
                }
            }
            Event::Ipc { from, payload } if from == "manager" && payload.starts_with("MAP_CHANGE|") => {
                armed.clear(); strikes.clear(); last_fire.clear();
                api.track_positions(false); api.enable_fast_tick(false);
            }
            Event::Leave { player } => {
                if let Some(cid) = api.last_leave_cid.or_else(|| api.player_cids.get(&player).or_else(|| api.cids.get(&player)).copied()) { armed.remove(&cid); last_fire.remove(&cid); }
                let act = !armed.is_empty() || !strikes.is_empty();
                api.track_positions(act);
                api.enable_fast_tick(act);
            }
            Event::Tick { .. } => {
                if !armed.is_empty() || !strikes.is_empty() { api.track_positions(true); }
            }
            Event::Input { cid, fire, target_x, target_y, .. } => {
                let prev = last_fire.insert(cid, fire & 1).unwrap_or(0);
                if (fire & 1) != 0 && prev == 0 && armed.contains(&cid) {
                    if let Some(me) = api.states.get(&cid) {
                        let (sx, sy) = (me.x + target_x as f32, me.y + target_y as f32);
                        let mut locked: Option<i32>= None;
                        let mut best_d = 145.0f32;
                        for (&ocid, ost) in &api.states {
                            if ocid != cid {
                                let d = (ost.x - sx).hypot(ost.y - sy);
                                if d < best_d { best_d = d; locked = Some(ocid); }
                            }
                        }
                        if let Some(tcid) = locked {
                            api.broadcast_to(tcid, "⚠ НА ТЕБЯ НАВЕДЁН SUNSTRIKE! ⚠");
                            api.play_sound(tcid, 38);
                        }
                        strikes.push(Strike { x: sx, y: sy, target_cid: locked, start_ms: uptime_ms, owner_cid: cid, last_beep: 0, last_draw: 0 });
                        api.spawn_fx("damage", sx, sy);
                        api.play_sound(-1, 38);
                    }
                }
            }
            Event::FastTick { uptime_ms: ms } => {
                uptime_ms = ms;
                let mut i = 0;
                while i < strikes.len() {
                    if strikes[i].target_cid.is_none() {
                        for (&ocid, ost) in &api.states {
                            if ocid != strikes[i].owner_cid && (ost.x - strikes[i].x).hypot(ost.y - strikes[i].y) < 110.0 {
                                strikes[i].target_cid = Some(ocid);
                                api.broadcast_to(ocid, "⚠ НА ТЕБЯ НАВЕДЁН SUNSTRIKE! ⚠");
                                api.play_sound(ocid, 38);
                                break;
                            }
                        }
                    }
                    if let Some(tcid) = strikes[i].target_cid {
                        if let Some(tst) = api.states.get(&tcid) { strikes[i].x = tst.x; strikes[i].y = tst.y; }
                    }
                    let elapsed = ms.saturating_sub(strikes[i].start_ms);
                    let (sx, sy, owner, is_locked) = (strikes[i].x, strikes[i].y, strikes[i].owner_cid, strikes[i].target_cid.is_some());
                    if elapsed < 1500 {
                        let prog = elapsed as f32 / 1500.0;
                        let r_out = 72.0 * (1.0 - prog * 0.72) + 12.0;
                        let r_in = r_out * 0.38;
                        let cy = if is_locked { sy } else { sy - 26.0 };
                        let base_ang = prog * TAU * 1.4 - TAU / 4.0;
                        let mut pts = [(0.0f32, 0.0f32); 10];
                        for k in 0..10 {
                            let a = base_ang + (k as f32) * TAU / 10.0;
                            let r = if k % 2 == 0 { r_out } else { r_in };
                            pts[k] = (sx + a.cos() * r, cy + a.sin() * r);
                        }
                        if ms.saturating_sub(strikes[i].last_draw) >= 100 {
                            strikes[i].last_draw = ms;
                            for k in 0..10 {
                                let (x1, y1) = pts[k];
                            let (x2, y2) = pts[(k + 1) % 10];
                                api.spawn_laser(x1, y1, x2, y2);
                            }
                        }
                        if ms.saturating_sub(strikes[i].last_beep) >= 280 {
                            strikes[i].last_beep = ms;
                            let idx = ((elapsed / 280) as usize) % 5;
                            api.spawn_fx("damage", pts[idx * 2].0, pts[idx * 2].1);
                            if is_locked { api.spawn_fx("damage", sx, sy - 42.0); }
                            api.play_sound(-1, 37);
                        }
                        i += 1;
                    } else {
                        for dx in [-40.0f32, -20.0, 0.0, 20.0, 40.0] { api.spawn_laser(sx + dx, sy - 750.0, sx + dx, sy); }
                        for k in 0..6 {
                            let a = (k as f32) * TAU / 6.0;
                            api.spawn_laser(sx, sy, sx + a.cos() * 140.0, sy + a.sin() * 140.0);
                            api.spawn_fx("damage", sx + a.cos() * 75.0, sy + a.sin() * 75.0);
                        }
                        for (dx, dy) in [(-75.0f32, -10.0), (-35.0, -40.0), (0.0, -55.0), (35.0, -40.0), (75.0, -10.0)] {
                            api.spawn_fx("explosion", sx + dx, sy + dy);
                        }
                        api.spawn_fx("hammer", sx, sy);
                        api.spawn_fx("hammer", sx - 35.0, sy - 30.0);
                        api.spawn_fx("hammer", sx + 35.0, sy - 30.0);
                        api.play_sound(-1, 6);
                        api.play_sound(-1, 4);
                        for (&vcid, vst) in &api.states {
                            if vcid != owner && (vst.x - sx).hypot(vst.y - sy) <= 135.0 {
                                let dir = if (vst.x - sx).abs() < 1.0 { 1.0 } else { (vst.x - sx).signum() };
                                api.set_vel(vcid, dir * 18.0, -22.0);
                                api.kill_by(vcid, owner, 4);
                                
                            }
                        }
                        if let Some(tcid) = strikes[i].target_cid { api.broadcast_to(tcid, "\u{200B}"); }
                        strikes.remove(i);
                        let act = !armed.is_empty() || !strikes.is_empty();
                        api.track_positions(act);
                        api.enable_fast_tick(act);
                    }
                }
            }
            _ => {}
        }
        
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_sunstrike_permissions_and_spoofing() {
        let mut api = ServerApi::new();
        let admins = "BuilderAcc\n";
        assert!(!can_use_sunstrike_with_list(&api, "BuilderAcc_fake", admins));
        assert!(!can_use_sunstrike_with_list(&api, "BuilderAcc", admins));
        api.authed_accounts.insert("Nick1".into(), "BuilderAcc".into());
        assert!(can_use_sunstrike_with_list(&api, "Nick1", admins));
    }
}
