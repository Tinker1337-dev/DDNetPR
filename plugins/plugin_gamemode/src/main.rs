use ddnet_sdk::prelude::*;
use std::collections::HashMap;

plugin_config! {
    pub struct GamemodeConfig {
        pub default_mode: String = "race".into(),
        pub score_limit: i32 = 50,
        pub race_map: String = "Sunny Side Up".into(),
        pub block_map: String = "ChillBlock5".into(),
        pub fng_map: String = "ctf5_fngores".into(),
        pub tdm_map: String = "CBattleGores".into(),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode { Race, Block, Fng, Tdm }

impl Mode {
    fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "race" | "ddrace" => Some(Self::Race),
            "block" | "ffa" | "dm" => Some(Self::Block),
            "fng" | "openfng" => Some(Self::Fng),
            "tdm" | "teams" => Some(Self::Tdm),
            _ => None,
        }
    }
    fn code(self) -> i32 { match self { Self::Race => 0, Self::Block => 1, Self::Tdm => 2, Self::Fng => 3 } }
    fn is_team(self) -> bool { matches!(self, Self::Fng | Self::Tdm) }
    fn map_name(self, cfg: &GamemodeConfig) -> &str {
        match self {
            Self::Race => cfg.race_map.trim(),
            Self::Block => cfg.block_map.trim(),
            Self::Fng => cfg.fng_map.trim(),
            Self::Tdm => cfg.tdm_map.trim(),
        }
    }
    fn title(self) -> &'static str {
        match self {
            Self::Race => "Race (время в Tab)",
            Self::Block => "Block (очки в Tab)",
            Self::Fng => "FNG (Red vs Blue + очки)",
            Self::Tdm => "TDM (Red vs Blue + очки)",
        }
    }
}

fn apply_fng_loadout(api: &ServerApi, cid: i32) {
    for w in [1, 2, 3, 5] { api.set_ammo(cid, w, -1); }
    api.give_weapon(cid, 0);
    api.give_weapon(cid, 4);
}

fn reset_round(api: &ServerApi, mode: Mode, scores: &mut HashMap<String, i32>, teams: &mut HashMap<String, i32>, cids_map: &mut HashMap<String, i32>, red: &mut i32, blue: &mut i32) {
    scores.clear(); teams.clear(); *red = 0; *blue = 0;
    api.set_gamemode(mode.code());
    api.set_team_score(0, 0);
    match mode {
        Mode::Race => { api.set_freeze_autokill(0); api.set_laser_freeze(0); api.set_hammer_team_unfreeze(false); api.set_fng_weapons(false); }
        Mode::Block => { api.set_freeze_autokill(10); api.set_laser_freeze(0); api.set_hammer_team_unfreeze(false); api.set_fng_weapons(false); }
        Mode::Fng => { api.set_freeze_autokill(5); api.set_laser_freeze(5); api.set_hammer_team_unfreeze(true); api.set_fng_weapons(true); }
        Mode::Tdm => { api.set_freeze_autokill(0); api.set_laser_freeze(4); api.set_hammer_team_unfreeze(true); api.set_fng_weapons(false); }
    }
    let mut pls: Vec<(String, i32)> = api.cids.iter().map(|(n, &c)| (n.clone(), c)).collect();
    pls.sort_by_key(|(_, c)| *c);
    for (n, cid) in pls {
        scores.insert(n.clone(), 0); cids_map.insert(n.clone(), cid); api.set_player_score(cid, 0);
        let t = if mode.is_team() { -1 } else { 0 };
        teams.insert(n, t); api.set_player_team(cid, t);
        if mode == Mode::Fng { apply_fng_loadout(api, cid); } else { api.set_ammo(cid, 4, -1); api.give_weapon(cid, 0); api.give_weapon(cid, 1); }
    }
}

