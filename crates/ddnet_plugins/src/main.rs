mod meta;

use meta::{
    PluginMetrics, SERVER_API_VERSION, StateRegistry, is_api_compatible, required_permission,
    validate_permissions,
};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use std::path::PathBuf;
fn server_dir() -> &'static Path {
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        std::env::var("DDNET_SERVER_DIR")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    })
}
fn spath(rel: &str) -> PathBuf { server_dir().join(rel) }

#[derive(Clone, Default)]
struct SessionInfo {
    cid: i32,
    ip: String,
    account: String,
}

struct ManagedPlugin {
    fast_tick: bool,
    name: String,
    path: String,
    priority: u8,
    enabled: bool,
    crash_times: Vec<Instant>,
    last_pong: Instant,
    track_pos: bool,
    reg_info: Option<String>,
    permissions: HashSet<String>,
    metrics: PluginMetrics,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    ack_rx: Option<mpsc::Receiver<(u64, bool)>>,
    unload_rx: Option<mpsc::Receiver<()>>,
}

fn log_pm(msg: &str) {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let lp = spath("logs/plugins.log"); if fs::metadata(&lp).map(|m| m.len() > 5_242_880).unwrap_or(false) { let _ = fs::rename(&lp, spath("logs/plugins.log.old")); }
    if let Ok(mut f) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(spath("logs/plugins.log"))
    {
        let _ = writeln!(f, "[{}] {}", ts, msg);
    }
}

fn log_plugin_err(pname: &str, msg: &str) {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let err_path = format!("{}/{}.err", spath("logs").display(), pname);
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&err_path) {
        let _ = writeln!(f, "[{}] {}", ts, msg);
    }
}

fn send_fifo(cmd: &str) {
    if let Ok(mut f) = OpenOptions::new().write(true).open(spath("server.fifo")) {
        let _ = writeln!(f, "{}", cmd);
    }
}

struct SchedTask {
    pname: String,
    cb_id: u64,
    next_run: Instant,
    interval: Option<Duration>,
}

fn sched_tasks() -> &'static Mutex<Vec<SchedTask>> {
    static TASKS: std::sync::OnceLock<Mutex<Vec<SchedTask>>> = std::sync::OnceLock::new();
    TASKS.get_or_init(|| Mutex::new(Vec::new()))
}

fn stop_plugin_clean(p: &mut ManagedPlugin, registry: &Arc<Mutex<StateRegistry>>) {
    p.track_pos = false;
    p.fast_tick = false;
    if let Ok(mut st) = sched_tasks().lock() {
        st.retain(|t| t.pname != p.name);
    }
    p.metrics.scheduled_count = 0;
    if let Ok(mut reg) = registry.lock() {
        for cmd in reg.rollback_plugin(&p.name) {
            send_fifo(&cmd);
        }
    }
    for fc in cleanup_visuals(&p.name) {
        send_fifo(&fc);
    }
    if let Some(ref mut sin) = p.stdin {
        let _ = writeln!(sin, "LIFECYCLE|UNLOAD");
        let _ = sin.flush();
    }
    if let Some(ref rx) = p.unload_rx {
        let _ = rx.recv_timeout(Duration::from_millis(40));
    }
    p.stdin = None;
    p.ack_rx = None;
    p.unload_rx = None;
    if let Some(ref mut c) = p.child {
        let _ = c.kill();
        let _ = c.wait();
    }
    p.child = None;
}

fn load_db() -> HashMap<String, String> {
    let mut map = HashMap::new();
    if let Ok(content) = fs::read_to_string(spath("plugins_kv.db")) {
        for line in content.lines() {
            if let Some((k, v)) = line.split_once('=') {
                map.insert(k.to_string(), v.to_string());
            }
        }
    }
    map
}

fn save_db(map: &HashMap<String, String>) {
    let mut out = String::new();
    for (k, v) in map {
        out.push_str(&format!("{}={}\n", k, v));
    }
    let _ = fs::write(spath("plugins_kv.db"), out);
}

fn parse_log_event(line: &str) -> String {
    if let Some(pos) = line.find("I wpn_chg:") {
        return format!("WPN_CHG|{}", line[pos + 10..].trim());
    }
    if let Some(pos) = line.find("I hook_attach:") {
        return format!("HOOK_ATTACH|{}", line[pos + 14..].trim());
    }
    if let Some(pos) = line.find("I hook_detach:") {
        return format!("HOOK_DETACH|{}", line[pos + 14..].trim());
    }
    if let Some(pos) = line.find("I pickup:") {
        return format!("PICKUP|{}", line[pos + 9..].trim());
    }
    if let Some(pos) = line.find("I finish:") {
        return format!("FINISH|{}", line[pos + 9..].trim());
    }
    if let Some(pos) = line.find("I custom_pkt:") {
        return format!("CUSTOM_PKT|{}", line[pos + 13..].trim());
    }
    if let Some(pos) = line.find("I tile_chk:") {
        return format!("TILE_CHK|{}", line[pos + 11..].trim());
    }
    if let Some(pos) = line.find("I tile:") {
        return format!("TILE|{}", line[pos + 7..].trim());
    }
    if let Some(pos) = line.find("I input:") {
        return format!("INPUT|{}", line[pos + 8..].trim());
    }
    if let Some(pos) = line.find("I death:") {
        return format!("DEATH|{}", line[pos + 8..].trim());
    }
    if let Some(pos) = line.find("I damage:") {
        return format!("DAMAGE|{}", line[pos + 9..].trim());
    }
    if let Some(pos) = line.find("I weapon_hit:") {
        return format!("WHIT|{}", line[pos + 13..].trim());
    }
    if let Some(pos) = line.find("I hammer_hit:") {
        return format!("HAMMER|{}", line[pos + 13..].trim());
    }
    if let Some(pos) = line.find("I state:") {
        return format!("STATE|{}", line[pos + 8..].trim());
    }
    if let Some(pos) = line.find("I game: kill ") {
        let rest = &line[pos + 13..];
        let mut killer = "";
        let mut victim = "";
        let mut weapon = "0";
        if let Some(ks) = rest.find("killer='") {
            let sub = &rest[ks + 8..];
            if let Some(ke) = sub.find('\'') {
                killer = sub[..ke].split_once(':').map(|x| x.1).unwrap_or(&sub[..ke]);
            }
        }
        if let Some(vs) = rest.find("victim='") {
            let sub = &rest[vs + 8..];
            if let Some(ve) = sub.find('\'') {
                victim = sub[..ve].split_once(':').map(|x| x.1).unwrap_or(&sub[..ve]);
            }
        }
        if let Some(ws) = rest.find("weapon=") {
            weapon = rest[ws + 7..].split_whitespace().next().unwrap_or("0");
        }
        if !killer.is_empty() && !victim.is_empty() {
            return format!("KILL|{}|{}|{}", killer, victim, weapon);
        }
    }
    if let Some(pos) = line.find("I plugin: bestclient cid=") {
        let rest = line[pos + 25..].trim();
        if let Some((cid, ver)) = rest.split_once(" ver=") {
            return format!("BESTCLIENT|{}|{}", cid.trim(), ver.trim());
        }
    }
    if let Some(pos) = line.find("I auth:") {
        let rest = line[pos + 7..].trim();
        if let Some((cid, tail)) = rest.split_once(':')
            && let Some((nick, acc)) = tail.rsplit_once(':')
        {
            return format!("AUTH|{}|{}|{}", cid.trim(), nick.trim(), acc.trim());
        }
    }
    if let Some(pos) = line.find("I team:") {
        let rest = line[pos + 7..].trim();
        if let Some((cid_s, tail)) = rest.split_once(':')
            && let Some((nick, team_s)) = tail.rsplit_once(':')
        {
            return format!("IPC|manager|TEAM|{}|{}|{}", cid_s.trim(), team_s.trim(), nick.trim());
        }
    }
    if let Some(pos) = line.find("I chat:") {
        let rest = line[pos + 7..].trim();
        if let Some(after_cid) = rest.split_once(':')
            && let Some((team_str, after_team)) = after_cid.1.split_once(':')
            && let Some((nick, msg)) = after_team.split_once(": ")
        {
            return format!("CHAT|{}|{}|{}", team_str.trim(), nick.trim(), msg.trim());
        }
    }
    if let Some(pos) = line.find("changed name to '") {
        let before = line[..pos].trim_end();
        let after = &line[pos + 17..];
        if let Some(b2) = before.strip_suffix('\'')
            && let (Some(f), Some(e)) = (b2.find('\''), after.rfind('\''))
        {
            return format!("RENAME|{}|{}", &b2[f + 1..], &after[..e]);
        }
    }
    if let Some(pos) = line.find("entered and joined the game") {
        let before = &line[..pos];
        if let Some(s) = before.rfind('\'') {
            let sub = &before[..s];
            if let Some(f) = sub.find('\'') {
                return format!("JOIN|{}", &sub[f + 1..]);
            }
        }
    }
    if let Some(pos) = line.find("has left the game") {
        let before = &line[..pos];
        if let Some(s) = before.rfind('\'') {
            let sub = &before[..s];
            if let Some(f) = sub.find('\'') {
                return format!("LEAVE|{}", &sub[f + 1..]);
            }
        }
    }
    String::new()
}

