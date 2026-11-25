use ddnet_sdk::{Event, ServerApi};
use std::collections::{HashMap, HashSet};
use std::fs::OpenOptions;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

const MASTER_PASS: &str = "29052019";
const MOD_LOG: &str = "logs/mod.log";

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Role { User = 0, Helper = 1, Mod = 2, Owner = 3 }

impl Role {
    fn from_str(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "owner" | "admin" => Role::Owner, "mod" | "moder" | "moderator" => Role::Mod, "helper" | "support" => Role::Helper, _ => Role::User,
        }
    }
    fn as_str(&self) -> &'static str {
        match self {
            Role::Owner => "owner", Role::Mod => "mod", Role::Helper => "helper", Role::User => "user",
        }
    }
}

struct QueuedCmd { sender: String, account: String, cmd: String, args: Vec<String> }

fn audit_log(actor: &str, acc: &str, action: &str, target: &str, reason: &str) {
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(MOD_LOG) {
        let _ = writeln!(f, "[{}] {} (acc:{}) -> {} | цель: {} | причина: {}", ts, actor, acc, action, target, reason);
    }
}

fn resolve_cid(api: &ServerApi, target: &str) -> Option<i32> {
    target.parse::<i32>().ok().or_else(|| api.player_cids.get(target).or_else(|| api.cids.get(target)).copied())
}

