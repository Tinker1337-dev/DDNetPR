use ddnet_sdk::{Event, ServerApi};
use std::collections::{BTreeMap, HashMap};

/// Проверяет, содержит ли сообщение реакцию сердечка (<3, ❤️, ♥).
fn has_heart_token(msg: &str) -> bool {
    msg.contains("<3") || msg.contains("❤️") || msg.contains("♥")
}

/// Определяет адресата реакции сердечка: явно указанный ник в начале или последний писавший игрок.
fn resolve_heart_target(sender: &str, msg: &str, last_chatter: Option<&str>, api: &ServerApi) -> Option<String> {
    if let Some((prefix, _)) = msg.split_once(':') {
        let cand = prefix.trim();
        if !cand.is_empty() && cand != sender && api.cids.contains_key(cand) {
            return Some(cand.to_string());
        }
    }
    let first_word = msg.split_whitespace().next().unwrap_or("");
    if !first_word.is_empty() && first_word != sender && api.cids.contains_key(first_word) {
        return Some(first_word.to_string());
    }
    last_chatter
        .filter(|&prev| prev != sender && api.cids.contains_key(prev))
        .map(str::to_string)
}

fn main() {
    let mut api = ServerApi::new();
    let mut public_reg: BTreeMap<String, String> = BTreeMap::new();
    let mut hidden_reg: BTreeMap<String, String> = BTreeMap::new();
    let mut metrics_map: BTreeMap<String, String> = BTreeMap::new();
    let mut discover_count = 0u8;
    let mut now_sec = 0u64;
    let mut last_chatter: Option<String> = None;
    let mut heart_cd: HashMap<String, u64> = HashMap::new();

    let self_help = "Справка: !help (разделы сервера), !plugins (метрики плагинов), !<раздел> (команды)";
    api.register_plugin("help", self_help);
    public_reg.insert("help".to_string(), self_help.to_string());
    api.run(|api, event| match event {
        Event::Tick { uptime_sec } => { now_sec = uptime_sec; if discover_count < 3 { api.broadcast_ipc("DISCOVER"); discover_count += 1; } }
        Event::Online { .. } => {
            if discover_count < 3 {
                api.broadcast_ipc("DISCOVER");
                discover_count += 1;
            }
        }
        Event::Ipc { from, payload } => {
            if from == "manager" && payload.starts_with("MAP_CHANGE|") { last_chatter = None; heart_cd.clear(); }
            if let Some(rest) = payload.strip_prefix("REGISTER|") {
                if let Some((name, help)) = rest.split_once('|') {
                    let key = name.trim().to_lowercase();
                    if let Some(clean_help) = help.trim().strip_prefix("HIDDEN|") {
                        public_reg.remove(&key); hidden_reg.insert(key, clean_help.to_string());
                    } else {
                        public_reg.insert(key, help.trim().to_string());
                    }
                }
            } else if let Some(m_rest) = payload.strip_prefix("METRICS|") {
                for entry in m_rest.split(" | ") {
                    if let Some((pname, stats)) = entry.split_once(':') {
                        metrics_map.insert(pname.trim().to_string(), stats.trim().to_string());
                    }
                }
            }
        }
        Event::Chat { team, player, message } => {
            let msg = message.trim();
            if !msg.starts_with('!') && !msg.starts_with('/') {
                if team == 0 && has_heart_token(msg) {
                    let ready = heart_cd.get(&player).is_none_or(|&t| now_sec.saturating_sub(t) >= 3);
                    if ready {
                        heart_cd.insert(player.clone(), now_sec);
                        // Отправляем нативный эмотикон сердечек (EMOTICON_HEARTS = 2) отправителю и адресату
                        if let Some(&scid) = api.cids.get(&player).or_else(|| api.player_cids.get(&player)) {
                            api.raw(&format!("emote {} 2", scid));
                        }
                        if let Some(target) = resolve_heart_target(&player, msg, last_chatter.as_deref(), api) {
                            if let Some(&tcid) = api.cids.get(&target).or_else(|| api.player_cids.get(&target)) {
                                api.raw(&format!("emote {} 2", tcid));
                            }
                        }
                    }
                }
                if team == 0 && !msg.is_empty() {
                    last_chatter = Some(player);
                }
                return;
            }
            if !msg.starts_with('!') { return; }
            let raw = msg.trim_start_matches('!');
            let mut sp = raw.splitn(2, ' ');
            let cmd = sp.next().unwrap_or("").to_lowercase();
            let arg = sp.next().unwrap_or("").trim().to_lowercase();
            let is_staff = api.authed_accounts.get(&player)
                .and_then(|acc| api.roles.get(acc))
                .is_some_and(|r| matches!(r.as_str(), "helper" | "mod" | "owner"));

            if matches!(cmd.as_str(), "help" | "menu" | "помощь" | "меню") {
                api.broadcast_ipc("DISCOVER");
                let names: Vec<String> = public_reg.keys().cloned().collect();
                api.whisper(&player, &format!("[Меню сервера] Разделы: {} | Напиши !<раздел> для команд.", names.join(", ")));
            } else if cmd == "plugins" {
                if !arg.is_empty() {
                    if let Some(st) = metrics_map.get(&arg) {
                        api.whisper(&player, &format!("[Плагин {}] {}", arg, st));
                    } else {
                        api.whisper(&player, &format!("[Плагины] Метрики для \x27{}\x27 не найдены.", arg));
                    }
                } else {
                    let names: Vec<String> = metrics_map.keys().cloned().collect();
                    api.whisper(&player, &format!("[Плагины ({})] {} | Подробнее: !plugins <имя>", names.len(), names.join(", ")));
                }
            } else if cmd == "modules" && is_staff {
                api.broadcast_ipc("DISCOVER");
                let pub_names: Vec<String> = public_reg.keys().cloned().collect();
                let hid_names: Vec<String> = hidden_reg.keys().cloned().collect();
                api.whisper(&player, &format!("[Public ({})] {}", pub_names.len(), pub_names.join(", ")));
                api.whisper(&player, &format!("[Staff ({})] {}", hid_names.len(), hid_names.join(", ")));
            } else if let Some(help_text) = public_reg.get(&cmd) {
                api.whisper(&player, &format!("[Раздел: {}] {}", cmd, help_text));
            } else if let Some(help_text) = hidden_reg.get(&cmd).filter(|_| is_staff && cmd != "brush" && cmd != "sunstrike") {
                api.whisper(&player, &format!("[Staff: {}] {}", cmd, help_text));
            }
        }
        Event::Leave { player } => {
            heart_cd.remove(&player);
            if last_chatter.as_deref() == Some(player.as_str()) { last_chatter = None; }
        }
        _ => {}
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heart_reaction_target_and_cooldown() {
        let mut api = ServerApi::new();
        api.cids.insert("Tinker1337".into(), 0);
        api.cids.insert("dezo".into(), 1);
        assert!(has_heart_token("спасибо <3"));
        assert!(!has_heart_token("привет"));
        assert_eq!(resolve_heart_target("Tinker1337", "dezo: <3", None, &api).as_deref(), Some("dezo"));
        assert_eq!(resolve_heart_target("Tinker1337", "<3", Some("dezo"), &api).as_deref(), Some("dezo"));
        assert_eq!(resolve_heart_target("Tinker1337", "<3", Some("Tinker1337"), &api), None);
    }
}

