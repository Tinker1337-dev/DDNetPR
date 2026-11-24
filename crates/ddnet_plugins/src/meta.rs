use std::collections::{HashMap, HashSet, VecDeque};

pub const SERVER_API_VERSION: &str = "11.0.0";
pub const KNOWN_PERMISSIONS: &[&str] = &["tile_edit", "force_input", "noclip", "damage"];

pub fn is_api_compatible(ver: &str) -> bool {
    let mut parts = ver.trim().split('.');
    matches!((parts.next(), parts.next()), (Some("10") | Some("11"), Some("0")))
}

pub fn validate_permissions(perms: &[String]) -> Result<(), String> {
    for p in perms {
        if !KNOWN_PERMISSIONS.contains(&p.as_str()) {
            return Err(format!("unknown permission flag: '{}'", p));
        }
    }
    Ok(())
}

pub fn required_permission(cmd: &str) -> Option<&'static str> {
    let head = cmd.split_whitespace().next().unwrap_or("");
    match head {
        "set_tile" | "fill_tiles" | "restore_tiles" => Some("tile_edit"),
        "force_input" | "clear_force_input" => Some("force_input"),
        "set_noclip" => Some("noclip"),
        "deal_damage" | "heal" => Some("damage"),
        _ => None,
    }
}

#[derive(Default)]
pub struct StateRegistry {
    pub visible_to: HashMap<(String, i32, i32), bool>,
    pub alpha: HashMap<(String, i32), u8>,
    pub tiles: HashMap<(String, i32, i32), i32>,
    pub noclip: HashMap<String, u64>,
    pub gravity: HashMap<(String, i32), f32>,
    pub hud_lines: HashMap<(String, i32, u8), String>,
    pub fake_names: HashSet<(String, i32)>,
    pub fake_clans: HashSet<(String, i32)>,
    pub force_inputs: HashSet<(String, i32)>,
    pub projectiles: HashMap<String, HashSet<i32>>,
}

impl StateRegistry {
    pub fn track_command(&mut self, pname: &str, cmd: &str) {
        let p: Vec<&str> = cmd.split_whitespace().collect();
        match p.first().copied() {
            Some("set_visible") if p.len() >= 4 => {
                if let (Ok(v), Ok(t), Ok(st)) = (
                    p[1].parse::<i32>(),
                    p[2].parse::<i32>(),
                    p[3].parse::<i32>(),
                ) {
                    let key = (pname.to_string(), v, t);
                    if st == 1 {
                        self.visible_to.remove(&key);
                    } else {
                        self.visible_to.insert(key, false);
                    }
                }
            }
            Some("set_alpha") if p.len() >= 3 => {
                if let (Ok(c), Ok(a)) = (p[1].parse::<i32>(), p[2].parse::<u8>()) {
                    let key = (pname.to_string(), c);
                    if a == 255 {
                        self.alpha.remove(&key);
                    } else {
                        self.alpha.insert(key, a);
                    }
                }
            }
            Some("set_noclip") if p.len() >= 3 => {
                if let (Ok(c), Ok(en)) = (p[1].parse::<i32>(), p[2].parse::<i32>())
                    && (0..64).contains(&c)
                {
                    let mask = self.noclip.entry(pname.to_string()).or_insert(0);
                    if en != 0 {
                        *mask |= 1u64 << c;
                    } else {
                        *mask &= !(1u64 << c);
                    }
                }
            }
            Some("set_gravity") if p.len() >= 3 => {
                if let (Ok(c), Ok(g)) = (p[1].parse::<i32>(), p[2].parse::<f32>()) {
                    let key = (pname.to_string(), c);
                    if (g - 0.5).abs() < 1e-4 {
                        self.gravity.remove(&key);
                    } else {
                        self.gravity.insert(key, g);
                    }
                }
            }
            Some("set_hud_line") if p.len() >= 3 => {
                if let (Ok(c), Ok(slot)) = (p[1].parse::<i32>(), p[2].parse::<u8>()) {
                    self.hud_lines
                        .insert((pname.to_string(), c, slot), String::new());
                }
            }
            Some("set_fake_name") if p.len() >= 2 => {
                if let Ok(c) = p.get(1).unwrap_or(&"").parse::<i32>() {
                    self.fake_names.insert((pname.to_string(), c));
                }
            }
            Some("set_fake_clan") if p.len() >= 2 => {
                if let Ok(c) = p.get(1).unwrap_or(&"").parse::<i32>() {
                    self.fake_clans.insert((pname.to_string(), c));
                }
            }
            Some("force_input") if p.len() >= 2 => {
                if let Ok(c) = p.get(1).unwrap_or(&"").parse::<i32>() {
                    self.force_inputs.insert((pname.to_string(), c));
                }
            }
            Some("clear_force_input") if p.len() >= 2 => {
                if let Ok(c) = p.get(1).unwrap_or(&"").parse::<i32>() {
                    self.force_inputs.remove(&(pname.to_string(), c));
                }
            }
            Some("clear_hud_line") if p.len() >= 3 => {
                if let (Ok(c), Ok(slot)) = (p[1].parse::<i32>(), p[2].parse::<u8>()) {
                    self.hud_lines.remove(&(pname.to_string(), c, slot));
                }
            }
            Some("spawn_proj") if p.len() >= 2 => {
                if let Ok(id) = p[1].parse::<i32>() {
                    self.projectiles
                        .entry(pname.to_string())
                        .or_default()
                        .insert(id);
                }
            }
            Some("despawn_proj") if p.len() >= 2 => {
                if let Ok(id) = p[1].parse::<i32>()
                    && let Some(set) = self.projectiles.get_mut(pname)
                {
                    set.remove(&id);
                }
            }
            _ => {}
        }
    }