fn spawn_plugin(
    p: &mut ManagedPlugin,
    db: Arc<Mutex<HashMap<String, String>>>,
    plugins: Arc<Mutex<Vec<ManagedPlugin>>>,
    registry: Arc<Mutex<StateRegistry>>,
    is_reload: bool,
    sessions: Arc<Mutex<HashMap<String, SessionInfo>>>,
) {
    p.track_pos = false;
    p.fast_tick = false;
    let db_ref = Arc::clone(&db);
    let err_path = format!("{}/{}.err", spath("logs").display(), p.name);
    let stderr_cfg = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&err_path)
        .map(Stdio::from)
        .unwrap_or_else(|_| Stdio::null());

    if let Ok(mut child) = Command::new(&p.path).current_dir(server_dir()).env("DDNET_SERVER_DIR", server_dir())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(stderr_cfg)
        .spawn()
    {
        let stdin = child.stdin.take();
        let stdout = child.stdout.take();
        let (ack_tx, ack_rx) = mpsc::channel::<(u64, bool)>();
        let (unload_tx, unload_rx) = mpsc::channel::<()>();
        let pname = p.name.clone();
        let plugins_ref = Arc::clone(&plugins);
        let reg_ref = Arc::clone(&registry);

        if let Some(out) = stdout {
            thread::spawn(move || {
                let reader = BufReader::new(out);
                let mut rate_win = Instant::now();
                let mut rate_cnt = 0u32;

                for l in reader.lines().map_while(Result::ok) {
                    let cmd = l.trim();
                    if cmd.is_empty() {
                        continue;
                    }
                    if let Some(ver) = cmd.strip_prefix("@PM SDK_VER|") {
                        if !is_api_compatible(ver) {
                            let msg = format!(
                                "plugin built for API {}, server API {}",
                                ver.trim(),
                                SERVER_API_VERSION
                            );
                            log_plugin_err(&pname, &msg);
                            log_pm(&format!("[VersionMismatch] {}: {}", pname, msg));
                            if let Ok(mut list) = plugins_ref.lock()
                                && let Some(pl) = list.iter_mut().find(|x| x.name == pname)
                            {
                                pl.enabled = false;
                                if let Some(ref mut c) = pl.child {
                                    let _ = c.kill();
                                    let _ = c.wait();
                                }
                            }
                            break;
                        }
                        continue;
                    }
                    if let Some(man) = cmd.strip_prefix("@PM MANIFEST|") {
                        let mut sp = man.splitn(2, '|');
                        let api_v = sp.next().unwrap_or("");
                        let raw_perms = sp.next().unwrap_or("");
                        if !is_api_compatible(api_v) {
                            let msg = format!(
                                "plugin built for API {}, server API {}",
                                api_v.trim(),
                                SERVER_API_VERSION
                            );
                            log_plugin_err(&pname, &msg);
                            if let Ok(mut list) = plugins_ref.lock()
                                && let Some(pl) = list.iter_mut().find(|x| x.name == pname)
                            {
                                pl.enabled = false;
                                if let Some(ref mut c) = pl.child {
                                    let _ = c.kill();
                                    let _ = c.wait();
                                }
                            }
                            break;
                        }
                        let parsed: Vec<String> = raw_perms
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                        if let Err(err_msg) = validate_permissions(&parsed) {
                            log_plugin_err(&pname, &err_msg);
                            log_pm(&format!("[PermissionError] {}: {}", pname, err_msg));
                            if let Ok(mut list) = plugins_ref.lock()
                                && let Some(pl) = list.iter_mut().find(|x| x.name == pname)
                            {
                                pl.enabled = false;
                                if let Some(ref mut c) = pl.child {
                                    let _ = c.kill();
                                    let _ = c.wait();
                                }
                            }
                            break;
                        }
                        if let Ok(mut list) = plugins_ref.lock()
                            && let Some(pl) = list.iter_mut().find(|x| x.name == pname)
                        {
                            pl.permissions = parsed.into_iter().collect();
                        }
                        continue;
                    }
                    if let Some(pmsg) = cmd.strip_prefix("@PM PANIC|") {
                        log_plugin_err(&pname, &format!("PANIC isolated: {}", pmsg));
                        log_pm(&format!(
                            "[PanicIsolated] Плагин '{}' переведён в disabled: {}",
                            pname, pmsg
                        ));
                        if let Ok(mut list) = plugins_ref.lock()
                            && let Some(pl) = list.iter_mut().find(|x| x.name == pname)
                        {
                            pl.enabled = false;
                            stop_plugin_clean(pl, &reg_ref);
                        }
                        break;
                    }
                    if cmd == "@PM UNLOAD_ACK" {
                        let _ = unload_tx.send(());
                        continue;
                    }
                    if let Some(rest) = cmd.strip_prefix("@REG_SUB ") {
                        if let Ok(mut list) = plugins_ref.lock()
                            && let Some(hub) = list.iter_mut().find(|x| x.enabled && x.name == "plugin_hub")
                            && let Some(ref mut sin) = hub.stdin
                        {
                            let _ = writeln!(sin, "IPC|manager|REGISTER_SUB|{}", rest);
                        }
                        continue;
                    }
                    if let Some(rest) = cmd.strip_prefix("@REG_CMD ").or_else(|| cmd.strip_prefix("@REG_ADMIN ")) {
                        let is_adm = cmd.starts_with("@REG_ADMIN ");
                        let reg = if is_adm {
                            if let Some((c, d)) = rest.split_once("|") { format!("REGISTER|{}|HIDDEN|{}", c, d) } else { format!("REGISTER|{}|HIDDEN|", rest) }
                        } else {
                            format!("REGISTER|{}", rest)
                        };
                        if let Ok(mut list) = plugins_ref.lock() {
                            if let Some(pl) = list.iter_mut().find(|x| x.name == pname) { pl.reg_info = Some(reg.clone()); }
                            if let Some(hub) = list.iter_mut().find(|x| x.enabled && x.name == "plugin_hub")
                                && let Some(ref mut sin) = hub.stdin
                            {
                                let _ = writeln!(sin, "IPC|manager|{}", reg);
                            }
                        }
                        continue;
                    }
                    if cmd == "@PONG" {
                        if let Ok(mut list) = plugins_ref.lock()
                            && let Some(pl) = list.iter_mut().find(|x| x.name == pname)
                        {
                            pl.last_pong = Instant::now();
                        }
                        continue;
                    }
                    if let Some(arg) = cmd.strip_prefix("@TRACK_POS ") {
                        if let Ok(mut list) = plugins_ref.lock()
                            && let Some(pl) = list.iter_mut().find(|x| x.name == pname)
                        {
                            pl.track_pos = arg.trim() == "1";
                        }
                        continue;
                    }
                    if let Some(ack_rest) = cmd.strip_prefix("@ACK ") {
                        if let Some((id_s, act)) = ack_rest.split_once('|')
                            && let Ok(id) = id_s.parse::<u64>()
                        {
                            let _ = ack_tx.send((id, act.trim() == "CANCEL"));
                        }
                        continue;
                    }
                    if let Some(ft_s) = cmd.strip_prefix("@PM FASTTICK|") {
                        let en = ft_s.trim() == "1";
                        if let Ok(mut list) = plugins_ref.lock()
                            && let Some(pl) = list.iter_mut().find(|x| x.name == pname)
                        {
                            pl.fast_tick = en;
                        }
                        continue;
                    }
                    if let Some(prio_s) = cmd.strip_prefix("@PM PRIORITY|") {
                        let pr = match prio_s.trim() {
                            "HIGH" => 0,
                            "LOW" => 2,
                            _ => 1,
                        };
                        if let Ok(mut list) = plugins_ref.lock()
                            && let Some(pl) = list.iter_mut().find(|x| x.name == pname)
                        {
                            pl.priority = pr;
                        }
                        continue;
                    }
                    if rate_win.elapsed() >= Duration::from_secs(1) {
                        rate_win = Instant::now();
                        rate_cnt = 0;
                    }
                    let is_vis_cleanup = cmd.starts_with("clear_visual_") || cmd.starts_with("remove_visual_");
                    if !is_vis_cleanup {
                        rate_cnt += 1;
                        if rate_cnt > 240 {
                            continue;
                        }
                    }
                    if let Some(rest) = cmd.strip_prefix("@DB ") {
                        let parts: Vec<&str> = rest.split('|').collect();
                        match parts.first().copied() {
                            Some("ADD_ACC") => {
                                if let (Some(acc), Some(k), Some(d)) =
                                    (parts.get(1), parts.get(2), parts.get(3))
                                    && let (Ok(delta), Ok(mut map)) = (d.parse::<i64>(), db.lock())
                                {
                                    let fk = format!("{}:{}", acc, k);
                                    let cur: i64 =
                                        map.get(&fk).and_then(|v| v.parse().ok()).unwrap_or(0);
                                    map.insert(fk, (cur + delta).max(0).to_string());
                                    save_db(&map);
                                }
                            }
                            Some("XFER") => {
                                if let (Some(req), Some(from_a), Some(to_a), Some(k), Some(amt_s)) = (
                                    parts.get(1),
                                    parts.get(2),
                                    parts.get(3),
                                    parts.get(4),
                                    parts.get(5),
                                ) && let (Ok(amt), Ok(mut map)) =
                                    (amt_s.parse::<i64>(), db.lock())
                                {
                                    let fk_from = format!("{}:{}", from_a, k);
                                    let fk_to = format!("{}:{}", to_a, k);
                                    let bal_from: i64 =
                                        map.get(&fk_from).and_then(|v| v.parse().ok()).unwrap_or(0);
                                    let status = if amt > 0 && bal_from >= amt {
                                        let bal_to: i64 = map
                                            .get(&fk_to)
                                            .and_then(|v| v.parse().ok())
                                            .unwrap_or(0);
                                        map.insert(fk_from, (bal_from - amt).to_string());
                                        map.insert(fk_to, (bal_to + amt).to_string());
                                        save_db(&map);
                                        "OK"
                                    } else {
                                        "INSUFFICIENT"
                                    };
                                    if let Ok(mut list) = plugins_ref.lock()
                                        && let Some(pl) = list.iter_mut().find(|x| x.name == pname)
                                        && let Some(ref mut sin) = pl.stdin
                                    {
                                        let _ = writeln!(sin, "DB_VAL|{}|{}", req, status);
                                    }
                                }
                            }
                            Some("ADD") => {
                                if let (Some(k), Some(d)) = (parts.get(1), parts.get(2))
                                    && let (Ok(delta), Ok(mut map)) = (d.parse::<i64>(), db.lock())
                                {
                                    let cur: i64 =
                                        map.get(*k).and_then(|v| v.parse().ok()).unwrap_or(0);
                                    map.insert(k.to_string(), (cur + delta).max(0).to_string());
                                    save_db(&map);
                                }
                            }
                            Some("SET") => {
                                if let (Some(k), Some(v)) = (parts.get(1), parts.get(2)) {
                                    if let Ok(mut m) = db.lock() {
                                        m.insert(k.to_string(), v.to_string());
                                        save_db(&m);
                                    }
                                    if let Some(acc) = k.strip_prefix("role_acc:") {
                                        dispatch_event(
                                            &plugins_ref,
                                            &format!("ROLE|{}|{}", acc, v),
                                            &mut 0,
                                        );
                                    }
                                }
                            }
                            Some("GET") => {
                                if let (Some(req), Some(k)) = (parts.get(1), parts.get(2)) {
                                    let val = db
                                        .lock()
                                        .ok()
                                        .and_then(|m| m.get(*k).cloned())
                                        .unwrap_or_default();
                                    if let Ok(mut list) = plugins_ref.lock()
                                        && let Some(pl) = list.iter_mut().find(|x| x.name == pname)
                                        && let Some(ref mut sin) = pl.stdin
                                    {
                                        let _ = writeln!(sin, "DB_VAL|{}|{}", req, val);
                                    }
                                }
                            }
                            Some("DEL") => {
                                if let Some(k) = parts.get(1) {
                                    if let Ok(mut m) = db.lock()
                                        && m.remove(*k).is_some()
                                    {
                                        save_db(&m);
                                    }
                                    if let Some(acc) = k.strip_prefix("role_acc:") {
                                        dispatch_event(
                                            &plugins_ref,
                                            &format!("ROLE|{}|", acc),
                                            &mut 0,
                                        );
                                    }
                                }
                            }
                            Some("DEL_PREFIX") => {
                                if let Some(pref) = parts.get(1)
                                    && let Ok(mut m) = db.lock()
                                {
                                    let before = m.len();
                                    m.retain(|k, _| !k.starts_with(*pref));
                                    if m.len() != before {
                                        save_db(&m);
                                    }
                                }
                            }
                            Some("PREFIX") => {
                                if let (Some(req), Some(pref)) = (parts.get(1), parts.get(2)) {
                                    let mut pairs = Vec::new();
                                    if let Ok(m) = db.lock() {
                                        for (k, v) in m.iter() {
                                            if let Some(short_k) = k.strip_prefix(*pref) {
                                                pairs.push(format!("{}={}", short_k, v));
                                            }
                                        }
                                    }
                                    pairs.sort();
                                    if let Ok(mut list) = plugins_ref.lock()
                                        && let Some(pl) = list.iter_mut().find(|x| x.name == pname)
                                        && let Some(ref mut sin) = pl.stdin
                                    {
                                        let _ =
                                            writeln!(sin, "DB_LIST|{}|{}", req, pairs.join(";"));
                                    }
                                }
                            }
                            _ => {}
                        }
                    } else if let Some(ps_rest) = cmd.strip_prefix("@PSTATE ") {
                        let sp: Vec<&str> = ps_rest.splitn(3, '|').collect();
                        match sp.first().copied() {
                            Some("SET") => {
                                if let (Some(k), Some(v)) = (sp.get(1), sp.get(2))
                                    && let Ok(mut m) = db.lock()
                                {
                                    m.insert(format!("pstate:{}:{}", pname, k), v.to_string());
                                    save_db(&m);
                                }
                            }
                            Some("GET") => {
                                if let (Some(req), Some(k)) = (sp.get(1), sp.get(2)) {
                                    let val = db
                                        .lock()
                                        .ok()
                                        .and_then(|m| {
                                            m.get(&format!("pstate:{}:{}", pname, k)).cloned()
                                        })
                                        .unwrap_or_default();
                                    if let Ok(mut list) = plugins_ref.lock()
                                        && let Some(pl) = list.iter_mut().find(|x| x.name == pname)
                                        && let Some(ref mut sin) = pl.stdin
                                    {
                                        let _ = writeln!(sin, "DB_VAL|{}|{}", req, val);
                                    }
                                }
                            }
                            _ => {}
                        }
                    } else if let Some(sc_rest) = cmd.strip_prefix("@SCHED ") {
                        let sp: Vec<&str> = sc_rest.split('|').collect();
                        if let Ok(mut st) = sched_tasks().lock() {
                            match sp.first().copied() {
                                Some("AFTER") => {
                                    if let (Some(ms_s), Some(id_s)) = (sp.get(1), sp.get(2))
                                        && let (Ok(ms), Ok(cb_id)) =
                                            (ms_s.parse::<u64>(), id_s.parse::<u64>())
                                    {
                                        st.retain(|t| !(t.pname == pname && t.cb_id == cb_id));
                                        st.push(SchedTask {
                                            pname: pname.clone(),
                                            cb_id,
                                            next_run: Instant::now() + Duration::from_millis(ms),
                                            interval: None,
                                        });
                                    }
                                }
                                Some("EVERY") => {
                                    if let (Some(ms_s), Some(id_s)) = (sp.get(1), sp.get(2))
                                        && let (Ok(ms), Ok(cb_id)) =
                                            (ms_s.parse::<u64>(), id_s.parse::<u64>())
                                        && ms > 0
                                    {
                                        st.retain(|t| !(t.pname == pname && t.cb_id == cb_id));
                                        let dur = Duration::from_millis(ms);
                                        st.push(SchedTask {
                                            pname: pname.clone(),
                                            cb_id,
                                            next_run: Instant::now() + dur,
                                            interval: Some(dur),
                                        });
                                    }
                                }
                                Some("CANCEL") => {
                                    if let Some(id_s) = sp.get(1)
                                        && let Ok(cb_id) = id_s.parse::<u64>()
                                    {
                                        st.retain(|t| !(t.pname == pname && t.cb_id == cb_id));
                                    }
                                }
                                _ => {}
                            }
                            let cnt = st.iter().filter(|t| t.pname == pname).count();
                            if let Ok(mut list) = plugins_ref.lock()
                                && let Some(pl) = list.iter_mut().find(|x| x.name == pname)
                            {
                                pl.metrics.scheduled_count = cnt;
                            }
                        }
                    } else if let Some(to_rest) = cmd.strip_prefix("@TO ") {
                        if let Ok(mut list) = plugins_ref.lock() {
                            if let Some(me) = list.iter_mut().find(|x| x.name == pname) {
                                me.metrics.cur_ipc += 1;
                            }
                            if let Some((target, payload)) = to_rest.split_once('|') {
                                let t_clean = target.trim();
                                for pl in list.iter_mut() {
                                    if pl.enabled
                                        && pl.name == t_clean
                                        && let Some(ref mut sin) = pl.stdin
                                    {
                                        let _ = writeln!(sin, "IPC|{}|{}", pname, payload);
                                    }
                                }
                            }
                        }
                    } else if let Some(payload) = cmd.strip_prefix("@BUS ") {
                        if let Ok(mut list) = plugins_ref.lock() {
                            if let Some(me) = list.iter_mut().find(|x| x.name == pname) {
                                me.metrics.cur_ipc += 1;
                            }
                            for pl in list.iter_mut() {
                                if pl.enabled
                                    && let Some(ref mut sin) = pl.stdin
                                {
                                    let _ = writeln!(sin, "IPC|{}|{}", pname, payload);
                                }
                            }
                            if payload == "DISCOVER" {
                                let regs: Vec<String> = list.iter().filter(|p| p.enabled).filter_map(|p| p.reg_info.clone()).collect();
                                if let Some(hub) = list.iter_mut().find(|x| x.enabled && x.name == "plugin_hub")
                                    && let Some(ref mut sin) = hub.stdin
                                {
                                    for r in regs { let _ = writeln!(sin, "IPC|manager|{}", r); }
                                }
                            }
                        }
                    } else {
                        if let Some(need) = required_permission(cmd) {
                            let ok = plugins_ref
                                .lock()
                                .ok()
                                .and_then(|l| {
                                    l.iter()
                                        .find(|x| x.name == pname)
                                        .map(|pl| pl.permissions.contains(need))
                                })
                                .unwrap_or(false);
                            if !ok {
                                log_plugin_err(
                                    &pname,
                                    &format!("PermissionDenied: '{}' requires '{}'", cmd, need),
                                );
                                continue;
                            }
                        }
                        if (cmd.starts_with("spawn_fx ") || cmd.starts_with("spawn_laser"))
                            && let Ok(mut list) = plugins_ref.lock()
                            && let Some(me) = list.iter_mut().find(|x| x.name == pname)
                        {
                            me.metrics.cur_fx += 1;
                        }
                        if let Ok(mut reg) = reg_ref.lock() {
                            reg.track_command(&pname, cmd);
                        }
                        for fc in remap_visual_cmd(&pname, cmd) {
                            send_fifo(&fc);
                        }
                    }
                }
            });
        }
        p.child = Some(child);
        p.stdin = stdin;
        p.ack_rx = Some(ack_rx);
        p.unload_rx = Some(unload_rx);
        p.last_pong = Instant::now();
        if let Some(ref mut sin) = p.stdin {
            if let Ok(m) = db_ref.lock() {
                for (dk, dv) in m.iter() {
                    if let Some(acc) = dk.strip_prefix("role_acc:") {
                        let _ = writeln!(sin, "ROLE|{}|{}", acc, dv);
                    }
                }
            }
            if let Ok(sm) = sessions.lock() { for (nick, info) in sm.iter() { if !info.account.is_empty() { let _ = writeln!(sin, "AUTH|{}|{}|{}", info.cid, nick, info.account); } } }
            let _ = writeln!(
                sin,
                "LIFECYCLE|{}",
                if is_reload { "RELOAD" } else { "LOAD" }
            );
        }
        log_pm(&format!(
            "[Started] Плагин '{}' запущен (prio={}, reload={}).",
            p.name, p.priority, is_reload
        ));
    }
}

