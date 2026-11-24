use ddnet_sdk::{Event, ServerApi};
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

fn pick_random(list: &[String], exclude: Option<&str>) -> Option<String> {
    let pool: Vec<&String> = list.iter().filter(|p| Some(p.as_str()) != exclude).collect();
    if pool.is_empty() { return None; }
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().subsec_nanos() as usize;
    Some(pool[nanos % pool.len()].clone())
}

fn main() {
    let mut api = ServerApi::new();
    api.register_plugin("potato", "Ивент Горячая Картошка (удар молотком): !potato, !potato start, !potato vote, !potato stop");

    let mut active = false;
    let mut alive: HashSet<String> = HashSet::new();
    let mut holder: Option<String> = None;
    let mut timer: i32 = 0;
    let mut positions: HashMap<String, (f32, f32)> = HashMap::new();

    api.run(|api, event| match event {
        Event::Chat { player, message, .. } => {
            let parts: Vec<&str> = message.trim().split_whitespace().collect();
            if parts.first().copied() != Some("!potato") { return; }
            match parts.get(1).copied().unwrap_or("help") {
                "help" | "info" => api.whisper(&player, "[Картошка] Команды: !potato vote (голосование F3/F4), !potato start / stop (для персонала). Передача строго ударом молотка! Приз: +300 монет."),
                "vote" => {
                    if active { api.whisper(&player, "[Картошка] Ивент уже идёт!"); }
                    else if api.online.len() < 2 { api.whisper(&player, "[Картошка] Нужно минимум 2 игрока онлайн."); }
                    else { api.cmd("vote \"Запустить ивент: Горячая картошка? (F3 - Да)\" \"say !potato force_start\" \"Ивент Горячая Картошка\""); }
                }
                "start" | "force_start" => {
                    let is_vote = parts.get(1).copied() == Some("force_start");
                    if !is_vote && !api.authed_accounts.contains_key(&player) {
                        api.whisper(&player, "[Картошка] Только персонал может запускать без голосования. Используй !potato vote");
                        return;
                    }
                    if active { api.whisper(&player, "[Картошка] Ивент уже запущен!"); return; }
                    if api.online.len() < 2 { api.say("[Картошка] Недостаточно игроков для старта (нужно минимум 2)."); return; }
                    active = true;
                    api.enable_fast_tick(true);
                    alive = api.online.clone();
                    positions.clear();
                    let list: Vec<String> = alive.iter().cloned().collect();
                    holder = pick_random(&list, None);
                    timer = 200;
                    if let Some(ref h) = holder {
                        if let Some(&cid) = api.player_cids.get(h) { api.give_weapon(cid, 0); api.request_state(cid); api.play_sound(cid, 37); }
                        api.say(&format!("[Горячая картошка] Ивент начался! Участников: {}. Картошка у {} (20.0 сек)! Ударь МОЛОТКОМ другого игрока!", alive.len(), h));
                    }
                }
                "stop" => {
                    if !api.authed_accounts.contains_key(&player) { return; }
                    if active { active = false; api.enable_fast_tick(false); holder = None; alive.clear(); api.say("[Горячая картошка] Ивент остановлен администратором."); }
                }
                _ => {}
            }
        }
        Event::HammerHit { attacker, victim, .. } => {
            if !active { return; }
            if holder.as_deref() == Some(&attacker) && alive.contains(&victim) && attacker != victim {
                holder = Some(victim.clone());
                timer = 150;
                api.say(&format!("[Горячая картошка] {} передал картошку молотком игроку {}! Осталось 15.0 сек!", attacker, victim));
                if let Some(&acid) = api.player_cids.get(&attacker) {
                    api.reset_hook(acid);
                    api.play_sound(acid, 4);
                    api.broadcast_to(acid, &format!("Картошка передана игроку {} (у него 15.0 сек)! Убегай!", victim));
                }
                if let Some(&vcid) = api.player_cids.get(&victim) {
                    api.give_weapon(vcid, 0);
                    api.request_state(vcid);
                    api.play_sound(vcid, 4);
                    api.broadcast_to(vcid, "[!] КАРТОШКА У ТЕБЯ (15.0 сек)! УДАРЬ МОЛОТКОМ ДРУГОГО!");
                }
                if let Some(&(vx, vy)) = positions.get(&victim) { api.spawn_fx("hammer", vx, vy); api.spawn_fx("damage", vx, vy - 35.0); }
            }
        }
        Event::Leave { player } => {
            if !active { return; }
            alive.remove(&player);
            positions.remove(&player);
            if holder.as_deref() == Some(&player) {
                let list: Vec<String> = alive.iter().cloned().collect();
                holder = pick_random(&list, None);
                timer = 150;
                if let Some(ref h) = holder {
                    if let Some(&cid) = api.player_cids.get(h) { api.give_weapon(cid, 0); api.play_sound(cid, 37); }
                    api.say(&format!("[Горячая картошка] Носитель вышел! Картошка перешла к {} (15.0 сек)!", h));
                }
            }
        }
        Event::PlayerState { player, x, y, .. } => {
            if active && (x != 0.0 || y != 0.0) {
                positions.insert(player.clone(), (x, y));
                if holder.as_deref() == Some(&player) { api.spawn_fx("damage", x, y - 34.0); api.spawn_fx("damage", x, y - 48.0); }
            }
        }
        Event::FastTick { .. } => {
            if !active { return; }
            if alive.len() <= 1 {
                active = false;
                api.enable_fast_tick(false);
                if let Some(winner) = alive.iter().next().cloned() {
                    if let Some(acc) = api.authed_accounts.get(&winner) {
                        api.say(&format!("[Горячая картошка] ПОБЕДИТЕЛЬ: {}! Награда: +300 монет!", winner));
                        api.db_add(&format!("acc_coins:{}", acc), 300);
                    } else {
                        api.say(&format!("[Горячая картошка] ПОБЕДИТЕЛЬ: {}! (Войди в /login, чтобы получать монеты за победы!)", winner));
                    }
                    if let Some(&cid) = api.player_cids.get(&winner) { api.broadcast_to(cid, "ПОБЕДА В ГОРЯЧЕЙ КАРТОШКЕ! +300 МОНЕТ!"); api.play_sound(cid, 39); }
                    if let Some(&(wx, wy)) = positions.get(&winner) { api.spawn_fx("explosion", wx, wy - 40.0); }
                }
                holder = None;
                alive.clear();
                return;
            }
            if let Some(ref h) = holder.clone() {
                if timer % 3 == 0 { if let Some(&hcid) = api.player_cids.get(h) { api.request_state(hcid); } }
                if timer > 0 && timer % 5 == 0 {
                    for p in &alive {
                        if let Some(&cid) = api.player_cids.get(p) {
                            if p == h { api.broadcast_to(cid, &format!("[ГОРЯЧАЯ КАРТОШКА У ТЕБЯ!] Взрыв через {:.1}с — БЕЙ МОЛОТКОМ!", timer as f32 / 10.0)); }
                            else { api.broadcast_to(cid, &format!("[КАРТОШКА У: {}] Взрыв через {:.1}с — УБЕГАЙ!", h, timer as f32 / 10.0)); }
                        }
                    }
                }
                if (timer <= 50 && timer > 25 && timer % 10 == 0) || (timer <= 25 && timer > 0 && timer % 5 == 0) {
                    if let Some(&hcid) = api.player_cids.get(h) { api.play_sound(hcid, 37); }
                }
                if timer == 100 || (timer <= 50 && timer > 0 && timer % 10 == 0) {
                    api.say(&format!("[Горячая картошка] До взрыва {} сек! Картошка у: {}", timer / 10, h));
                }
                timer -= 1;
                if timer <= 0 {
                    if let Some(&(hx, hy)) = positions.get(h) { api.spawn_fx("explosion", hx, hy); }
                    api.play_sound(-1, 6);
                    if let Some(&cid) = api.player_cids.get(h) { api.reset_hook(cid); api.force_kill(cid); api.broadcast_to(cid, "БУМ! Картошка взорвалась у тебя в руках!"); }
                    alive.remove(h);
                    positions.remove(h);
                    api.say(&format!("[Горячая картошка] БУМ! {} выбывает! Осталось игроков: {}", h, alive.len()));
                    if alive.len() > 1 {
                        let list: Vec<String> = alive.iter().cloned().collect();
                        holder = pick_random(&list, None);
                        timer = 150;
                        if let Some(ref next_h) = holder {
                            if let Some(&cid) = api.player_cids.get(next_h) { api.give_weapon(cid, 0); api.request_state(cid); api.play_sound(cid, 37); }
                            api.say(&format!("[Горячая картошка] Новая картошка у {} (15.0 сек)!", next_h));
                        }
                    }
                }
            }
        }
        _ => {}
    });
}