    #[allow(dead_code)]
    pub fn record_original_tile(&mut self, pname: &str, x: i32, y: i32, orig_tile: i32) {
        self.tiles
            .entry((pname.to_string(), x, y))
            .or_insert(orig_tile);
    }

    pub fn rollback_plugin(&mut self, pname: &str) -> Vec<String> {
        let mut cmds = Vec::new();
        self.visible_to.retain(|(p, v, t), _| {
            if p == pname {
                cmds.push(format!("set_visible {} {} 1", v, t));
                false
            } else {
                true
            }
        });
        self.alpha.retain(|(p, c), _| {
            if p == pname {
                cmds.push(format!("set_alpha {} 255", c));
                false
            } else {
                true
            }
        });
        self.tiles.retain(|(p, x, y), orig| {
            if p == pname {
                cmds.push(format!("set_tile {} {} {}", x, y, orig));
                false
            } else {
                true
            }
        });
        if let Some(mask) = self.noclip.remove(pname) {
            for cid in 0..64 {
                if (mask & (1u64 << cid)) != 0 {
                    cmds.push(format!("set_noclip {} 0", cid));
                }
            }
        }
        self.gravity.retain(|(p, c), _| {
            if p == pname {
                cmds.push(format!("set_gravity {} 0.5", c));
                false
            } else {
                true
            }
        });
        self.hud_lines.retain(|(p, c, slot), _| {
            if p == pname {
                cmds.push(format!("clear_hud_line {} {}", c, slot));
                false
            } else {
                true
            }
        });
        self.fake_names.retain(|(p, c)| {
            if p == pname {
                cmds.push(format!("set_fake_name {} \"\"", c));
                false
            } else {
                true
            }
        });
        self.fake_clans.retain(|(p, c)| {
            if p == pname {
                cmds.push(format!("set_fake_clan {} \"\"", c));
                false
            } else {
                true
            }
        });
        self.force_inputs.retain(|(p, c)| {
            if p == pname {
                cmds.push(format!("clear_force_input {}", c));
                false
            } else {
                true
            }
        });
        if let Some(ids) = self.projectiles.remove(pname) {
            for id in ids {
                cmds.push(format!("despawn_proj {}", id));
            }
        }
        cmds
    }
}

pub const MAX_FX_PER_SEC: u32 = 110;
pub const MAX_IPC_PER_SEC: u32 = 120;

#[derive(Default)]
pub struct PluginMetrics {
    pub tick_samples_us: VecDeque<u64>,
    pub cur_fx: u32,
    pub cur_ipc: u32,
    pub last_fx_sec: u32,
    pub last_ipc_sec: u32,
    pub scheduled_count: usize,
    pub over_limit_streak: u8,
}

impl PluginMetrics {
    pub fn record_tick_us(&mut self, us: u64) {
        if self.tick_samples_us.len() >= 100 {
            self.tick_samples_us.pop_front();
        }
        self.tick_samples_us.push_back(us);
    }

    pub fn avg_p99_us(&self) -> (u64, u64) {
        if self.tick_samples_us.is_empty() {
            return (0, 0);
        }
        let mut sorted: Vec<u64> = self.tick_samples_us.iter().copied().collect();
        sorted.sort_unstable();
        let sum: u64 = sorted.iter().sum();
        let avg = sum / sorted.len() as u64;
        let p99_idx = ((sorted.len() * 99) / 100).min(sorted.len() - 1);
        (avg, sorted[p99_idx])
    }

    pub fn roll_second(&mut self) -> (bool, bool) {
        self.last_fx_sec = self.cur_fx;
        self.last_ipc_sec = self.cur_ipc;
        self.cur_fx = 0;
        self.cur_ipc = 0;
        let exceeded = self.last_fx_sec > MAX_FX_PER_SEC || self.last_ipc_sec > MAX_IPC_PER_SEC;
        if exceeded {
            self.over_limit_streak = self.over_limit_streak.saturating_add(1);
        } else {
            self.over_limit_streak = 0;
        }
        (exceeded, self.over_limit_streak >= 3)
    }

    pub fn format_summary(&self, name: &str) -> String {
        let (avg, p99) = self.avg_p99_us();
        format!(
            "{}:tick_avg={}us,p99={}us,fx={}/s,ipc={}/s,sched={}",
            name, avg, p99, self.last_fx_sec, self.last_ipc_sec, self.scheduled_count
        )
    }
}
