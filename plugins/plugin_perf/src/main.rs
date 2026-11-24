use ddnet_sdk::{Event, PlayerId, ServerApi};
use std::collections::HashSet;
use std::fs;
use std::process::Command;

struct CpuSample { idle: u64, total: u64 }

fn read_cpu() -> Option<CpuSample> {
    let s = fs::read_to_string("/proc/stat").ok()?;
    let nums: Vec<u64> = s.lines().next()?.split_whitespace().skip(1).filter_map(|v| v.parse().ok()).collect();
    if nums.len() < 4 { return None; }
    Some(CpuSample { idle: nums[3] + nums.get(4).copied().unwrap_or(0), total: nums.iter().sum() })
}

fn srv_pid() -> Option<String> {
    let out = Command::new("pgrep").args(["-x", "DDNet-Server"]).output().ok()?;
    let pid = String::from_utf8_lossy(&out.stdout).lines().next()?.trim().to_string();
    if pid.is_empty() { None } else { Some(pid) }
}

fn netlimit(n: usize) -> i32 { if n <= 8 { 850 } else if n <= 20 { 1100 } else { 1400 } }

fn disk_gb() -> (f64, f64) {
    let Ok(out) = Command::new("df").args(["-k", "/"]).output() else { return (0.0, 0.0) };
    let b = String::from_utf8_lossy(&out.stdout); let cols: Vec<&str> = b.lines().nth(1).unwrap_or("").split_whitespace().collect();
    let tot: f64 = cols.get(1).and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let av: f64 = cols.get(3).and_then(|v| v.parse().ok()).unwrap_or(0.0);
    (av / 1048576.0, tot / 1048576.0)
}

fn uptime_str() -> String {
    let s = fs::read_to_string("/proc/uptime").ok().and_then(|v| v.split_whitespace().next()?.parse::<f64>().ok()).unwrap_or(0.0) as u64;
    if s >= 86400 { format!("{}д {}ч", s / 86400, (s % 86400) / 3600) }
    else if s >= 3600 { format!("{}ч {}м", s / 3600, (s % 3600) / 60) }
    else { format!("{}м {}с", s / 60, s % 60) }
}

fn plugins_mb() -> u64 {
    let Ok(out) = Command::new("ps").args(["-eo", "rss,comm"]).output() else { return 0 };
    let sum_kb: u64 = String::from_utf8_lossy(&out.stdout).lines().filter_map(|l| {
        let mut sp = l.split_whitespace();
        let kb = sp.next()?.parse::<u64>().ok()?;
        let cmd = sp.next()?;
        if cmd == "ddnet_plugins" || cmd.starts_with("plugin_") { Some(kb) } else { None }
    }).sum();
    sum_kb / 1024
}

fn stats(pid: &str) -> (String, String, u64, u64, u64) {
    let la = fs::read_to_string("/proc/loadavg").unwrap_or_default();
    let p: Vec<&str> = la.split_whitespace().collect();
    let l1 = p.first().copied().unwrap_or("0.00").to_string();
    let lall = if p.len() >= 3 { format!("{}/{}/{}", p[0], p[1], p[2]) } else { l1.clone() };
    let rss = fs::read_to_string(format!("/proc/{}/status", pid)).ok()
        .and_then(|s| s.lines().find(|l| l.starts_with("VmRSS:"))?.split_whitespace().nth(1)?.parse::<u64>().ok()).unwrap_or(0) / 1024;
    let (mut tot, mut av) = (0u64, 0u64);
    for l in fs::read_to_string("/proc/meminfo").unwrap_or_default().lines() {
        if l.starts_with("MemTotal:") { tot = l.split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0) / 1024; }
        else if l.starts_with("MemAvailable:") { av = l.split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0) / 1024; }
    }
    (l1, lall, rss, av, tot)
}
fn send_report(api: &ServerApi, player: &str, cpu: f64, hud: bool) {
    let Some(pid) = srv_pid() else { return };
    let (_, lall, rss, av, tot) = stats(&pid);
    let (df, dt) = disk_gb();
    let n = api.online.len();
    api.whisper(player, &format!("[Perf] CPU {:.1}% (Load {}) | UP {} | Онлайн {} (netlimit {}) | HUD: {}", cpu, lall, uptime_str(), n, netlimit(n), if hud { "ВКЛ" } else { "ВЫКЛ" }));
    api.whisper(player, &format!("[Perf] RAM: Ядро {}М + Плагины {}М | Своб {}/{}М | Диск {:.1}/{:.1}Г", rss, plugins_mb(), av, tot, df, dt));
}

