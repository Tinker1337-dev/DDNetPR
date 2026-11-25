use ddnet_sdk::prelude::*;
use std::collections::HashSet;

plugin_config! {
    pub struct KatanaConfig {
        /// Включён ли плагин
        pub enabled: bool = true,
        /// Базовая высота катаны над головой (в пикселях, вверх — отрицательная)
        pub base_offset_y: f32 = -48.0,
        /// Амплитуда плавного покачивания (в пикселях)
        pub bob_amplitude: f32 = 5.0,
    }
}

ddnet_sdk::plugin! {
    name: "katana",
    desc: "!katana — левитирующая VIP-катана над головой для владельца и модераторов",
    admin: false,
    priority: Priority::Normal,
    cmds: [("katana", "", "Показать или скрыть левитирующую катану над головой")],
    config: KatanaConfig,
    state: {
        hidden_cids: HashSet<i32> = HashSet::new(),
        spawned_cids: HashSet<i32> = HashSet::new(),
    },
    handle(api, cfg, ev) {
        match ev {
            Event::Tick { .. } | Event::Auth { .. } | Event::Role { .. } | Event::Online { .. } => {
                let mut active = Vec::new();
                if cfg.enabled {
                    for (name, &cid) in &api.cids {
                        if cid >= 0 && cid < 64 && api.is_staff(name) && !hidden_cids.contains(&cid) {
                            active.push(cid);
                        }
                    }
                }
                let stale: Vec<i32> = spawned_cids.iter().filter(|c| !active.contains(c)).copied().collect();
                for cid in stale {
                    api.remove_visual_pickup(cid);
                    spawned_cids.remove(&cid);
                }
                for cid in active {
                    if spawned_cids.insert(cid) {
                        api.spawn_visual_pickup_follow(cid, 2, 5, cid, 0.0, cfg.base_offset_y, cfg.bob_amplitude);
                    }
                }
            }
            Event::Chat { player, message, .. } => {
                if message.trim().eq_ignore_ascii_case("!katana") {
                    if !api.is_staff(&player) {
                        api.whisper(&player, "Левитирующая катана доступна только владельцу и модераторам.");
                        return;
                    }
                    if let Some(&cid) = api.cids.get(&player) {
                        if hidden_cids.remove(&cid) {
                            spawned_cids.insert(cid);
                            api.spawn_visual_pickup_follow(cid, 2, 5, cid, 0.0, cfg.base_offset_y, cfg.bob_amplitude);
                            api.whisper(&player, "VIP-катана над головой включена.");
                        } else {
                            hidden_cids.insert(cid);
                            if spawned_cids.remove(&cid) { api.remove_visual_pickup(cid); }
                            api.whisper(&player, "VIP-катана над головой скрыта.");
                        }
                    }
                }
            }
            Event::Leave { player, .. } => {
                let cid = api.last_leave_cid.or_else(|| api.cids.get(&player).copied());
                if let Some(c) = cid {
                    hidden_cids.remove(&c);
                    if spawned_cids.remove(&c) { api.remove_visual_pickup(c); }
                }
            }
            _ => {}
        }
    }
}