ddnet_sdk::plugin! {
    name: "gamemode",
    desc: "!mode <race|block|fng|tdm> — переключение режима, карты, очков и команд",
    admin: false,
    priority: Priority::Normal,
    cmds: [("mode", "[race|block|fng|tdm] [nomap]", "Сменить режим"), ("team", "<red|blue|spec>", "Выбрать команду")],
    config: GamemodeConfig,
    state: { init: bool = false, mode: Mode = Mode::Race, scores: HashMap<String, i32> = HashMap::new(), teams: HashMap<String, i32> = HashMap::new(), cids_map: HashMap<String, i32> = HashMap::new(), red: i32 = 0, blue: i32 = 0 },
    handle(api, cfg, ev) {
        if !init { init = true; mode = Mode::parse(&cfg.default_mode).unwrap_or(Mode::Race); reset_round(api, mode, &mut scores, &mut teams, &mut cids_map, &mut red, &mut blue); }
        match ev {
            Event::Scheduled { callback_id: 101 } => {
                reset_round(api, mode, &mut scores, &mut teams, &mut cids_map, &mut red, &mut blue);
                api.say(&format!("[Режим] Карта загружена — активен: {}!", mode.title()));
            }
            Event::Join { player } => {
                if let Some(&cid) = api.cids.get(&player) {
                    cids_map.insert(player.clone(), cid);
                    api.set_player_score(cid, *scores.get(&player).unwrap_or(&0));
                    if mode.is_team() { let t = *teams.entry(player).or_insert(-1); api.set_player_team(cid, t); }
                }
            }
            Event::Pickup { cid, kind } if mode == Mode::Fng && cid >= 0 && kind != 0 && kind != 4 => { apply_fng_loadout(api, cid); }
            Event::WeaponChange { cid, new, .. } if mode == Mode::Fng && cid >= 0 && new != 0 && new != 4 => { apply_fng_loadout(api, cid); }
            Event::TeamChange { cid, player, team } => {
                cids_map.insert(player.clone(), cid);
                if team >= 0 { teams.insert(player, team); } else { teams.remove(&player); }
            }
            Event::Leave { player } => { teams.remove(&player); }
            Event::Kill { killer, victim, weapon } if mode != Mode::Race && !killer.is_empty() && killer != victim => {
                let kt = *teams.get(&killer).unwrap_or(&-1);
                let vt = *teams.get(&victim).unwrap_or(&-1);
                if mode.is_team() && kt >= 0 && kt == vt { return; }
                let pts = if mode == Mode::Fng && weapon == 4 { 5 } else { 1 };
                let sc = scores.entry(killer.clone()).or_insert(0);
                *sc += pts;
                api.set_player_score_name(&killer, *sc);
                if mode == Mode::Fng && pts == 5 { api.say(&format!("[FNG] {} закинул {} в шипы (+5 очков)!", killer, victim)); }
                if mode.is_team() {
                    if kt == 1 { blue += pts; } else { red += pts; }
                    api.set_team_score(red, blue);
                    if cfg.score_limit > 0 && (red >= cfg.score_limit || blue >= cfg.score_limit) {
                        api.say(&format!("[Режим] Победила команда {} ({}:{})!", if red >= cfg.score_limit { "Red" } else { "Blue" }, red, blue));
                        reset_round(api, mode, &mut scores, &mut teams, &mut cids_map, &mut red, &mut blue);
                    }
                } else if cfg.score_limit > 0 && *sc >= cfg.score_limit {
                    api.say(&format!("[Режим] {} победил ({} очков)!", killer, *sc));
                    reset_round(api, mode, &mut scores, &mut teams, &mut cids_map, &mut red, &mut blue);
                }
            }
            Event::Chat { player, message, .. } => {
                let p: Vec<&str> = message.split_whitespace().collect();
                let cmd = p.first().copied().unwrap_or("");
                if cmd.eq_ignore_ascii_case("!mode") {
                    if p.len() == 1 { api.whisper(&player, &format!("[Режим] {} | Red {}:{} Blue", mode.title(), red, blue)); }
                    else if !api.is_staff(&player) { api.whisper(&player, "Только для модераторов и владельца."); }
                    else if let Some(m) = Mode::parse(p.get(1).copied().unwrap_or("")) {
                        mode = m;
                        let skip_map = p.get(2).is_some_and(|arg| arg.eq_ignore_ascii_case("nomap"));
                        let target_map = mode.map_name(&cfg);
                        reset_round(api, mode, &mut scores, &mut teams, &mut cids_map, &mut red, &mut blue);
                        if !skip_map && !target_map.is_empty() {
                            api.say(&format!("[Режим] {} включил: {} — смена карты на {}...", player, mode.title(), target_map));
                            api.change_map(target_map);
                            api.schedule_after(2, 101);
                        } else {
                            api.say(&format!("[Режим] {} включил: {}!", player, mode.title()));
                        }
                    }
                    else { api.whisper(&player, "Доступно: !mode race | block | fng | tdm [nomap]"); }
                } else if cmd.eq_ignore_ascii_case("!team") && mode.is_team() {
                    let Some(&cid) = api.cids.get(&player) else { return; };
                    let t = match p.get(1).map(|s| s.to_lowercase()).as_deref() { Some("red" | "r" | "0") => Some(0), Some("blue" | "b" | "1") => Some(1), Some("spec" | "-1") => Some(-1), _ => None };
                    if let Some(nt) = t { if nt >= 0 { teams.insert(player.clone(), nt); } else { teams.remove(&player); } api.set_player_team(cid, nt); api.whisper(&player, "Команда обновлена."); }
                    else { api.whisper(&player, "Использование: !team red | blue | spec"); }
                }
            }
            _ => {}
        }
    }
}
