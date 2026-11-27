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

/// Проверяет, имеет ли игрок права стаффа (helper/mod/admin/owner).
fn can_run_reboot(api: &ServerApi, player: &str) -> bool {
    api.is_staff(player)
        || api.authed_accounts.get(player)
            .and_then(|acc| api.roles.get(acc))
            .is_some_and(|r| matches!(r.as_str(), "helper" | "mod" | "admin" | "owner"))
}

fn main() {
    let mut api = ServerApi::new();
    let cfg: RebootConfig = api.load_config("reboot");
    api.set_manifest("11.0.0", &["moderation"]);
    api.set_priority(Priority::High);
    api.register_admin_plugin("reboot", "!reboot [сек] — мягкий ребут с предупреждением (Admin/Owner)");
    api.register_cmd("reboot", "[сек]", "Мягкий ребут с обратным отсчётом");

    let mut target_end: Option<u64> = None;
    let mut reason = String::new();
    let mut is_manual = false;
    let mut booted = false;

    api.run_intercept(move |api, ev| {
        match ev {
            Event::Chat { player, message, .. } => {
                let msg = message.trim();
                if msg == "!reboot" || msg.starts_with("!reboot ") {
                    if !can_run_reboot(api, &player) {
                        api.whisper(&player, "Только для Staff/Admin.");
                        return EventAction::Cancel;
                    }
                    let sec = msg.strip_prefix("!reboot").unwrap_or("").trim()
                        .parse::<u64>().unwrap_or(cfg.default_sec).clamp(cfg.min_sec, cfg.max_sec);
                    let _ = fs::write("/tmp/ddnet_reboot.json", format!("{}|Ручной рестарт ({})", sec, player));
                    api.whisper(&player, &format!("Запущен мягкий ребут через {} сек.", sec));
                    return EventAction::Cancel;
                }
            }
            Event::Tick { uptime_sec } => {
                if !booted {
                    booted = true;
                    log_info!("plugin_reboot loaded (default_sec={})", cfg.default_sec);
                }
                if let Ok(raw) = fs::read_to_string("/tmp/ddnet_reboot.json") {
                    let _ = fs::remove_file("/tmp/ddnet_reboot.json");
                    let mut sp = raw.trim().splitn(2, '|');
                    let sec = sp.next().and_then(|v| v.parse::<u64>().ok()).unwrap_or(cfg.default_sec);
                    reason = sp.next().unwrap_or(&cfg.default_reason).to_string();
                    log_warn!("scheduled reboot in {}s: {}", sec, reason);
                    is_manual = reason.starts_with("Ручной");
                    target_end = Some(uptime_sec + sec);
                }
                if let Some(end) = target_end {
                    let rem = end.saturating_sub(uptime_sec);
                    if rem > 0 && (rem <= 10 || rem % 5 == 0) {
                        api.raw(&format!("broadcast \"Перезагрузка сервера через {} сек! ({})\"", rem, reason));
                        api.say(&format!("[Ребут] Перезагрузка через {} сек ({})...", rem, reason));
                        if rem <= 5 {
                            for &cid in api.player_cids.values() { api.play_sound(cid, 30); }
                        }
                    } else if rem == 0 {
                        target_end = None;
                        api.raw("save_all");
                        for &cid in api.player_cids.values() {
                            api.raw(&format!("kick {} {}: перезагрузка сервера", cid, reason));
                        }
                        if is_manual {
                            let sdir = std::env::var("DDNET_SERVER_DIR").unwrap_or_else(|_| "/root/server_run".to_string());
                            let _ = std::process::Command::new("sh")
                                .arg("-c")
                                .arg(format!("sleep 1 && cd {} && ./start.sh", sdir))
                                .spawn();
                        }
                    }
                }
            }
            _ => {}
        }
        EventAction::Continue
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_reboot_staff_check() {
        let mut api = ServerApi::new();
        assert!(!can_run_reboot(&api, "Guest"));
        api.accounts.insert("Masua".into(), "Tinker1337".into());
        api.roles.insert("Tinker1337".into(), "owner".into());
        assert!(can_run_reboot(&api, "Masua"));
    }
}
