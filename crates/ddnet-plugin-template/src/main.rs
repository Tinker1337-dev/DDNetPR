use ddnet_sdk::prelude::*;
use std::collections::HashSet;

plugin_config! {
    pub struct TemplateConfig {
        /// Включён ли плагин
        pub enabled: bool = true,
        /// Кулдаун команды в секундах
        pub cooldown_sec: u64 = 3,
    }
}

ddnet_sdk::plugin! {
    name: "template",
    desc: "!template — шаблонный плагин DDNetPR v11.0",
    admin: false,
    priority: Priority::Normal,
    cmds: [("template", "", "Проверка работы шаблонного плагина")],
    config: TemplateConfig,
    state: {
        booted: bool = false,
        active_cids: HashSet<i32> = HashSet::new(),
    },
    handle(api, cfg, ev) {
        match ev {
            Event::Tick { .. } => {
                if !booted {
                    booted = true;
                    log_info!("plugin_template started (enabled={})", cfg.enabled);
                }
            }
            Event::Chat { player, message, .. } => {
                let cid = api.cids.get(&player).copied().unwrap_or(-1);
                if message.trim() == "!template" {
                    if !cfg.enabled {
                        api.whisper(&player, "Плагин временно отключён в конфиге.");
                        return;
                    }
                    active_cids.insert(cid);
                    log_info!("!template used by {} (cid={})", player, cid);
                    api.whisper(&player, "Шаблонный плагин DDNetPR v11.0 работает!");
                }
            }
            Event::Leave { player, .. } => {
                if let Some(cid) = api.cids.get(&player) { active_cids.remove(cid); }
            }
            _ => {}
        }
    }
}
