use ddnet_sdk::prelude::*;
use std::collections::{HashMap, HashSet};

const T_SPAWN_X: f32 = 256.0;
const T_SPAWN_Y: f32 = 1152.0;
const CT_SPAWN_X: f32 = 4128.0;
const CT_SPAWN_Y: f32 = 1152.0;
const PLANT_A_X: f32 = 4160.0;
const PLANT_A_Y: f32 = 544.0;
const PLANT_B_X: f32 = 1280.0;
const PLANT_B_Y: f32 = 2016.0;
const PLANT_RADIUS: f32 = 150.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase { Warmup, FreezeTime, Live, RoundEnd }

#[derive(Clone, Copy, PartialEq, Eq)]
enum Primary { None, Rifle, Awp }

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pistol { Stock, Deagle }

#[derive(Clone)]
struct CsPlayer {
    team: i32,
    money: i32,
    hp: i32,
    armor: i32,
    alive: bool,
    primary: Primary,
    pistol: Pistol,
    has_he: bool,
    has_kit: bool,
    buy_slot: usize,
    fire_cd: u32,
}

impl Default for CsPlayer {
    fn default() -> Self {
        Self { team: 0, money: 800, hp: 100, armor: 0, alive: true, primary: Primary::None, pistol: Pistol::Stock, has_he: false, has_kit: false, buy_slot: 2, fire_cd: 0 }
    }
}

struct Grenade { owner: i32, x: f32, y: f32, vx: f32, vy: f32, ticks_left: u32 }

fn dist2(x1: f32, y1: f32, x2: f32, y2: f32) -> f32 { (x1 - x2).powi(2) + (y1 - y2).powi(2) }

fn raycast_player(api: &ServerApi, players: &HashMap<i32, CsPlayer>, shooter: i32, shooter_team: i32, x1: f32, y1: f32, dx: f32, dy: f32, max_dist: f32) -> Option<(i32, f32, f32)> {
    let mut best = None;
    let mut best_t = max_dist;
    for (&cid, p) in players {
        if cid == shooter || !p.alive || p.team < 0 || p.team == shooter_team { continue; }
        let Some(st) = api.states.get(&cid) else { continue };
        let (vx, vy) = (st.x - x1, st.y - y1);
        let proj = vx * dx + vy * dy;
        if proj < 0.0 || proj > best_t { continue; }
        if (vx - dx * proj).powi(2) + (vy - dy * proj).powi(2) <= 42.0 * 42.0 {
            best_t = proj;
            best = Some((cid, x1 + dx * proj, y1 + dy * proj));
        }
    }
    best
}

fn give_cs_loadout(api: &ServerApi, cid: i32, p: &CsPlayer) {
    api.give_weapon(cid, 0); api.set_ammo(cid, 0, -1);
    api.give_weapon(cid, 1); api.set_ammo(cid, 1, 12);
    if p.primary == Primary::Rifle { api.give_weapon(cid, 2); api.set_ammo(cid, 2, 30); } else { api.set_ammo(cid, 2, -1); }
    if p.has_he { api.give_weapon(cid, 3); api.set_ammo(cid, 3, 1); } else { api.set_ammo(cid, 3, -1); }
    if p.primary == Primary::Awp { api.give_weapon(cid, 4); api.set_ammo(cid, 4, 10); } else { api.set_ammo(cid, 4, -1); }
}

fn sync_hud_stats(api: &ServerApi, cid: i32, p: &CsPlayer) {
    let hearts = if p.alive { ((p.hp + 9) / 10).clamp(1, 10) } else { 0 };
    let shields = if p.alive { ((p.armor + 9) / 10).clamp(0, 10) } else { 0 };
    api.raw(&format!("plugin_set_stats {} {} {}", cid, hearts, shields));
}

fn buy_slot_info(slot: usize, team: i32) -> (&'static str, i32) {
    match slot {
        0 => if team == 1 { ("Kevlar + Kit", 650) } else { ("Броня Kevlar", 650) },
        1 => ("Desert Eagle", 700),
        2 => if team == 1 { ("M4A1 Карабин", 2700) } else { ("AK-47 Автомат", 2700) },
        3 => ("Граната HE", 300),
        _ => ("Снайперская AWP", 4750),
    }
}