fn push_hud(api: &ServerApi, watchers: &HashSet<String>, cpu: f64) {
    if watchers.is_empty() { return; }
    let Some(pid) = srv_pid() else { return };
    let (l1, _, rss, av, tot) = stats(&pid);
    let (df, _) = disk_gb();
    let n = api.online.len();
    let s1 = format!("[Perf] CPU {:.1}% (load {}) | UP {} | Онлайн {} (net {})", cpu, l1, uptime_str(), n, netlimit(n));
    let s2 = format!("[Perf] RAM: Ядро {}М + Плаг {}М | Своб {}/{}М | Диск {:.1}Г", rss, plugins_mb(), av, tot, df);
    for nick in watchers {
        if let Some(&cid) = api.player_cids.get(nick) {
            let p = PlayerId(cid);
            let _ = api.set_hud_line(p, 3, &s1);
            let _ = api.set_hud_line(p, 4, &s2);
        }
    }
}

fn main() {
    let mut api = ServerApi::new();
    api.register_admin_plugin("perf", "Мониторинг: !perf (отчёт + HUD вкл/выкл), !perf log, !online");
    let mut boosted = false;
    let mut watchers: HashSet<String> = HashSet::new();
    let mut prev = read_cpu();
    let mut cpu = 0.0f64;
    let mut ticks = 0u64;

    api.run(|api, event| match event {
        Event::Online { .. } | Event::Join { .. } => {
            if !boosted { if let Some(pid) = srv_pid() {
                let _ = Command::new("renice").args(["-n", "-15", "-p", &pid]).output();
                let _ = Command::new("ionice").args(["-c", "2", "-n", "0", "-p", &pid]).output();
                boosted = true; }
            }
            api.cmd(&format!("sv_netlimit {}", netlimit(api.online.len())));
            api.cmd("sv_netlimit_alpha 50");
        }
        Event::Leave { player, .. } => {
            watchers.remove(&player);
            api.cmd(&format!("sv_netlimit {}", netlimit(api.online.len())));
        }
        Event::Tick { .. } => {
            ticks += 1;
            if ticks % 50 == 0 {
                if let Some(cur) = read_cpu() {
                    if let Some(p) = &prev {
                        let dt = cur.total.saturating_sub(p.total);
                        let di = cur.idle.saturating_sub(p.idle);
                        if dt > 0 { cpu = ((dt.saturating_sub(di)) as f64 / dt as f64) * 100.0; }
                    }
                    prev = Some(cur);
                }
                push_hud(api, &watchers, cpu);
            }
        }
        Event::Chat { player, message, .. } => {
            if !api.is_staff(&player) { return; }
            match message.trim().to_lowercase().as_str() {
                "!perf" => {
                    if watchers.remove(&player) {
                        if let Some(&cid) = api.player_cids.get(&player) {
                            let _ = api.clear_hud_line(PlayerId(cid), 3);
                            let _ = api.clear_hud_line(PlayerId(cid), 4);
                        }
                        api.whisper(&player, "[Perf] HUD мониторинг выключен.");
                    } else {
                        watchers.insert(player.clone());
                        send_report(api, &player, cpu, true);
                        push_hud(api, &watchers, cpu);
                    }
                }
                "!perf log" | "!lag" | "!ping" => send_report(api, &player, cpu, watchers.contains(&player)),
                "!online" => {
                    let mut l: Vec<String> = api.online.iter().cloned().collect();
                    l.sort();
                    api.whisper(&player, &format!("[Онлайн ({})] {}", l.len(), if l.is_empty() { "пусто".into() } else { l.join(", ") }));
                }
                _ => {}
            }
        }
        _ => {}
    });
}
