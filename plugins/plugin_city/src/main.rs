use ddnet_sdk::{Event, ServerApi};
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

enum Pending {
    Bal { player: String },
    Dice { player: String, acc: String, bet: i64 },
    Give { sender: String, to_nick: String, to_acc: String, amount: i64 },
    Pay { from: String, from_acc: String, to_nick: String, to_acc: String, amount: i64 },
}

fn cleanup_player_pending(pending: &mut HashMap<String, Pending>, player: &str) {
    pending.retain(|_, v| match v {
        Pending::Bal { player: p } | Pending::Dice { player: p, .. } => p != player,
        Pending::Give { sender, to_nick, .. } => sender != player && to_nick != player,
        Pending::Pay { from, to_nick, .. } => from != player && to_nick != player,
    });
}

fn rand_num(max: u64) -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().subsec_nanos() as u64 % max + 1
}

fn main() {
    let mut api = ServerApi::new();
    api.register_plugin("city", "Экономика (/login): !bal, !top, !dice <ставка|all|half>, !pay <ник> <сумма>, !givecoins <ник> <сумма> (Owner) (Owner: !givecoins)");
    let mut pending: HashMap<String, Pending> = HashMap::new();
    let mut acc_nicks: HashMap<String, String> = HashMap::new();
    let mut seq: u64 = 0;

    api.db_get_prefix("init_nicks", "acc_nick:");

    api.run(|api, event| match event {
        Event::Join { player } => { cleanup_player_pending(&mut pending, &player); api.say(&format!("[City] Добро пожаловать, {}! Войди в /login для зарплаты и !help для команд.", player)); }
        Event::Leave { player } => { cleanup_player_pending(&mut pending, &player); }
        Event::Ipc { from, payload } if from == "manager" && payload.starts_with("MAP_CHANGE|") => { pending.clear(); }
        Event::Auth { player, account, .. } => {
            if !player.is_empty() && !account.is_empty() {
                acc_nicks.insert(account.clone(), player.clone());
                api.db_set(&format!("acc_nick:{}", account), &player);
            }
        }
        Event::Rename { new_name, .. } => {
            if let Some(acc) = api.authed_accounts.get(&new_name).cloned() {
                acc_nicks.insert(acc.clone(), new_name.clone());
                api.db_set(&format!("acc_nick:{}", acc), &new_name);
            }
        }
        Event::Tick { uptime_sec } if uptime_sec > 0 && uptime_sec % 60 == 0 && !api.authed_accounts.is_empty() => {
            let mut rewarded_ips: HashSet<String> = HashSet::new();
            let mut rewarded_accs: HashSet<String> = HashSet::new();
            let mut list: Vec<String> = api.online.iter().cloned().collect();
            list.sort();
            for p in &list {
                let Some(acc) = api.authed_accounts.get(p) else { continue; };
                if !rewarded_accs.insert(acc.clone()) { continue; }
                if let Some(ip) = api.player_ips.get(p) {
                    if !ip.is_empty() && !rewarded_ips.insert(ip.clone()) { continue; }
                }
                api.db_add(&format!("acc_coins:{}", acc), 10);
                if let Some(&cid) = api.player_cids.get(p).or_else(|| api.cids.get(p)) { api.play_sound(cid, 24); }
            }
            if !rewarded_accs.is_empty() {
                api.say("[Банк] Авторизованным игрокам начислена зарплата: +10 монет!");
            }
        }
        Event::Chat { player, message, .. } => {
            let parts: Vec<&str> = message.split_whitespace().collect();
            let cmd = parts.first().copied().unwrap_or("");
            match cmd {
                "!bal" | "!balance" => {
                    let Some(acc) = api.authed_accounts.get(&player).cloned() else {
                        api.whisper(&player, "[Банк] Войди в аккаунт (/login), чтобы открыть счёт и получать зарплату!");
                        return;
                    };
                    seq += 1;
                    let id = format!("b_{}", seq);
                    pending.insert(id.clone(), Pending::Bal { player: player.clone() });
                    api.db_get(&id, &format!("acc_coins:{}", acc));
                }
                "!top" => {
                    seq += 1;
                    api.db_get_prefix(&format!("t_{}", seq), "acc_coins:");
                }
                "!dice" => {
                    let arg = parts.get(1).copied().unwrap_or("");
                    let all = arg == "all" || arg == "half";
                    let bet: i64 = if arg == "all" { -1 } else if arg == "half" { -2 } else { arg.parse().unwrap_or(0) };
                    if !all && bet <= 0 {
                        api.say(&format!("[Казино] {} бросил кубик и выбил: {}!", player, rand_num(6)));
                    } else {
                        let Some(acc) = api.authed_accounts.get(&player).cloned() else {
                            api.whisper(&player, "[Казино] Игра на монеты доступна только после /login!");
                            return;
                        };
                        seq += 1;
                        let id = format!("d_{}", seq);
                        pending.insert(id.clone(), Pending::Dice { player: player.clone(), acc: acc.clone(), bet});
                        api.db_get(&id, &format!("acc_coins:{}", acc));
                    }
                }
                "!givecoins" => {
                    let Some(fa) = api.authed_accounts.get(&player).cloned() else { return; };
                    let tn = parts.get(1).copied().unwrap_or("");
                    let amt: i64 = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
                    let Some(ta) = api.authed_accounts.get(tn).cloned() else { api.whisper(&player, "[Банк] Игрок не онлайн или без /login"); return; };
                    if amt > 0 { seq += 1; let id = format!("g_{}", seq); pending.insert(id.clone(), Pending::Give { sender: player.clone(), to_nick: tn.into(), to_acc: ta, amount: amt }); api.db_get(&id, &format!("role_acc:{}", fa)); }
                }
                "!pay" => {
                    let Some(from_acc) = api.authed_accounts.get(&player).cloned() else {
                        api.whisper(&player, "[Банк] Переводы доступны только после /login!");
                        return;
                    };
                    let to_nick = parts.get(1).copied().unwrap_or("");
                    let amt: i64 = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
                    if to_nick.is_empty() || amt <= 0 || to_nick == player {
                        api.whisper(&player, "[Банк] Использование: !pay <ник_онлайн> <сумма>");
                        return;
                    }
                    let Some(to_acc) = api.authed_accounts.get(to_nick).cloned() else {
                        api.whisper(&player, "[Банк] Получатель должен быть онлайн и войти в аккаунт (/login)!");
                        return;
                    };
                    if from_acc == to_acc {
                        api.whisper(&player, "[Банк] Нельзя переводить монеты самому себе!");
                        return;
                    }
                    if let (Some(ip1), Some(ip2)) = (api.player_ips.get(&player), api.player_ips.get(to_nick)) {
                        if !ip1.is_empty() && ip1 == ip2 {
                            api.whisper(&player, "[Банк] Запрещено переводить монеты на твинк (одинаковый IP)!");
                            return;
                        }
                    }
                    seq += 1;
                    let id = format!("p_{}", seq);
                    pending.insert(id.clone(), Pending::Pay {
                        from: player.clone(), from_acc: from_acc.clone(),
                        to_nick: to_nick.to_string(), to_acc, amount: amt,
                    });
                    api.db_get(&id, &format!("acc_coins:{}", from_acc));
                }
                _ => {}
            }
        }
        Event::DbList { req_id, items } => {
            if req_id == "init_nicks" {
                for (acc, nick) in items { if !nick.is_empty() { acc_nicks.insert(acc, nick); } }
                return;
            }
            let mut board: Vec<(String, i64)> = items.into_iter().map(|(acc, v)| {
                let display_nick = api.authed_accounts.iter().find(|(_, a)| **a == acc).map(|(n, _)| n.clone())
                    .or_else(|| acc_nicks.get(&acc).cloned()).unwrap_or_else(|| "Игрок".to_string());
                (display_nick, v.parse().unwrap_or(0))
            }).filter(|(_, v)| *v > 0).collect();
            board.sort_by(|a, b| b.1.cmp(&a.1));
            if board.is_empty() {
                api.say("[Топ-5 богачей] Пока пусто.");
            } else {
                api.say("=== [Топ-5 богачей сервера] ===");
                for (i, (nick, c)) in board.into_iter().take(5).enumerate() {
                    api.say(&format!("  #{} — {} ({} монет)", i + 1, nick, c));
                }
            }
        }
        // Обработка ответов из key-value базы данных (баланс, ставки !dice, переводы !pay)
        Event::DbValue { req_id, value } => {
            if let Some(act) = pending.remove(&req_id) {
                let cur: i64 = value.parse().unwrap_or(0);
                match act {
                    Pending::Bal { player } => api.whisper(&player, &format!("[Банк] Твой баланс: {} монет.", cur)),
                    Pending::Give { sender, to_nick, to_acc, amount } => {
                        if value.trim() == "owner" { api.db_add(&format!("acc_coins:{}", to_acc), amount); if let Some(&c) = api.player_cids.get(&to_nick).or_else(|| api.cids.get(&to_nick)) { api.play_sound(c, 24); } api.say(&format!("[Банк] {} начислил {} монет игроку {}!", sender, amount, to_nick)); }
                    }
                    Pending::Dice { player, acc, bet} => {
                        let bet = if bet == -1 { cur } else if bet == -2 { (cur / 2).max(1) } else { bet };
                        if cur <= 0 || cur < bet {
                            api.whisper(&player, &format!("[Казино] Не хватает монет (баланс: {}).", cur));
                        } else {
                            let r = rand_num(6);
                            let delta = if r >= 4 { bet } else { -bet };
                            api.db_add(&format!("acc_coins:{}", acc), delta);
                            if let Some(&cid) = api.player_cids.get(&player).or_else(|| api.cids.get(&player)) { api.play_sound(cid, if delta > 0 { 39 } else { 30 }); }
                            api.say(&format!("[Казино] {} выбил {} и {} {} монет! (Баланс: {})", player, r, if delta > 0 { "ВЫИГРАЛ" } else { "проиграл" }, bet, cur + delta));
                        }
                    }
                    Pending::Pay { from, from_acc, to_nick, to_acc, amount } => {
                        if cur < amount {
                            api.whisper(&from, &format!("[Банк] Недостаточно монет (баланс: {}).", cur));
                        } else {
                            api.db_add(&format!("acc_coins:{}", from_acc), -amount);
                            api.db_add(&format!("acc_coins:{}", to_acc), amount);
                            if let Some(&cid) = api.player_cids.get(&to_nick).or_else(|| api.cids.get(&to_nick)) { api.play_sound(cid, 24); }
                            api.say(&format!("[Банк] {} перевёл {} монет игроку {}!", from, amount, to_nick));
                        }
                    }
                }
            }
        }
        _ => {}
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_city_leave_and_map_change_cleanup_preserves_db() {
        let mut pending: HashMap<String, Pending> = HashMap::new();
        let mut acc_nicks: HashMap<String, String> = HashMap::new();
        acc_nicks.insert("AccAlice".into(), "Alice".into());
        pending.insert("d_1".into(), Pending::Dice { player: "Alice".into(), acc: "AccAlice".into(), bet: 50 });
        pending.insert("p_2".into(), Pending::Pay { from: "Bob".into(), from_acc: "AccBob".into(), to_nick: "Alice".into(), to_acc: "AccAlice".into(), amount: 25 });
        pending.insert("b_3".into(), Pending::Bal { player: "Charlie".into() });

        cleanup_player_pending(&mut pending, "Alice");
        assert!(!pending.contains_key("d_1"));
        assert!(!pending.contains_key("p_2"));
        assert!(pending.contains_key("b_3"));
        assert_eq!(acc_nicks.get("AccAlice").map(String::as_str), Some("Alice"));

        pending.clear();
        assert!(pending.is_empty());
        assert_eq!(acc_nicks.get("AccAlice").map(String::as_str), Some("Alice"));
    }
}
