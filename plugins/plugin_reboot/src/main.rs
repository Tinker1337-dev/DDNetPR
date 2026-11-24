use ddnet_sdk::prelude::*;
use std::fs;

plugin_config! {
    pub struct RebootConfig {
        /// Время отсчета по умолчанию (в секундах)
        pub default_sec: u64 = 10,
        /// Минимальное время ребута (сек)
        pub min_sec: u64 = 3,
        /// Максимальное время ребута (сек)
        pub max_sec: u64 = 300,
        /// Причина перезагрузки по умолчанию
        pub default_reason: String = String::from("Обновление сервера"),
    }
}
ddnet_sdk::plugin! {
    name: "reboot", desc: "!reboot [сек] — мягкий ребут с предупреждением (Admin/Owner)", admin: true, priority: Priority::High,
    cmds: [("reboot", "[сек]", "Мягкий ребут с обратным отсчётом")], config: RebootConfig,
    state: { target_end: Option<u64> = None, reason: String = String::new(), is_manual: bool = false, booted: bool = false },
    intercept(api, cfg, ev) {
        match ev {
            Event::Chat { player, message, .. } => {
                let msg = message.trim();
                if msg == "!reboot" || msg.starts_with("!reboot ") {
                    if !api.is_staff(&player) { api.whisper(&player, "Только для Staff/Admin."); return EventAction::Cancel; }
                    let sec = msg.strip_prefix("!reboot").unwrap_or("").trim().parse::<u64>().unwrap_or(cfg.default_sec).clamp(cfg.min_sec, cfg.max_sec);
                    let _ = fs::write("/tmp/ddnet_reboot.json", format!("{}|Ручной рестарт ({})", sec, player));
                    api.whisper(&player, &format!("Запущен мягкий ребут через {} сек.", sec));
                    return EventAction::Cancel;
                }
            }
            Event::Tick { uptime_sec } => {
                if !booted { booted = true; log_info!("plugin_reboot loaded (default_sec={})", cfg.default_sec); }
                if let Ok(raw) = fs::read_to_string("/tmp/ddnet_reboot.json") {
                    let _ = fs::remove_file("/tmp/ddnet_reboot.json");
                    let mut sp = raw.trim().splitn(2, "|");
                    let sec = sp.next().and_then(|v| v.parse::<u64>().ok()).unwrap_or(cfg.default_sec);
                    reason = sp.next().unwrap_or(&cfg.default_reason).to_string();
                    log_warn!("scheduled reboot in {}s: {}", sec, reason); is_manual = reason.starts_with("Ручной"); target_end = Some(uptime_sec + sec);
                }
                if let Some(end) = target_end {
                    let rem = end.saturating_sub(uptime_sec);
                    if rem > 0 && (rem <= 10 || rem % 5 == 0) {
                        println!("server_alert \"Перезагрузка сервера через {} сек! ({})\\\\nПрогресс сохраняется автоматически...\"", rem, reason);
                        if rem <= 5 { for &cid in api.player_cids.values() { api.play_sound(cid, 30); } }
                    } else if rem == 0 {
                        target_end = None; println!("save_all");
                        for &cid in api.player_cids.values() { println!("kick {} {}: перезагрузка сервера", cid, reason); }
                        if is_manual { let _ = std::process::Command::new("systemctl").arg("restart").arg("ddnet.service").spawn(); }
                    }
                }
            }
            _ => {}
        }
        EventAction::Continue
    }
}
