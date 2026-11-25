use ddnet_sdk::prelude::*;
use std::collections::HashMap;

plugin_config! {
    pub struct GamemodeConfig {
        pub default_mode: String = "race".into(),
        pub score_limit: i32 = 50,
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
    fn code(self) -> i32 { match self { Self::Race => 0, Self::Block => 1, Self::Fng | Self::Tdm => 2 } }
    fn is_team(self) -> bool { matches!(self, Self::Fng | Self::Tdm) }
    fn title(self) -> &'static str {
        match self {
            Self::Race => "Race (время в Tab)",
            Self::Block => "Block (очки в Tab)",
            Self::Fng => "fng (Red vs Blue + очки)",
            Self::Tdm => "TDM (Red vs Blue + очки)",
        }
    }
}

fn reset_round(api: &ServerApi, mode: Mode, scores: &mut HashMap<String, i32>, teams: &mut HashMap<String, i32>, red: &mut i32, blue: &mut i32) {
    scores.clear(); teams.clear(); *red = 0; *blue = 0;
    api.set_gamemode(mode.code());
    api.set_team_score(0, 0);
    let mut pls: Vec<(String, i32)> = api.cids.iter().map(|(n, &c)| (n.clone(), c)).collect();
    pls.sort_by_key(|(_, c)| *c);
    for (_i, (n, cid)) in pls.into_iter().enumerate() {
        scores.insert(n.clone(), 0);
        api.set_player_score(cid, 0);
        let t = if mode.is_team() { -1 } else { 0 };
        teams.insert(n, t);
        api.set_player_team(cid, t);
    }
}

ddnet_sdk::plugin! {
    name: "gamemode",
    desc: "!mode <race|block|fng|tdm> — переключение режима, очков и команд",
    admin: false,
    priority: Priority::Normal,
    cmds: [("mode", "[race|block|fng|tdm]", "Сменить режим"), ("team", "<red|blue|spec>", "Выбрать команду")],
    config: GamemodeConfig,
    state: { init: bool = false, mode: Mode = Mode::Race, scores: HashMap<String, i32> = HashMap::new(), teams: HashMap<String, i32> = HashMap::new(), red: i32 = 0, blue: i32 = 0 },
    handle(api, cfg, ev) {
        if !init { init = true; mode = Mode::parse(&cfg.default_mode).unwrap_or(Mode::Race); reset_round(api, mode, &mut scores, &mut teams, &mut red, &mut blue); }
        match ev {
            Event::Join { player, .. } => {
                if let Some(&cid) = api.cids.get(&player) {
                    api.set_player_score(cid, *scores.get(&player).unwrap_or(&0));
                    if mode.is_team() {
                        let t = *teams.entry(player).or_insert(-1);
                        api.set_player_team(cid, t);
                    }
                }
            }
            Event::Leave { player } => { teams.remove(&player); }
            Event::Kill { killer, victim, .. } if mode != Mode::Race && !killer.is_empty() && killer != victim => {
                let kt = *teams.get(&killer).unwrap_or(&0);
                let vt = *teams.get(&victim).unwrap_or(&0);
                if mode.is_team() && kt == vt { return; }
                let sc = scores.entry(killer.clone()).or_insert(0);
                *sc += 1;
                if let Some(&kcid) = api.cids.get(&killer) { api.set_player_score(kcid, *sc); }
                if mode.is_team() {
                    if kt == 0 { red += 1; } else { blue += 1; }
                    api.set_team_score(red, blue);
                    if cfg.score_limit > 0 && (red >= cfg.score_limit || blue >= cfg.score_limit) {
                        api.say(&format!("[Режим] Победила команда {} ({}:{})!", if red >= cfg.score_limit { "Red" } else { "Blue" }, red, blue));
                        reset_round(api, mode, &mut scores, &mut teams, &mut red, &mut blue);
                    }
                } else if cfg.score_limit > 0 && *sc >= cfg.score_limit {
                    api.say(&format!("[Режим] {} победил ({} очков)!", killer, *sc));
                    reset_round(api, mode, &mut scores, &mut teams, &mut red, &mut blue);
                }
            }
            Event::Chat { player, message, .. } => { eprintln!("[gamemode] {} -> {}", player, message);
                let p: Vec<&str> = message.split_whitespace().collect();
                let cmd = p.first().copied().unwrap_or("");
                if cmd.eq_ignore_ascii_case("!mode") {
                    if p.len() == 1 { api.whisper(&player, &format!("[Режим] {} | Red {}:{} Blue", mode.title(), red, blue)); }
                    else if !api.is_staff(&player) { api.whisper(&player, "Только для модераторов и владельца."); }
                    else if let Some(m) = Mode::parse(p.get(1).copied().unwrap_or("")) { mode = m; reset_round(api, mode, &mut scores, &mut teams, &mut red, &mut blue); api.say(&format!("[Режим] {} включил: {}!", player, mode.title())); }
                    else { api.whisper(&player, "Доступно: !mode race | block | fng | tdm"); }
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