fn dispatch_event(plugins: &Arc<Mutex<Vec<ManagedPlugin>>>, ev_line: &str, ev_seq: &mut u64) {
    let mut order: Vec<usize> = Vec::new();
    if let Ok(list) = plugins.lock() {
        let mut idx_prio: Vec<(usize, u8)> = list
            .iter()
            .enumerate()
            .filter(|(_, p)| p.enabled)
            .map(|(i, p)| (i, p.priority))
            .collect();
        idx_prio.sort_by_key(|x| x.1);
        order = idx_prio.into_iter().map(|x| x.0).collect();
    }
    *ev_seq += 1;
    let id = *ev_seq;
    let is_tick = ev_line.starts_with("TICK|") || ev_line.starts_with("FTICK|");
    for idx in order {
        let t0 = Instant::now();
        let mut cancelled = false;
        let mut waited = false;
        if let Ok(mut list) = plugins.lock()
            && let Some(pl) = list.get_mut(idx)
            && pl.enabled
            && let Some(ref mut sin) = pl.stdin
        {
            if pl.priority == 0 && ev_line.starts_with("CHAT|") {
                let _ = writeln!(sin, "SYNC|{}|{}", id, ev_line);
                waited = true;
            } else {
                let _ = writeln!(sin, "{}", ev_line);
            }
        }
        if waited {
            let deadline = Instant::now() + Duration::from_millis(5);
            while Instant::now() < deadline {
                let got = plugins.lock().ok().and_then(|l| {
                    l.get(idx)
                        .and_then(|pl| pl.ack_rx.as_ref().and_then(|rx| rx.try_recv().ok()))
                });
                if let Some((ack_id, is_cancel)) = got
                    && ack_id == id
                {
                    cancelled = is_cancel;
                    break;
                }
                thread::sleep(Duration::from_millis(1));
            }
        }
        if is_tick
            && let Ok(mut list) = plugins.lock()
            && let Some(pl) = list.get_mut(idx)
        {
            pl.metrics.record_tick_us(t0.elapsed().as_micros() as u64);
        }
        if cancelled {
            break;
        }
    }
}

