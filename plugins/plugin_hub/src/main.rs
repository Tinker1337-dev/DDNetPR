use ddnet_sdk::{Event, ServerApi};
use std::collections::BTreeMap;

fn main() {
    let mut api = ServerApi::new();
    let mut public_reg: BTreeMap<String, String> = BTreeMap::new();
    let mut hidden_reg: BTreeMap<String, String> = BTreeMap::new();
    let mut metrics_map: BTreeMap<String, String> = BTreeMap::new();
    let mut discover_count = 0u8;

    let self_help = "Справка: !help (разделы сервера), !plugins (метрики плагинов), !<раздел> (команды)";
    api.register_plugin("help", self_help);
    public_reg.insert("help".to_string(), self_help.to_string());
    api.run(|api, event| match event {
        Event::Online { .. } | Event::Tick { .. } => {
            if discover_count < 3 {
                api.broadcast_ipc("DISCOVER");
                discover_count += 1;
            }
        }
        Event::Ipc { payload, .. } => {
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
        Event::Chat { player, message, .. } => {
            let msg = message.trim();
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
        _ => {}
    });
}