fn try_buy_slot(api: &ServerApi, cid: i32, p: &mut CsPlayer, slot: usize) -> bool {
    let (name, cost) = buy_slot_info(slot, p.team);
    if p.money < cost { api.play_sound(cid, 30); return false; }
    let ok = match slot {
        0 => { if p.armor >= 100 && (p.team != 1 || p.has_kit) { return false; } p.armor = 100; if p.team == 1 { p.has_kit = true; } true }
        1 => { if p.pistol == Pistol::Deagle { return false; } p.pistol = Pistol::Deagle; true }
        2 => { if p.primary == Primary::Rifle { return false; } p.primary = Primary::Rifle; true }
        3 => { if p.has_he { return false; } p.has_he = true; true }
        _ => { if p.primary == Primary::Awp { return false; } p.primary = Primary::Awp; true }
    };
    if ok {
        p.money -= cost;
        give_cs_loadout(api, cid, p);
        sync_hud_stats(api, cid, p);
        api.play_sound(cid, 28);
        if let Some(pl) = api.names.get(&cid) { api.whisper(pl, &format!("[CS] Куплено: {} (-${}) | Остаток: ${}", name, cost, p.money)); }
    }
    ok
}

fn auto_buy(api: &ServerApi, cid: i32, p: &mut CsPlayer) {
    if p.money >= 5400 { try_buy_slot(api, cid, p, 0); try_buy_slot(api, cid, p, 4); try_buy_slot(api, cid, p, 3); }
    else if p.money >= 3350 { try_buy_slot(api, cid, p, 0); try_buy_slot(api, cid, p, 2); try_buy_slot(api, cid, p, 3); }
    else if p.money >= 2700 { try_buy_slot(api, cid, p, 2); }
    else if p.money >= 1350 { try_buy_slot(api, cid, p, 0); try_buy_slot(api, cid, p, 1); }
    else { try_buy_slot(api, cid, p, 1); }
}