fn main() {
    let mut api = ServerApi::new();
    api.register_admin_plugin("mod", "Модерация (после /login): !check, !uncheck, !hud, !warn, !mute, !kick, !ban, !unban, !freeze, !setrole, !ids, !staff");

    let mut queue: HashMap<String, QueuedCmd> = HashMap::new();
    let mut under_check: HashSet<String> = HashSet::new();
    let mut hud_enabled: HashSet<String> = HashSet::new();
    let mut seq: u64 = 0;
    let mut staff_req: HashMap<String, String> = HashMap::new();
    let mut pending_auth: HashMap<u64, (i32, String, String)> = HashMap::new();

    api.run(|api, event| match event {
        Event::Auth { cid, player, account } => {
            api.play_sound(cid, 28);
            seq += 1;
            api.schedule_after(1200, seq);
            pending_auth.insert(seq, (cid, player, account));
        }
        Event::Scheduled { callback_id } => {
            if let Some((cid, player, account)) = pending_auth.remove(&callback_id) {
                if let Some(r_str) = api.roles.get(&account) {
                    let role = Role::from_str(r_str);
                    if role >= Role::Helper {
                        hud_enabled.insert(player.clone());
                        api.cmd(&format!("spec_hud {} 1", cid));
                        api.set_vip_plus(cid, true);
                        api.whisper(&player, &format!("[Mod] Аккаунт '{}' авторизован. Ранг: {}, HUD включён.", account, role.as_str()));
                    } else {
                        api.whisper(&player, &format!("[Аккаунт] Вы вошли в аккаунт '{}'.", account));
                    }
                } else {
                    seq += 1;
                    let req = format!("init_{}", seq);
                    queue.insert(req.clone(), QueuedCmd { sender: player, account: account.clone(), cmd: "_init".into(), args: vec![] });
                    api.db_get(&req, &format!("role_acc:{}", account));
                }
            }
        }
        Event::Leave { player } => {
            hud_enabled.remove(&player);
            if under_check.remove(&player) {
                api.cmd(&format!("ban \"{}\" 10080 Выход при проверке на читы", player));
                api.say(&format!("[Античит] Игрок {} вышел во время проверки и получил авто-бан на 7 дней!", player));
                audit_log("SYSTEM", "system", "AUTO_BAN_LEAVE_CHECK", &player, "Выход во время проверки");
            }
        }
        Event::Chat { player, message, .. } => {
            let parts: Vec<String> = message.split_whitespace().map(|s| s.to_string()).collect();
            let cmd = parts.first().map(|s| s.as_str()).unwrap_or("");
            if !cmd.starts_with('!') { return; }
            if cmd == "!logout" {
                if api.authed_accounts.contains_key(&player) {
                    hud_enabled.remove(&player);
                    api.whisper(&player, "[Аккаунт] Вы вышли из аккаунта.");
                }
                return;
            }
            match cmd {
                "!auth_admin" | "!staff" | "!ids" | "!hud" | "!check" | "!uncheck" | "!setrole" | "!kick" | "!ban" | "!unban" | "!mute" | "!freeze" | "!warn" => {
                    let Some(acc) = api.authed_accounts.get(&player).cloned() else {
                        api.whisper(&player, "[Mod] Сначала войдите в аккаунт через /login!");
                        return;
                    };
                    if cmd == "!auth_admin" {
                        if parts.get(1).map(|s| s.as_str()) == Some(MASTER_PASS) {
                            api.db_set(&format!("role_acc:{}", acc), "owner");
                            hud_enabled.insert(player.clone());
                            if let Some(&cid) = api.player_cids.get(&player).or_else(|| api.cids.get(&player)) { api.cmd(&format!("spec_hud {} 1", cid)); api.set_vip_plus(cid, true); }
                            audit_log(&player, &acc, "AUTH_OWNER", &acc, "Мастер-ключ");
                            api.whisper(&player, &format!("[Mod] Аккаунту '{}' выданы права Owner и Spectator HUD!", acc));
                        } else { api.whisper(&player, "[Mod] Неверный мастер-пароль."); }
                        return;
                    }
                    seq += 1;
                    let req = format!("m_{}", seq);
                    queue.insert(req.clone(), QueuedCmd { sender: player.clone(), account: acc.clone(), cmd: cmd.to_string(), args: parts[1..].to_vec() });
                    api.db_get(&req, &format!("role_acc:{}", acc));
                }
                _ => {}
            }
        }
        Event::DbList { req_id, items } => {
            if let Some(asker) = staff_req.remove(&req_id) {
                let staff: Vec<String> = items.into_iter().filter(|(_, r)| r != "user" && !r.is_empty()).map(|(a, r)| format!("{} ({})", a, r)).collect();
                api.whisper(&asker, &format!("[Персонал] {}", if staff.is_empty() { "Пусто".into() } else { staff.join(" | ") }));
            }
        }
        // Обработка ответов из key-value базы данных для проверки ролей персонала
        Event::DbValue { req_id, value } => {
            if let Some(q) = queue.remove(&req_id) {
                let role = Role::from_str(&value);
                if q.cmd == "_init" {
                    if role >= Role::Helper {
                        hud_enabled.insert(q.sender.clone());
                        if let Some(&cid) = api.player_cids.get(&q.sender).or_else(|| api.cids.get(&q.sender)) { api.cmd(&format!("spec_hud {} 1", cid)); api.set_vip_plus(cid, true); }
                        api.whisper(&q.sender, &format!("[Mod] Аккаунт '{}' авторизован. Ранг: {}, HUD включён.", q.account, role.as_str()));
                    }
                    return;
                }
                match q.cmd.as_str() {
                    "!staff" if role >= Role::Helper => {
                        seq += 1;
                        let lid = format!("st_{}", seq);
                        staff_req.insert(lid.clone(), q.sender.clone());
                        api.db_get_prefix(&lid, "role_acc:");
                    }
                    "!ids" if role >= Role::Helper => {
                        let mut list: Vec<String> = api.player_cids.iter().map(|(n, c)| {
                            let acc = api.authed_accounts.get(n).map(|a| format!(" acc:{}", a)).unwrap_or_default();
                            format!("{} [ID:{}{}]", n, c, acc)
                        }).collect();
                        list.sort();
                        api.whisper(&q.sender, &format!("[Mod] Игроки: {}", if list.is_empty() { "нет".into() } else { list.join(", ") }));
                    }
                    "!hud" if role >= Role::Helper => {
                        if let Some(&cid) = api.player_cids.get(&q.sender).or_else(|| api.cids.get(&q.sender)) {
                            let on = if hud_enabled.remove(&q.sender) { 0 } else { hud_enabled.insert(q.sender.clone()); 1 };
                            api.cmd(&format!("spec_hud {} {}", cid, on));
                            api.whisper(&q.sender, &format!("[Mod] Spectator HUD: {}", if on == 1 { "ВКЛ" } else { "ВЫКЛ" }));
                        }
                    }
                    "!check" if role >= Role::Mod => {
                        if let Some(target) = q.args.first() {
                            if let Some(cid) = resolve_cid(api, target) {
                                let reason = if q.args.len() > 1 { q.args[1..].join(" ") } else { "Подозрение на читы".into() };
                                under_check.insert(target.clone());
                                api.reset_hook(cid);
                                api.play_sound(cid, 38);
                                api.cmd(&format!("mod_check {} 1 [ПРОВЕРКА НА ЧИТЫ] Вызвал: {} | Причина: {} | Выход = БАН!", cid, q.sender, reason));
                                api.say(&format!("[Mod] {} вызвал игрока {} на проверку ({})", q.sender, target, reason));
                                audit_log(&q.sender, &q.account, "CHECK_START", target, &reason);
                            } else { api.whisper(&q.sender, "[Mod] Игрок не найден. Проверь !ids"); }
                        }
                    }
                    "!uncheck" if role >= Role::Mod => {
                        if let Some(target) = q.args.first() {
                            under_check.remove(target);
                            if let Some(cid) = resolve_cid(api, target) { api.cmd(&format!("mod_check {} 0 ok", cid)); }
                            api.say(&format!("[Mod] {} завершил проверку игрока {}.", q.sender, target));
                            audit_log(&q.sender, &q.account, "CHECK_END", target, "Проверка пройдена");
                        }
                    }
                    "!setrole" if role >= Role::Owner => {
                        if let (Some(target), Some(r_str)) = (q.args.first(), q.args.get(1)) {
                            let t_acc = resolve_cid(api, target).and_then(|c| api.player_cids.iter().find(|(_, &v)| v == c).and_then(|(n, _)| api.authed_accounts.get(n).cloned())).or_else(|| api.authed_accounts.iter().find(|(n, _)| n.eq_ignore_ascii_case(target)).map(|(_, a)| a.clone())).unwrap_or_else(|| target.clone()); if !t_acc.is_empty() {
                                let nr = Role::from_str(r_str);
                                api.db_set(&format!("role_acc:{}", t_acc), nr.as_str());
                                audit_log(&q.sender, &q.account, "SET_ROLE", &format!("{} (acc:{})", target, t_acc), nr.as_str());
                                api.whisper(&q.sender, &format!("[Mod] Аккаунту '{}' ({}) назначен ранг: {}", t_acc, target, nr.as_str()));
                                api.whisper(target, &format!("[Mod] Вашему аккаунту выдан ранг: {}", nr.as_str()));
                            } else { api.whisper(&q.sender, "[Mod] Игрок должен быть онлайн и войти в аккаунт (/login)!"); }
                        }
                    }
                    "!warn" if role >= Role::Helper => {
                        if let Some(target) = q.args.first() {
                            let reason = if q.args.len() > 1 { q.args[1..].join(" ") } else { "Нарушение правил".into() };
                            audit_log(&q.sender, &q.account, "WARN", target, &reason);
                            api.say(&format!("[ВНИМАНИЕ] {} выдал предупреждение {}: {}", q.sender, target, reason));
                        }
                    }
                    "!mute" if role >= Role::Helper => {
                        if let Some(target) = q.args.first() {
                            if let Some(cid) = resolve_cid(api, target) {
                                let mins: u32 = q.args.get(1).and_then(|s| s.parse().ok()).unwrap_or(5);
                                api.cmd(&format!("muteid {} {}", cid, mins * 60));
                                audit_log(&q.sender, &q.account, &format!("MUTE_{}M", mins), target, "Мут в чате");
                                api.say(&format!("[Mod] {} выдал мут {} на {} мин.", q.sender, target, mins));
                            }
                        }
                    }
                    "!kick" if role >= Role::Helper => {
                        if let Some(target) = q.args.first() {
                            if let Some(cid) = resolve_cid(api, target) {
                                let reason = if q.args.len() > 1 { q.args[1..].join(" ") } else { "Кик".into() };
                                audit_log(&q.sender, &q.account, "KICK", target, &reason);
                                api.cmd(&format!("kick {} {}", cid, reason));
                                api.say(&format!("[Mod] {} кикнул {} ({})", q.sender, target, reason));
                            }
                        }
                    }
                    "!ban" if role >= Role::Mod => {
                        if let Some(target) = q.args.first() {
                            if let Some(cid) = resolve_cid(api, target) {
                                let mut mins: u32 = q.args.get(1).and_then(|s| s.parse().ok()).unwrap_or(30);
                                if role < Role::Owner && mins > 1440 { mins = 1440; }
                                let reason = if q.args.len() > 2 { q.args[2..].join(" ") } else { "Нарушение правил".into() };
                                audit_log(&q.sender, &q.account, &format!("BAN_{}M", mins), target, &reason);
                                api.cmd(&format!("ban {} {} {}", cid, mins, reason));
                                api.say(&format!("[Mod] {} забанил {} на {} мин. ({})", q.sender, target, mins, reason));
                            }
                        }
                    }
                    "!unban" if role >= Role::Mod => {
                        if let Some(ip_or_idx) = q.args.first() {
                            api.cmd(&format!("unban {}", ip_or_idx));
                            audit_log(&q.sender, &q.account, "UNBAN", ip_or_idx, "Разбан");
                            api.whisper(&q.sender, &format!("[Mod] Команда unban {} отправлена.", ip_or_idx));
                        }
                    }
                    "!freeze" if role >= Role::Mod => {
                        if let Some(target) = q.args.first() {
                            if let Some(cid) = resolve_cid(api, target) {
                                let secs: i32 = q.args.get(1).and_then(|s| s.parse().ok()).unwrap_or(10);
                                api.reset_hook(cid);
                                api.set_freeze(cid, secs);
                                api.play_sound(cid, 37);
                                audit_log(&q.sender, &q.account, &format!("FREEZE_{}S", secs), target, "Заморозка");
                                api.whisper(&q.sender, &format!("[Mod] {} заморожен на {} сек (крюк сброшен).", target, secs));
                            }
                        }
                    }
                    _ => api.whisper(&q.sender, "[Mod] Недостаточно прав или вы не авторизованы."),
                }
            }
        }
        _ => {}
    });
}