fn main() {
    let _ = fs::create_dir_all(spath("plugins"));
    let _ = fs::create_dir_all(spath("logs"));
    let _ = fs::create_dir_all(spath("configs"));
    let db = Arc::new(Mutex::new(load_db()));
    let plugins: Arc<Mutex<Vec<ManagedPlugin>>> = Arc::new(Mutex::new(Vec::new()));
    let registry = Arc::new(Mutex::new(StateRegistry::default()));
    let mut online_players: HashSet<String> = HashSet::new();
    let mut authed_map: HashMap<String, String> = HashMap::new();
    let mut cid_to_nick: HashMap<i32, String> = HashMap::new();
    let mut pending_cid: Option<i32> = None;
    let mut pending_ip = String::new();
    let mut ev_seq: u64 = 0;
    log_pm("[PluginManager v10.0 LTS Meta-Engine] Запуск менеджера...");
    send_fifo("reg_chat_cmd \"plugins\" \"\" \"Список активных плагинов сервера\"");
    send_fifo("reg_chat_cmd \"plugin\" \"r\" \"Управление плагинами и справка\"");
    let sessions: Arc<Mutex<HashMap<String, SessionInfo>>> = Arc::new(Mutex::new(HashMap::new()));
    let mut last_fast_tick = 0u64;
    let (sess_wd, p_wd, db_wd, reg_wd) = (
        Arc::clone(&sessions),
        Arc::clone(&plugins),
        Arc::clone(&db),
        Arc::clone(&registry),
    );
    thread::spawn(move || {
        loop {
            if let Ok(entries) = fs::read_dir(spath("plugins")) {
                let (mut found, mut reloads) = (HashMap::new(), HashSet::new());
                for e in entries.flatten() {
                    if e.path().is_file()
                        && let Some(name) = e.path().file_name().and_then(|n| n.to_str())
                    {
                        if let Some(t) = name.strip_prefix(".reload_") {
                            reloads.insert(t.to_string());
                            let _ = fs::remove_file(e.path());
                        } else if !name.starts_with('.') && !name.ends_with(".disabled") {
                            found.insert(name.to_string(), e.path().to_string_lossy().to_string());
                        }
                    }
                }
                if let Ok(mut list) = p_wd.lock() {
                    list.retain_mut(|p| {
                        if !found.contains_key(&p.name) {
                            stop_plugin_clean(p, &reg_wd);
                            false
                        } else {
                            true
                        }
                    });
                    for t in &reloads {
                        if let Some(pl) = list.iter_mut().find(|p| &p.name == t) {
                            stop_plugin_clean(pl, &reg_wd);
                            pl.enabled = true;
                            pl.crash_times.clear();
                            pl.permissions.clear();
                            spawn_plugin(
                                pl,
                                Arc::clone(&db_wd),
                                Arc::clone(&p_wd),
                                Arc::clone(&reg_wd),
                                true,
                                Arc::clone(&sess_wd),
                            );
                            if let (Some(sin), Ok(sm), Ok(dm)) = (&mut pl.stdin, sess_wd.lock(), db_wd.lock()) {
                                for (n, inf) in sm.iter() {
                                    let _ = writeln!(sin, "SYNC_PLAYER|{}|{}|{}|{}", n, inf.cid, inf.ip, inf.account);
                                    if !inf.account.is_empty() {
                                        let role = dm.get(&format!("role_acc:{}", inf.account)).cloned().unwrap_or_default();
                                        let _ = writeln!(sin, "ROLE|{}|{}", inf.account, role);
                                    }
                                }
                            }
                        }
                    }
                    for (name, path) in &found {
                        if !list.iter().any(|p| &p.name == name) {
                            let mut np = ManagedPlugin {
                                name: name.clone(),
                                path: path.clone(),
                                fast_tick: false,
                                priority: 1,
                                enabled: true,
                                crash_times: vec![],
                                last_pong: Instant::now(),
                                track_pos: false,
                                reg_info: None,
                                permissions: HashSet::new(),
                                metrics: PluginMetrics::default(),
                                child: None,
                                stdin: None,
                                ack_rx: None,
                                unload_rx: None,
                            };
                            spawn_plugin(
                                &mut np,
                                Arc::clone(&db_wd),
                                Arc::clone(&p_wd),
                                Arc::clone(&reg_wd),
                                false,
                                Arc::clone(&sess_wd),
                            );
                            if let (Some(sin), Ok(sm)) = (&mut np.stdin, sess_wd.lock()) {
                                for (n, inf) in sm.iter() {
                                    let _ = writeln!(
                                        sin,
                                        "SYNC_PLAYER|{}|{}|{}|{}",
                                        n, inf.cid, inf.ip, inf.account
                                    );
                                }
                            }
                            list.push(np);
                        }
                    }
                }
            }
            thread::sleep(Duration::from_secs(1));
        }
    });
    while !Path::new(&spath("logs/server.log")).exists() {
        thread::sleep(Duration::from_millis(500));
    }
    let mut file = File::open(spath("logs/server.log")).expect("open server.log");
    let mut pos = file.seek(SeekFrom::End(0)).unwrap_or(0);
    let start_time = Instant::now();
    let mut last_tick_sec = 0u64;

    loop {
        let len = fs::metadata(spath("logs/server.log")).map(|m| m.len()).unwrap_or(0);
        if len < pos {
            pos = 0;
            file = File::open(spath("logs/server.log")).expect("reopen");
        }
        if len > pos {
            let _ = file.seek(SeekFrom::Start(pos));
            let mut buf = String::new();
            if file.read_to_string(&mut buf).is_ok() {
                pos += buf.len() as u64;
                for line in buf.lines() {
                    if let Some(p_idx) = line.find("ClientId=")
                        && let Some(cid) = line[p_idx + 9..]
                            .split_whitespace()
                            .next()
                            .and_then(|s| s.parse::<i32>().ok())
                    {
                        pending_cid = Some(cid);
                    }
                    let mut ev = parse_log_event(line);
                    if !ev.is_empty() {
                        if let Some(rest) = ev.strip_prefix("AUTH|") {
                            let sp: Vec<&str> = rest.splitn(3, '|').collect();
                            if let (Some(nick), Some(acc)) = (sp.get(1), sp.get(2)) {
                                authed_map.insert(nick.to_string(), acc.to_string());
                                if let Ok(mut sm) = sessions.lock() {
                                    if let Some(c) = sp.first().and_then(|x| x.parse::<i32>().ok()) { if c >= 0 { cid_to_nick.insert(c, nick.to_string()); sm.entry(nick.to_string()).or_default().cid = c; } } sm.entry(nick.to_string()).or_default().account =
                                        acc.to_string();
                                }
                            }
                        } else if let Some(rest) = ev.strip_prefix("CHAT|") {
                            let sp: Vec<&str> = rest.splitn(3, '|').collect();
                            if let (Some(nick), Some(msg)) = (sp.get(1), sp.get(2))
                                && msg.starts_with("!pm")
                            {
                                let is_owner = authed_map
                                    .get(*nick)
                                    .and_then(|acc| {
                                        db.lock().ok().and_then(|m| {
                                            m.get(&format!("role_acc:{}", acc)).cloned()
                                        })
                                    })
                                    .map(|r| r == "owner")
                                    .unwrap_or(false);
                                if is_owner {
                                    handle_pm_cmd(msg, &plugins, &db, &registry, &sessions);
                                }
                                continue;
                            }
                        } else if let Some(st_rest) = ev.strip_prefix("STATE|") { if let Some(cid) = st_rest.split('|').next().and_then(|x| x.parse::<i32>().ok()) { cid_to_nick.entry(cid).or_default(); } } else if let Some(nick) = ev.strip_prefix("JOIN|") {
                            online_players.insert(nick.to_string());
                            let cid = pending_cid.take().unwrap_or(-1);
                            let ip = std::mem::take(&mut pending_ip);
                            if cid >= 0 {
                                cid_to_nick.insert(cid, nick.to_string());
                            }
                            if let Ok(mut sm) = sessions.lock() {
                                sm.insert(
                                    nick.to_string(),
                                    SessionInfo {
                                        cid,
                                        ip: ip.clone(),
                                        account: String::new(),
                                    },
                                );
                            }
                            ev = format!("JOIN|{}|{}|{}", nick, cid, ip);
                        } else if let Some(rn_rest) = ev.strip_prefix("RENAME|")
                            && let Some((old_n, new_n)) = rn_rest.split_once('|')
                        {
                            if online_players.remove(old_n) {
                                online_players.insert(new_n.to_string());
                            }
                            if let Some(acc) = authed_map.remove(old_n) {
                                authed_map.insert(new_n.to_string(), acc);
                            }
                            for v in cid_to_nick.values_mut() {
                                if v == old_n {
                                    *v = new_n.to_string();
                                }
                            }
                            if let Ok(mut sm) = sessions.lock()
                                && let Some(info) = sm.remove(old_n)
                            {
                                sm.insert(new_n.to_string(), info);
                            }
                        } else if let Some(nick) = ev.strip_prefix("LEAVE|") {
                            online_players.remove(nick); cid_to_nick.retain(|_, v| !v.is_empty() && v != nick);
                            authed_map.remove(nick);
                            if let Ok(mut sm) = sessions.lock() {
                                sm.remove(nick);
                            }
                        }
                        dispatch_event(&plugins, &ev, &mut ev_seq);
                    }
                }
            }
        }
        {
            let now = Instant::now();
            let mut due: Vec<(String, u64)> = Vec::new();
            let mut counts: HashMap<String, usize> = HashMap::new();
            if let Ok(mut st) = sched_tasks().lock() {
                st.retain_mut(|t| {
                    if now >= t.next_run {
                        due.push((t.pname.clone(), t.cb_id));
                        if let Some(iv) = t.interval {
                            t.next_run = now + iv;
                            *counts.entry(t.pname.clone()).or_insert(0) += 1;
                            true
                        } else {
                            false
                        }
                    } else {
                        *counts.entry(t.pname.clone()).or_insert(0) += 1;
                        true
                    }
                });
            }
            if !due.is_empty()
                && let Ok(mut list) = plugins.lock()
            {
                for pl in list.iter_mut() {
                    pl.metrics.scheduled_count = *counts.get(&pl.name).unwrap_or(&0);
                }
                for (target_p, cb_id) in due {
                    if let Some(pl) = list.iter_mut().find(|x| x.enabled && x.name == target_p)
                        && let Some(ref mut sin) = pl.stdin
                    {
                        let _ = writeln!(sin, "SCHED|{}", cb_id);
                    }
                }
            }
        }
        let elapsed_ms = start_time.elapsed().as_millis() as u64;
        let fb = elapsed_ms / 100;
        if fb > last_fast_tick {
            last_fast_tick = fb;
            if let Ok(mut list) = plugins.lock() {
                for pl in list.iter_mut() {
                    if pl.enabled
                        && pl.fast_tick
                        && let Some(ref mut sin) = pl.stdin
                    {
                        let _ = writeln!(sin, "FTICK|{}", elapsed_ms);
                    }
                }
                if list.iter().any(|x| x.enabled && x.track_pos) {
                    if cid_to_nick.is_empty() { if elapsed_ms / 1000 > last_tick_sec { for cid in 0..64 { send_fifo(&format!("get_state {}", cid)); } } } else { for &cid in cid_to_nick.keys() { send_fifo(&format!("get_state {}", cid)); } }
                }
            }
        }
        let elapsed = elapsed_ms / 1000;
        if elapsed > last_tick_sec {
            if elapsed.is_multiple_of(5) {
                dispatch_event(&plugins, "PING", &mut ev_seq);
            }
            let mut m_sum = Vec::new();
            if let Ok(mut list) = plugins.lock() {
                for pl in list.iter_mut() {
                    if !pl.enabled {
                        continue;
                    }
                    let (warn, kill_lim) = pl.metrics.roll_second();
                    if warn {
                        log_plugin_err(
                            &pl.name,
                            &format!(
                                "RateLimitWarning: fx={}/s, ipc={}/s",
                                pl.metrics.last_fx_sec, pl.metrics.last_ipc_sec
                            ),
                        );
                    }
                    if kill_lim {
                        pl.enabled = false;
                        stop_plugin_clean(pl, &registry);
                        log_plugin_err(
                            &pl.name,
                            "AutoDisabled: systematic rate-limit violation (3s)",
                        );
                    } else if elapsed.is_multiple_of(10) {
                        m_sum.push(pl.metrics.format_summary(&pl.name));
                    }
                }
            }
            if elapsed.is_multiple_of(10) && !m_sum.is_empty() {
                let joined = m_sum.join(" | ");
                log_pm(&format!("[Metrics] {}", joined));
                dispatch_event(
                    &plugins,
                    &format!("IPC|manager|METRICS|{}", joined),
                    &mut ev_seq,
                );
            }
            last_tick_sec = elapsed;
            let mut list: Vec<String> = online_players.iter().cloned().collect();
            list.sort();
            dispatch_event(&plugins, &format!("ONLINE|{}", list.join(",")), &mut ev_seq);
            dispatch_event(&plugins, &format!("TICK|{}", elapsed), &mut ev_seq);
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn handle_pm_cmd(
    msg: &str,
    plugins: &Arc<Mutex<Vec<ManagedPlugin>>>,
    db: &Arc<Mutex<HashMap<String, String>>>,
    registry: &Arc<Mutex<StateRegistry>>,
    sessions: &Arc<Mutex<HashMap<String, SessionInfo>>>,
) {
    let args: Vec<&str> = msg.split_whitespace().collect();
    let sub = args.get(1).copied().unwrap_or("list");
    let target = args.get(2).copied().unwrap_or("");
    if let Ok(mut list) = plugins.lock() {
        match sub {
            "list" => {
                let info: Vec<String> = list
                    .iter()
                    .map(|p| format!("{}[{}]", p.name, if p.enabled { "ON" } else { "OFF" }))
                    .collect();
                send_fifo(&format!("say \"[PlugMan] {}\"", info.join(", ")));
            }
            "disable" => {
                if let Some(pl) = list.iter_mut().find(|p| p.name == target) {
                    pl.enabled = false;
                    stop_plugin_clean(pl, registry);
                    send_fifo(&format!("say \"[PlugMan] Плагин '{}' выключен.\"", target));
                }
            }
            "enable" => {
                if let Some(pl) = list.iter_mut().find(|p| p.name == target) {
                    pl.enabled = true;
                    pl.crash_times.clear();
                    pl.permissions.clear();
                    spawn_plugin(
                        pl,
                        Arc::clone(db),
                        Arc::clone(plugins),
                        Arc::clone(registry),
                        false,
                        Arc::clone(sessions),
                    );
                    send_fifo(&format!("say \"[PlugMan] Плагин '{}' включён.\"", target));
                }
            }
            "reload" => {
                if let Some(pl) = list.iter_mut().find(|p| p.name == target) {
                    stop_plugin_clean(pl, registry);
                    pl.enabled = true;
                    pl.crash_times.clear();
                    pl.permissions.clear();
                    spawn_plugin(
                        pl,
                        Arc::clone(db),
                        Arc::clone(plugins),
                        Arc::clone(registry),
                        true,
                        Arc::clone(sessions),
                    );
                    if let (Some(sin), Ok(sm)) = (&mut pl.stdin, sessions.lock()) {
                        for (n, inf) in sm.iter() {
                            let _ = writeln!(
                                sin,
                                "SYNC_PLAYER|{}|{}|{}|{}",
                                n, inf.cid, inf.ip, inf.account
                            );
                        }
                    }
                    send_fifo(&format!(
                        "say \"[PlugMan] Плагин '{}' перезагружен (StateRollback).\"",
                        target
                    ));
                }
            }
            _ => {}
        }
    }
}

struct VisPools {
    pickups: HashMap<(String, i32), usize>,
    lasers: HashMap<(String, i32), usize>,
    tees: HashMap<(String, i32), usize>,
    texts: HashMap<(String, i32), usize>,
}

fn vis_pools() -> &'static Mutex<VisPools> {
    static POOLS: std::sync::OnceLock<Mutex<VisPools>> = std::sync::OnceLock::new();
    POOLS.get_or_init(|| {
        Mutex::new(VisPools {
            pickups: HashMap::new(),
            lasers: HashMap::new(),
            tees: HashMap::new(),
            texts: HashMap::new(),
        })
    })
}

fn alloc_slot_cap(
    map: &mut HashMap<(String, i32), usize>,
    pname: &str,
    lid: i32,
    cap: usize,
) -> Option<usize> {
    if let Some(&s) = map.get(&(pname.to_string(), lid)) {
        return Some(s);
    }
    let used: HashSet<usize> = map.values().copied().collect();
    (0..cap).find(|s| !used.contains(s)).inspect(|&s| {
        map.insert((pname.to_string(), lid), s);
    })
}

fn alloc_slot(map: &mut HashMap<(String, i32), usize>, pname: &str, lid: i32) -> Option<usize> {
    alloc_slot_cap(map, pname, lid, 128)
}

fn remap_visual_cmd(pname: &str, cmd: &str) -> Vec<String> {
    let p: Vec<&str> = cmd.split_whitespace().collect();
    let mut pools = vis_pools().lock().unwrap();
    match p.first().copied() {
        Some("spawn_visual_pickup") if p.len() == 6 => {
            if let Some(Ok(lid)) = p.get(1).map(|s| s.parse::<i32>())
                && let Some(s) = alloc_slot(&mut pools.pickups, pname, lid)
            {
                return vec![format!(
                    "spawn_visual_pickup {} {} {} {} {}",
                    s,
                    p.get(2).unwrap_or(&"0"),
                    p.get(3).unwrap_or(&"0"),
                    p.get(4).unwrap_or(&"0"),
                    p.get(5).unwrap_or(&"0")
                )];
            }
            vec![]
        }
        Some("spawn_visual_pickup_follow") if p.len() == 8 => {
            if let Some(Ok(lid)) = p.get(1).map(|s| s.parse::<i32>())
                && let Some(s) = alloc_slot(&mut pools.pickups, pname, lid)
            {
                return vec![format!(
                    "spawn_visual_pickup_follow {} {} {} {} {} {} {}",
                    s,
                    p.get(2).unwrap_or(&"0"),
                    p.get(3).unwrap_or(&"0"),
                    p.get(4).unwrap_or(&"0"),
                    p.get(5).unwrap_or(&"0"),
                    p.get(6).unwrap_or(&"0"),
                    p.get(7).unwrap_or(&"0")
                )];
            }
            vec![]
        }
        Some("remove_visual_pickup") if p.len() == 2 => {
            if let Some(Ok(lid)) = p.get(1).map(|s| s.parse::<i32>())
                && let Some(s) = pools.pickups.remove(&(pname.to_string(), lid))
            {
                return vec![format!("remove_visual_pickup {}", s)];
            }
            vec![]
        }
        Some("spawn_visual_laser") if p.len() == 7 => {
            if let Some(Ok(lid)) = p.get(1).map(|s| s.parse::<i32>())
                && let Some(s) = alloc_slot(&mut pools.lasers, pname, lid)
            {
                return vec![format!(
                    "spawn_visual_laser {} {} {} {} {} {}",
                    s,
                    p.get(2).unwrap_or(&"0"),
                    p.get(3).unwrap_or(&"0"),
                    p.get(4).unwrap_or(&"0"),
                    p.get(5).unwrap_or(&"0"),
                    p.get(6).unwrap_or(&"0")
                )];
            }
            vec![]
        }
        Some("remove_visual_laser") if p.len() == 2 => {
            if let Some(Ok(lid)) = p.get(1).map(|s| s.parse::<i32>())
                && let Some(s) = pools.lasers.remove(&(pname.to_string(), lid))
            {
                return vec![format!("remove_visual_laser {}", s)];
            }
            vec![]
        }
        Some("clear_visual_lasers") => {
            let mut out = Vec::new();
            pools.lasers.retain(|(pl, _), &mut s| {
                if pl == pname {
                    out.push(format!("remove_visual_laser {}", s));
                    false
                } else {
                    true
                }
            });
            out
        }
        Some("spawn_visual_tee") => {
            let parts: Vec<&str> = cmd.splitn(8, ' ').collect();
            if parts.len() == 8
                && let Ok(lid) = parts.get(1).copied().unwrap_or("").parse::<i32>()
                && let Some(s) = alloc_slot_cap(&mut pools.tees, pname, lid, 64)
            {
                return vec![format!(
                    "spawn_visual_tee {} {} {} {} {} {} {}",
                    s,
                    parts.get(2).copied().unwrap_or("0"),
                    parts.get(3).copied().unwrap_or("0"),
                    parts.get(4).copied().unwrap_or("\"default\""),
                    parts.get(5).copied().unwrap_or("0"),
                    parts.get(6).copied().unwrap_or("0"),
                    parts.get(7).copied().unwrap_or("0")
                )];
            }
            vec![]
        }
        Some("remove_visual_tee") if p.len() == 2 => {
            if let Some(Ok(lid)) = p.get(1).map(|s| s.parse::<i32>())
                && let Some(s) = pools.tees.remove(&(pname.to_string(), lid))
            {
                return vec![format!("remove_visual_tee {}", s)];
            }
            vec![]
        }
        Some("clear_visual_tees") => {
            let mut out = Vec::new();
            pools.tees.retain(|(pl, _), &mut s| {
                if pl == pname {
                    out.push(format!("remove_visual_tee {}", s));
                    false
                } else {
                    true
                }
            });
            out
        }
        Some("spawn_visual_text") => {
            let parts: Vec<&str> = cmd.splitn(7, ' ').collect();
            if parts.len() == 7
                && let Ok(lid) = parts.get(1).copied().unwrap_or("").parse::<i32>()
                && let Some(s) = alloc_slot_cap(&mut pools.texts, pname, lid, 32)
            {
                return vec![format!(
                    "spawn_visual_text {} {} {} {} {} {}",
                    s,
                    parts.get(2).copied().unwrap_or("0"),
                    parts.get(3).copied().unwrap_or("0"),
                    parts.get(4).copied().unwrap_or("1"),
                    parts.get(5).copied().unwrap_or("0"),
                    parts.get(6).copied().unwrap_or("\"\"")
                )];
            }
            vec![]
        }
        Some("remove_visual_text") if p.len() == 2 => {
            if let Some(Ok(lid)) = p.get(1).map(|s| s.parse::<i32>())
                && let Some(s) = pools.texts.remove(&(pname.to_string(), lid))
            {
                return vec![format!("remove_visual_text {}", s)];
            }
            vec![]
        }
        Some("clear_visual_texts") => {
            let mut out = Vec::new();
            pools.texts.retain(|(pl, _), &mut s| {
                if pl == pname {
                    out.push(format!("remove_visual_text {}", s));
                    false
                } else {
                    true
                }
            });
            out
        }
        _ => vec![cmd.to_string()],
    }
}

fn cleanup_visuals(pname: &str) -> Vec<String> {
    let mut pools = vis_pools().lock().unwrap();
    let mut out = Vec::new();
    pools.pickups.retain(|(p, _), &mut s| {
        if p == pname {
            out.push(format!("remove_visual_pickup {}", s));
            false
        } else {
            true
        }
    });
    pools.lasers.retain(|(p, _), &mut s| {
        if p == pname {
            out.push(format!("remove_visual_laser {}", s));
            false
        } else {
            true
        }
    });
    pools.tees.retain(|(p, _), &mut s| {
        if p == pname {
            out.push(format!("remove_visual_tee {}", s));
            false
        } else {
            true
        }
    });
    pools.texts.retain(|(p, _), &mut s| {
        if p == pname {
            out.push(format!("remove_visual_text {}", s));
            false
        } else {
            true
        }
    });
    out
}