ddnet_sdk::plugin! {
    name: "cs",
    desc: "Counter-Strike 2D (T vs CT, закуп в 1 клик, C4, AWP, дипфриз до конца раунда)",
    admin: false,
    priority: Priority::Normal,
    cmds: [("buy", "<kevlar|deagle|ak|he|awp>", "Купить снаряжение CS"), ("csstart", "", "Старт раунда CS"), ("cs", "", "Помощь CS")],
    state: {
        init: bool = false, active: bool = false, phase: Phase = Phase::Warmup, phase_ticks: u32 = 0, round_num: u32 = 0,
        t_score: i32 = 0, ct_score: i32 = 0, players: HashMap<i32, CsPlayer> = HashMap::new(),
        active_wpn: HashMap<i32, i32> = HashMap::new(), fire_held: HashSet<i32> = HashSet::new(), prev_dir: HashMap<i32, i32> = HashMap::new(), jump_held: HashSet<i32> = HashSet::new(),
        hook_held: HashSet<i32> = HashSet::new(), grenades: Vec<Grenade> = Vec::new(),
        bomb_carrier: Option<i32> = None, bomb_dropped: Option<(f32, f32)> = None,
        bomb_planted: bool = false, bomb_site: char = 'A', bomb_x: f32 = 0.0, bomb_y: f32 = 0.0,
        bomb_ticks: u32 = 0, plant_progress: u32 = 0, defuse_progress: u32 = 0,
        killfeed: String = String::new(), killfeed_ticks: u32 = 0, tick_counter: u32 = 0,
    },
    handle(api, ev) {
        if !init {
            init = true;
            api.set_manifest("11.0.0", &["player_control"]);
            if std::fs::read_to_string("/tmp/ddnet_cs_active.flag").is_ok_and(|v| v.trim() == "1") {
                active = true; api.set_gamemode(2); api.track_positions(true);
                api.spawn_visual_text(0, PLANT_A_X - 75.0, PLANT_A_Y - 90.0, 1.3, 1, "PLANT A");
                api.spawn_visual_text(1, PLANT_B_X - 75.0, PLANT_B_Y - 90.0, 1.3, 1, "PLANT B");
            }
        }
        match ev {
            Event::Ipc { payload, .. } => {
                if payload.contains("GAMEMODE:CS") {
                    api.set_gamemode(2);
                    active = true; round_num = 0; t_score = 0; ct_score = 0; phase = Phase::Warmup; phase_ticks = 0;
                    players.clear(); grenades.clear(); bomb_carrier = None; bomb_dropped = None; bomb_planted = false;
                    api.track_positions(true);
                    api.spawn_visual_text(0, PLANT_A_X - 75.0, PLANT_A_Y - 90.0, 1.3, 1, "PLANT A");
                    api.spawn_visual_text(1, PLANT_B_X - 75.0, PLANT_B_Y - 90.0, 1.3, 1, "PLANT B");
                    api.say("[CS 2D] Режим активирован! FreezeTime: 1-5/колесо выбор, ЛКМ купить, ПКМ автозакуп.");
                } else if payload.contains("GAMEMODE:OTHER") && active {
                    active = false; api.track_positions(false);
                    api.clear_visual_texts(); for id in 0..64 { api.remove_visual_pickup(id); } api.clear_visual_lasers();
                    for &cid in players.keys() { api.set_freeze(cid, 0); api.set_invincible(cid, false); for s in 0..4 { let _ = api.clear_hud_line(PlayerId(cid), s); } }
                    players.clear();
                }
            }
            Event::Leave { player } if active => {
                if let Some(&cid) = api.cids.get(&player) {
                    if bomb_carrier == Some(cid) {
                        bomb_carrier = None;
                        if let Some(st) = api.states.get(&cid) { bomb_dropped = Some((st.x, st.y)); api.spawn_visual_pickup(0, 1, 0, st.x, st.y - 12.0); }
                    }
                    players.remove(&cid); active_wpn.remove(&cid); fire_held.remove(&cid); hook_held.remove(&cid);
                    api.remove_visual_pickup(cid + 1);
                }
            }
            Event::Chat { player, message, .. } => {
                let parts: Vec<&str> = message.split_whitespace().collect();
                let cmd = parts.first().copied().unwrap_or("");
                if cmd.eq_ignore_ascii_case("!mode") {
                    if parts.get(1).is_some_and(|m| m.eq_ignore_ascii_case("cs")) {
                        let _ = std::fs::write("/tmp/ddnet_cs_active.flag", "1");
                        active = true; api.set_gamemode(2); api.track_positions(true);
                        api.spawn_visual_text(0, PLANT_A_X - 75.0, PLANT_A_Y - 90.0, 1.3, 1, "PLANT A");
                        api.spawn_visual_text(1, PLANT_B_X - 75.0, PLANT_B_Y - 90.0, 1.3, 1, "PLANT B");
                    } else if parts.len() > 1 {
                        let _ = std::fs::remove_file("/tmp/ddnet_cs_active.flag");
                        active = false;
                        for &c in players.keys() { api.set_freeze(c, 0); api.set_invincible(c, false); for sl in 0..4 { let _ = api.clear_hud_line(PlayerId(c), sl); } }
                        players.clear();
                    }
                    return;
                }
                if !active && !cmd.eq_ignore_ascii_case("!csstart") && !cmd.eq_ignore_ascii_case("!cs") { return; }
                let Some(&cid) = api.cids.get(&player) else { return; };
                if cmd.eq_ignore_ascii_case("!cs") {
                    api.whisper(&player, "[CS 2D] FreezeTime: 1-5/колесо выбор, ЛКМ купить, ПКМ автозакуп. В бою: ПКМ на A/B для C4.");
                } else if cmd.eq_ignore_ascii_case("!csstart") && api.is_staff(&player) {
                    active = true;
                    api.set_gamemode(2);
                    api.track_positions(true);
                    api.spawn_visual_text(0, PLANT_A_X - 75.0, PLANT_A_Y - 90.0, 1.3, 1, "PLANT A");
                    api.spawn_visual_text(1, PLANT_B_X - 75.0, PLANT_B_Y - 90.0, 1.3, 1, "PLANT B");
                    let mut all_cids: Vec<i32> = api.cids.values().copied().collect();
                    all_cids.sort();
                    for (idx, c) in all_cids.into_iter().enumerate() {
                        let team = (idx % 2) as i32;
                        let p = players.entry(c).or_default();
                        p.team = team;
                        api.set_player_team(c, team);
                    }
                    if !active { active = true; api.track_positions(true); api.spawn_visual_text(0, PLANT_A_X - 75.0, PLANT_A_Y - 90.0, 1.3, 1, "PLANT A"); api.spawn_visual_text(1, PLANT_B_X - 75.0, PLANT_B_Y - 90.0, 1.3, 1, "PLANT B"); }
                    phase = Phase::RoundEnd; phase_ticks = 1;
                    api.say("[CS 2D] Администратор запустил новый раунд!");
                } else if cmd.eq_ignore_ascii_case("!team") {
                    let nt = match parts.get(1).map(|s| s.to_lowercase()).as_deref() { Some("red" | "t" | "0") => Some(0), Some("blue" | "ct" | "1") => Some(1), Some("spec" | "-1") => Some(-1), _ => None };
                    if let Some(team) = nt {
                        let p = players.entry(cid).or_default(); p.team = team; p.alive = false;
                        api.set_player_team(cid, team); api.set_freeze(cid, 9999); api.set_invincible(cid, true);
                        api.whisper(&player, "[CS 2D] Команда изменена. Вступите в следующем раунде.");
                    }
                } else if cmd.eq_ignore_ascii_case("!buy") || cmd.eq_ignore_ascii_case("!b") {
                    if phase != Phase::FreezeTime && phase != Phase::Warmup { api.whisper(&player, "[CS 2D] Покупка только в FreezeTime!"); return; }
                    if let Some(p) = players.get_mut(&cid) {
                        if !p.alive { return; }
                        let slot = match parts.get(1).map(|s| s.to_lowercase()).as_deref() {
                            Some("kevlar" | "armor" | "kit") => Some(0), Some("deagle" | "dgl") => Some(1),
                            Some("ak" | "ak47" | "m4" | "m4a1" | "rifle") => Some(2), Some("he" | "grenade") => Some(3), Some("awp" | "sniper") => Some(4), _ => None,
                        };
                        if let Some(s) = slot { try_buy_slot(api, cid, p, s); }
                        else { api.whisper(&player, "[CS] !buy kevlar ($650) | deagle ($700) | ak ($2700) | he ($300) | awp ($4750)"); }
                    }
                }
            }
            Event::WeaponChange { cid, new, .. } if active => {
                active_wpn.insert(cid, new);
                if phase == Phase::FreezeTime {
                    if let Some(p) = players.get_mut(&cid) { if p.alive && (0..=4).contains(&new) { p.buy_slot = new as usize; api.play_sound(cid, 29); } }
                }
            }
            Event::Input { cid, dir, jump, fire, hook, target_x, target_y, .. } if active => {
                let fire_pressed = fire != 0 && fire_held.insert(cid);
                if fire == 0 { fire_held.remove(&cid); }
                let hook_pressed = hook != 0 && hook_held.insert(cid);
                if hook == 0 { hook_held.remove(&cid); }
                let Some(p) = players.get_mut(&cid) else { return; };
                if !p.alive || p.team < 0 { return; }
                let ld = prev_dir.insert(cid, dir).unwrap_or(0);
                let jump_pressed = jump != 0 && jump_held.insert(cid);
                if jump == 0 { jump_held.remove(&cid); }
                if phase == Phase::FreezeTime {
                    if dir != 0 && dir != ld {
                        p.buy_slot = if dir < 0 { (p.buy_slot + 4) % 5 } else { (p.buy_slot + 1) % 5 };
                        api.play_sound(cid, 29);
                    }
                    if jump_pressed || fire_pressed { try_buy_slot(api, cid, p, p.buy_slot); }
                    else if hook_pressed { auto_buy(api, cid, p); }
                    return;
                }
                if fire == 0 || p.fire_cd > 0 { return; }
                let Some(st) = api.states.get(&cid).copied() else { return; };
                let wpn = active_wpn.get(&cid).copied().unwrap_or(st.weapon);
                let len = ((target_x * target_x + target_y * target_y) as f32).sqrt().max(1.0);
                let (dx, dy) = (target_x as f32 / len, target_y as f32 / len);
                let (dmg, range, cd, wpn_name, reward, is_laser, is_he) = match wpn {
                    0 if fire_pressed => (55, 72.0, 20, "Нож", 1500, false, false),
                    1 if fire_pressed => if p.pistol == Pistol::Deagle { (54, 900.0, 22, "Desert Eagle", 300, false, false) } else { (27, 800.0, 11, if p.team == 1 { "USP-S" } else { "Glock-18" }, 300, false, false) },
                    2 if p.primary == Primary::Rifle => (35, 950.0, 7, if p.team == 1 { "M4A1" } else { "AK-47" }, 300, false, false),
                    3 if fire_pressed && p.has_he => (0, 0.0, 25, "HE Граната", 300, false, true),
                    4 if fire_pressed && p.primary == Primary::Awp => (125, 1600.0, 70, "AWP", 100, true, false),
                    _ => return,
                };
                p.fire_cd = cd;
                let shooter_team = p.team;
                if is_he {
                    p.has_he = false; api.set_ammo(cid, 3, -1);
                    grenades.push(Grenade { owner: cid, x: st.x + dx * 28.0, y: st.y + dy * 28.0, vx: dx * 24.0, vy: dy * 24.0 - 6.0, ticks_left: 85 });
                    api.play_sound(cid, 6); return;
                }
                let (mut end_x, mut end_y) = (st.x + dx * range, st.y + dy * range);
                let hit = raycast_player(api, &players, cid, shooter_team, st.x, st.y, dx, dy, range);
                if let Some((_, hx, hy)) = hit { end_x = hx; end_y = hy; }
                if is_laser { api.spawn_laser_ex(st.x, st.y, end_x, end_y, 0); api.play_sound(cid, 8); }
                else if wpn == 2 || (wpn == 1 && players.get(&cid).is_some_and(|x| x.pistol == Pistol::Deagle)) { api.spawn_laser_ex(st.x + dx * 24.0, st.y + dy * 24.0, end_x, end_y, 2); }
                if let Some((victim_cid, hx, hy)) = hit {
                    api.play_sound(cid, 14); api.play_sound(victim_cid, 15);
                    if let Some(vp) = players.get_mut(&victim_cid) {
                        let actual_armor = (((dmg as f32) * 0.4).round() as i32).min(vp.armor);
                        vp.armor -= actual_armor; vp.hp -= (dmg - actual_armor / 2).max(1);
                        if vp.hp <= 0 {
                            vp.hp = 0; vp.alive = false; api.set_freeze(victim_cid, 9999); api.set_invincible(victim_cid, true);
                            for w in 0..=4 { api.set_ammo(victim_cid, w, -1); }
                            api.remove_visual_pickup(victim_cid + 1);
                            if bomb_carrier == Some(victim_cid) { bomb_carrier = None; bomb_dropped = Some((hx, hy)); api.spawn_visual_pickup(0, 1, 0, hx, hy - 12.0); api.say("[CS] Носитель бомбы убит! C4 выпала."); }
                            let kname = api.names.get(&cid).cloned().unwrap_or_else(|| "Игрок".into());
                            let vname = api.names.get(&victim_cid).cloned().unwrap_or_else(|| "Игрок".into());
                            killfeed = format!("[KILL] {} [{}] -> {}", kname, wpn_name, vname); killfeed_ticks = 200;
                            api.set_player_score_name(&kname, t_score + ct_score + 1);
                            sync_hud_stats(api, victim_cid, vp);
                            if let Some(kp) = players.get_mut(&cid) { kp.money = (kp.money + reward).min(16000); }
                        } else {
                            sync_hud_stats(api, victim_cid, vp);
                        }
                    }
                }
            }
            Event::Tick { .. } if active => {
                tick_counter = tick_counter.wrapping_add(1);
                for p in players.values_mut() { if p.fire_cd > 0 { p.fire_cd -= 1; } }
                if killfeed_ticks > 0 { killfeed_ticks -= 1; if killfeed_ticks == 0 { killfeed.clear(); } }
                for (&cid, name) in &api.names {
                    if !players.contains_key(&cid) {
                        let t_cnt = players.values().filter(|p| p.team == 0).count();
                        let ct_cnt = players.values().filter(|p| p.team == 1).count();
                        let team = if t_cnt <= ct_cnt { 0 } else { 1 };
                        api.set_player_team(cid, team);
                        let mut p = CsPlayer { team, ..CsPlayer::default() };
                        p.alive = phase == Phase::Warmup;
                        if !p.alive { api.set_freeze(cid, 9999); api.set_invincible(cid, true); } else { give_cs_loadout(api, cid, &p); }
                        sync_hud_stats(api, cid, &p); players.insert(cid, p); api.whisper(name, "[CS 2D] Вы в игре!");
                    }
                }
                let mut i = 0;
                while i < grenades.len() {
                    grenades[i].x += grenades[i].vx; grenades[i].y += grenades[i].vy; grenades[i].vy += 0.55; grenades[i].vx *= 0.985;
                    let vid = 10 + (i as i32);
                    api.spawn_visual_pickup(vid, 3, 0, grenades[i].x, grenades[i].y);
                    if grenades[i].ticks_left > 0 { grenades[i].ticks_left -= 1; i += 1; } else {
                        let g = grenades.remove(i); api.remove_visual_pickup(vid);
                        for k in 0..6 { let a = (k as f32) * std::f32::consts::PI / 3.0; api.spawn_laser_ex(g.x, g.y, g.x + a.cos() * 140.0, g.y + a.sin() * 140.0, 1); }
                        let oteam = players.get(&g.owner).map(|p| p.team).unwrap_or(-1);
                        for (&vcid, vp) in players.iter_mut() {
                            if !vp.alive || vp.team < 0 || vp.team == oteam { continue; }
                            let Some(vst) = api.states.get(&vcid) else { continue };
                            let d = dist2(g.x, g.y, vst.x, vst.y).sqrt();
                            if d <= 190.0 {
                                vp.hp -= (((1.0 - d / 190.0) * 95.0).round() as i32).max(10);
                                if vp.hp <= 0 {
                                    vp.hp = 0; vp.alive = false; api.set_freeze(vcid, 9999); api.set_invincible(vcid, true);
                                    for w in 0..=4 { api.set_ammo(vcid, w, -1); }
                                    if bomb_carrier == Some(vcid) { bomb_carrier = None; bomb_dropped = Some((vst.x, vst.y)); api.spawn_visual_pickup(0, 1, 0, vst.x, vst.y - 12.0); }
                                    let kn = api.names.get(&g.owner).cloned().unwrap_or_else(|| "Игрок".into());
                                    let vn = api.names.get(&vcid).cloned().unwrap_or_else(|| "Игрок".into());
                                    killfeed = format!("[KILL] {} [HE Граната] -> {}", kn, vn); killfeed_ticks = 200;
                                }
                                sync_hud_stats(api, vcid, vp);
                            }
                        }
                    }
                }
                if let Some((bx, by)) = bomb_dropped {
                    for (&cid, p) in &players {
                        if p.alive && p.team == 0 && api.states.get(&cid).is_some_and(|st| dist2(st.x, st.y, bx, by) <= 56.0 * 56.0) {
                            bomb_dropped = None; bomb_carrier = Some(cid); api.remove_visual_pickup(0);
                            api.spawn_visual_pickup_follow(cid + 1, 1, 0, cid, -26.0, -18.0, 0.0); api.play_sound(cid, 28);
                            if let Some(nm) = api.names.get(&cid) { api.whisper(nm, "[CS] Вы подобрали бомбу C4!"); }
                            break;
                        }
                    }
                }
                if phase == Phase::Live {
                    if !bomb_planted {
                        if let Some(carrier) = bomb_carrier {
                            let ok = hook_held.contains(&carrier) && players.get(&carrier).is_some_and(|p| p.alive && p.team == 0)
                                && api.states.get(&carrier).is_some_and(|st| dist2(st.x, st.y, PLANT_A_X, PLANT_A_Y) <= PLANT_RADIUS * PLANT_RADIUS || dist2(st.x, st.y, PLANT_B_X, PLANT_B_Y) <= PLANT_RADIUS * PLANT_RADIUS);
                            if ok {
                                plant_progress += 1;
                                if plant_progress >= 150 {
                                    bomb_planted = true; bomb_carrier = None; plant_progress = 0; bomb_ticks = 35 * 50;
                                    api.remove_visual_pickup(carrier + 1);
                                    if let Some(st) = api.states.get(&carrier) {
                                        bomb_x = st.x; bomb_y = st.y;
                                        bomb_site = if dist2(st.x, st.y, PLANT_A_X, PLANT_A_Y) < dist2(st.x, st.y, PLANT_B_X, PLANT_B_Y) { 'A' } else { 'B' };
                                    }
                                    api.spawn_visual_pickup(0, 1, 0, bomb_x, bomb_y - 10.0);
                                    api.say(&format!("[CS 2D] БОМБА ЗАЛОЖЕНА НА ПЛЭНТЕ {}! Взрыв через 35 сек!", bomb_site));
                                }
                            } else { plant_progress = 0; }
                        }
                    } else {
                        if tick_counter % 25 == 0 { api.spawn_laser_ex(bomb_x, bomb_y, bomb_x, bomb_y - 110.0, 1); }
                        let mut kit = false; let mut defusing = false;
                        for (&cid, p) in &players {
                            if p.alive && p.team == 1 && hook_held.contains(&cid) && api.states.get(&cid).is_some_and(|st| dist2(st.x, st.y, bomb_x, bomb_y) <= 96.0 * 96.0) {
                                defusing = true; kit = p.has_kit; break;
                            }
                        }
                        if defusing {
                            defuse_progress += if kit { 2 } else { 1 };
                            if defuse_progress >= 250 {
                                bomb_planted = false; defuse_progress = 0; api.remove_visual_pickup(0);
                                ct_score += 1; api.set_team_score(t_score, ct_score);
                                for p in players.values_mut() { p.money = (p.money + if p.team == 1 { 3250 } else { 1400 }).min(16000); }
                                api.say(&format!("[CS 2D] Бомба обезврежена! Победа CT ({} : {})!", t_score, ct_score));
                                phase = Phase::RoundEnd; phase_ticks = 150;
                            }
                        } else { defuse_progress = 0; }
                    }
                }
                let active_pl = players.values().filter(|p| p.team == 0 || p.team == 1).count();
                if phase == Phase::Warmup && active_pl >= 2 {
                    let mut all_cids: Vec<i32> = players.keys().copied().collect();
                    all_cids.sort();
                    for (idx, c) in all_cids.into_iter().enumerate() {
                        let team = (idx % 2) as i32;
                        if let Some(p) = players.get_mut(&c) { p.team = team; api.set_player_team(c, team); }
                    }
                    phase = Phase::RoundEnd; phase_ticks = 100;
                    api.say("[CS 2D] Игроки готовы (T vs CT)! Матч через 2 сек...");
                }
                else if phase_ticks > 0 {
                    phase_ticks -= 1;
                    if phase_ticks == 0 {
                        match phase {
                            Phase::FreezeTime => { phase = Phase::Live; phase_ticks = 90 * 50; for (&cid, p) in &players { if p.alive && p.team >= 0 { api.set_freeze(cid, 0); api.set_invincible(cid, false); give_cs_loadout(api, cid, p); } } api.say("[CS 2D] В БОЙ! 01:30"); }
                            Phase::Live if !bomb_planted => { ct_score += 1; api.set_team_score(t_score, ct_score); for p in players.values_mut() { p.money = (p.money + if p.team == 1 { 3250 } else { 1400 }).min(16000); } api.say(&format!("[CS 2D] Время вышло! Победа CT ({} : {})!", t_score, ct_score)); phase = Phase::RoundEnd; phase_ticks = 150; }
                            Phase::RoundEnd => {
                                round_num += 1; phase = Phase::FreezeTime; phase_ticks = 7 * 50;
                                bomb_planted = false; bomb_dropped = None; bomb_carrier = None; plant_progress = 0; defuse_progress = 0; grenades.clear(); for id in 0..64 { api.remove_visual_pickup(id); }
                                let mut t_l: Vec<i32> = players.iter().filter(|(_, p)| p.team == 0).map(|(&c, _)| c).collect(); t_l.sort();
                                let mut ct_l: Vec<i32> = players.iter().filter(|(_, p)| p.team == 1).map(|(&c, _)| c).collect(); ct_l.sort();
                                for (i, &cid) in t_l.iter().enumerate() { if let Some(p) = players.get_mut(&cid) { p.alive = true; p.hp = 100; api.set_invincible(cid, false); api.set_pos(cid, T_SPAWN_X + (i as f32)*56.0, T_SPAWN_Y); api.set_vel(cid, 0.0, 0.0); api.set_freeze(cid, 7); api.set_invincible(cid, true); give_cs_loadout(api, cid, p); sync_hud_stats(api, cid, p); } }
                                for (i, &cid) in ct_l.iter().enumerate() { if let Some(p) = players.get_mut(&cid) { p.alive = true; p.hp = 100; api.set_invincible(cid, false); api.set_pos(cid, CT_SPAWN_X + (i as f32)*56.0, CT_SPAWN_Y); api.set_vel(cid, 0.0, 0.0); api.set_freeze(cid, 7); api.set_invincible(cid, true); give_cs_loadout(api, cid, p); sync_hud_stats(api, cid, p); } }
                                if let Some(&c) = t_l.get((round_num as usize) % t_l.len().max(1)) { bomb_carrier = Some(c); api.spawn_visual_pickup_follow(c + 1, 1, 0, c, -26.0, -18.0, 0.0); if let Some(nm) = api.names.get(&c) { api.whisper(nm, "[CS] У вас C4! Заложите её на A или B (ПКМ)."); } }
                                api.say(&format!("[CS 2D] Раунд #{} | T {} : {} CT | Закуп 6 сек!", round_num, t_score, ct_score));
                            }
                            _ => {}
                        }
                    }
                }
                if phase == Phase::Live {
                    if bomb_planted {
                        if bomb_ticks > 0 { bomb_ticks -= 1; } else {
                            bomb_planted = false; api.remove_visual_pickup(0);
                            for k in 0..8 { let a = (k as f32)*std::f32::consts::PI/4.0; api.spawn_laser_ex(bomb_x, bomb_y, bomb_x + a.cos()*320.0, bomb_y + a.sin()*320.0, 1); }
                            t_score += 1; api.set_team_score(t_score, ct_score);
                            for p in players.values_mut() { p.money = (p.money + if p.team == 0 { 3500 } else { 1400 }).min(16000); }
                            api.say(&format!("[CS 2D] БОМБА ВЗОРВАЛАСЬ! Победа T ({} : {})!", t_score, ct_score));
                            phase = Phase::RoundEnd; phase_ticks = 150;
                        }
                    }
                    if phase == Phase::Live {
                        let at = players.values().filter(|p| p.team == 0 && p.alive).count();
                        let act = players.values().filter(|p| p.team == 1 && p.alive).count();
                        if act == 0 && at > 0 {
                            t_score += 1; api.set_team_score(t_score, ct_score);
                            for p in players.values_mut() { p.money = (p.money + if p.team == 0 { 3250 } else { 1400 }).min(16000); }
                            api.say(&format!("[CS 2D] Все CT уничтожены! Победа T ({} : {})!", t_score, ct_score));
                            phase = Phase::RoundEnd; phase_ticks = 150;
                        } else if at == 0 && !bomb_planted && act > 0 {
                            ct_score += 1; api.set_team_score(t_score, ct_score);
                            for p in players.values_mut() { p.money = (p.money + if p.team == 1 { 3250 } else { 1400 }).min(16000); }
                            api.say(&format!("[CS 2D] Все T уничтожены! Победа CT ({} : {})!", t_score, ct_score));
                            phase = Phase::RoundEnd; phase_ticks = 150;
                        }
                    }
                }
                if tick_counter % 10 == 0 {
                    let at = players.values().filter(|p| p.team == 0 && p.alive).count();
                    let act = players.values().filter(|p| p.team == 1 && p.alive).count();
                    let sl = phase_ticks / 50;
                    for (&cid, p) in &players {
                        let l0 = match phase {
                            Phase::Warmup => format!("[CS 2D РАЗМИНКА] Ожидание 2-го игрока (или !csstart) | ${}", p.money),
                            Phase::FreezeTime => format!("[ЗАКУП: {}с] Баланс: ${} | A/D: Листать | Space: Купить | ПКМ: Авто", sl, p.money),
                            Phase::Live if bomb_planted => format!("[C4 НА {}] ВЗРЫВ: {}с | T {}:{} CT | Живы: {}v{}", bomb_site, bomb_ticks/50, t_score, ct_score, at, act),
                            Phase::Live => format!("[CS 2D] Раунд #{} | T {}:{} CT | {:02}:{:02} | Живы: {}v{}", round_num, t_score, ct_score, sl/60, sl%60, at, act),
                            Phase::RoundEnd => format!("[КОНЕЦ РАУНДА] T {}:{} CT | Следующий раунд через {}с...", t_score, ct_score, sl),
                        };
                        let l1 = if !p.alive && p.team >= 0 { format!("[ВЫ УБИТЫ — ДИПФРИЗ] Ожидание конца раунда | ${}", p.money) }
                        else if phase == Phase::FreezeTime { let (sn, sc) = buy_slot_info(p.buy_slot, p.team); format!("> {} (${}) < | 1:Броня 2:Deagle 3:AK/M4 4:HE 5:AWP", sn, sc) }
                        else {
                            let ws = match p.primary { Primary::Awp => "AWP", Primary::Rifle => if p.team == 1 { "M4A1" } else { "AK-47" }, Primary::None => if p.pistol == Pistol::Deagle { "Deagle" } else { "Пистолет" } };
                            let tag = if bomb_carrier == Some(cid) { " | [C4]" } else if p.has_kit { " | [KIT]" } else { "" };
                            format!("Ствол: {} | Баланс: ${}{}", ws, p.money, tag)
                        };
                        let mut l2 = String::new();
                        if p.alive && phase == Phase::Live {
                            if bomb_carrier == Some(cid) {
                                if plant_progress > 0 { l2 = format!("Закладка C4: {}%", (plant_progress*100/150).min(100)); }
                                else if let Some(st) = api.states.get(&cid) {
                                    if dist2(st.x, st.y, PLANT_A_X, PLANT_A_Y) <= PLANT_RADIUS*PLANT_RADIUS { l2 = "Вы на Плэнте A — зажмите ПКМ (Хук), чтобы заложить C4!".into(); }
                                    else if dist2(st.x, st.y, PLANT_B_X, PLANT_B_Y) <= PLANT_RADIUS*PLANT_RADIUS { l2 = "Вы на Плэнте B — зажмите ПКМ (Хук), чтобы заложить C4!".into(); }
                                }
                            } else if bomb_planted && p.team == 1 {
                                if defuse_progress > 0 { l2 = format!("Разминирование C4: {}%", (defuse_progress*100/250).min(100)); }
                                else if let Some(st) = api.states.get(&cid) { if dist2(st.x, st.y, bomb_x, bomb_y) <= 96.0*96.0 { l2 = "Рядом C4 — зажмите ПКМ для обезвреживания!".into(); } }
                            }
                        }
                        let _ = api.set_hud_line(PlayerId(cid), 0, &l0);
                        let _ = api.set_hud_line(PlayerId(cid), 1, &l1);
                        if l2.is_empty() { let _ = api.clear_hud_line(PlayerId(cid), 2); } else { let _ = api.set_hud_line(PlayerId(cid), 2, &l2); }
                        if killfeed.is_empty() { let _ = api.clear_hud_line(PlayerId(cid), 3); } else { let _ = api.set_hud_line(PlayerId(cid), 3, &killfeed); }
                    }
                }
            }
            _ => {}
        }
    }
}
