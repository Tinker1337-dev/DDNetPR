use ddnet_sdk::{Event, ServerApi};
use std::collections::HashMap;
struct BrushUser { active: bool, style: i32, tx: f32, ty: f32, last_pt: Option<(f32, f32)>, prev_jump: bool, prev_hook: bool }
fn style_name(s: i32) -> &'static str { match s { 2 => "Зеленый лазер", 3 => "Сердечки (вечные)", 4 => "Щиты брони (вечные)", 5 => "Огненный шлейф", 6 => "Размораживающий лазер", _ => "Синий лазер" } }
fn dist_sq_seg(px: f32, py: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 { let (dx, dy) = (x2 - x1, y2 - y1); let len2 = dx * dx + dy * dy; let t = if len2 < 0.001 { 0.0 } else { (((px - x1) * dx + (py - y1) * dy) / len2).clamp(0.0, 1.0) }; let (cx, cy) = (x1 + t * dx, y1 + t * dy); (px - cx) * (px - cx) + (py - cy) * (py - cy) }
fn main() {
    let mut api = ServerApi::new();
    api.register_plugin("brush", "!brush [1-6|clear] — кисть v11.7");
    api.register_cmd("brush", "[1-6|clear]", "Кисть: 1-синий, 2-зеленый, 3-сердца, 4-броня, 5-огонь, 6-антифриз, clear");
    let mut users: HashMap<i32, BrushUser> = HashMap::new();
    let mut segs: Vec<(i32, f32, f32, f32, f32)> = Vec::new();
    let mut vis_ids: Vec<i32> = Vec::new();
    let mut rr_idx: usize = 0;
    api.run(move |api, ev| {
        match ev {
            Event::Chat { player, message, .. } => { let _ = std::fs::write("/tmp/brush_dbg.txt", format!("p={:?} m={:?} staff={} cids={:?} accs={:?}", player, message, api.is_staff(&player), api.cids, api.accounts));
                let msg = message.trim();
                if msg == "!brush" || msg.starts_with("!brush ") {
                    let cid = api.player_cids.get(&player).or_else(|| api.cids.get(&player)).copied().unwrap_or(0); if !api.is_staff(&player) && !std::fs::read_to_string("build_admins.txt").unwrap_or_default().lines().any(|l| !l.trim().is_empty() && player.contains(l.trim())) { api.broadcast_to(cid, "[Staff] Сначала войдите в аккаунт (/login) с ролью Staff/Admin!"); return; }
                    let arg_str = msg.strip_prefix("!brush").unwrap_or("").trim();
                    if arg_str.eq_ignore_ascii_case("clear") {
                        segs.clear(); api.clear_visual_lasers();
                        for id in vis_ids.drain(..) { api.remove_visual_pickup(id); }
                        rr_idx = 0; if let Some(u) = users.get_mut(&cid) { u.last_pt = None; }
                        api.enable_fast_tick(false); api.broadcast_to(cid, "Полотно кисти очищено."); return;
                    }
                    let st = users.entry(cid).or_insert(BrushUser { active: false, style: 1, tx: 0.0, ty: 0.0, last_pt: None, prev_jump: false, prev_hook: false });
                    if let Ok(s) = arg_str.parse::<i32>() { st.style = s.clamp(1, 6); st.active = true; } else { st.active = !st.active; }
                    st.last_pt = None;
                    if st.active { api.broadcast_to(cid, &format!("Кисть ВКЛ [{}]: ЛКМ - рисовать, Пробел - разрыв, ПКМ - стереть.", style_name(st.style))); } else { api.broadcast_to(cid, "Кисть ВЫКЛ."); }
                    let any_active = users.values().any(|u| u.active);
                    let need_fast = segs.iter().any(|&(s, ..)| s == 5 || s == 6);
                    api.track_positions(any_active || need_fast); api.enable_fast_tick(need_fast); return;
                }
            }
            Event::Input { cid, fire, jump, hook, target_x, target_y, .. } => {
                if let Some(st) = users.get_mut(&cid) && st.active {
                    st.tx = target_x as f32; st.ty = target_y as f32;
                    let (hook_now, jump_now, fire_now) = (hook != 0, jump != 0, (fire & 1) != 0);
                    if hook_now && !st.prev_hook { api.reset_hook(cid); segs.clear(); api.clear_visual_lasers(); for id in vis_ids.drain(..) { api.remove_visual_pickup(id); } rr_idx = 0; st.last_pt = None; api.play_sound(cid, 30); api.enable_fast_tick(false); }
                    else if jump_now && !st.prev_jump { st.last_pt = None; api.play_sound(cid, 37); }
                    else if fire_now {
                        if let Some(pos) = api.states.get(&cid) {
                            let (px, py) = (pos.x + st.tx, pos.y + st.ty);
                            let step = if st.style == 3 || st.style == 4 { 26.0 } else { 14.0 };
                            if st.last_pt.map(|(lx, ly)| (px - lx).hypot(py - ly) >= step).unwrap_or(true) {
                                if st.style == 3 || st.style == 4 { if vis_ids.len() < 128 { let id = vis_ids.len() as i32; api.spawn_visual_pickup(id, if st.style == 3 { 0 } else { 1 }, 0, px, py); vis_ids.push(id); } }
                                else if let Some((lx, ly)) = st.last_pt { if segs.len() < 128 { let lid = segs.len() as i32; segs.push((st.style, lx, ly, px, py)); match st.style { 1 => api.spawn_visual_laser(lid, lx, ly, px, py, 0), 2 | 6 => api.spawn_visual_laser(lid, lx, ly, px, py, 1), 5 => api.spawn_fx("explosion", px, py), _ => {} } api.enable_fast_tick(segs.iter().any(|&(s, ..)| s == 5 || s == 6)); } }
                                st.last_pt = Some((px, py));
                            }
                        } else { }
                    }
                    st.prev_jump = jump_now; st.prev_hook = hook_now;
                }
            }
            Event::FastTick { .. } => { if !segs.is_empty() { for _ in 0..segs.len().min(4) { if rr_idx >= segs.len() { rr_idx = 0; } let (s, _, _, x2, y2) = segs[rr_idx]; if s == 5 { api.spawn_fx("explosion", x2, y2); } rr_idx += 1; } for (&vcid, vst) in &api.states { if vst.frozen != 0 && segs.iter().any(|&(s, x1, y1, x2, y2)| s == 6 && dist_sq_seg(vst.x, vst.y, x1, y1, x2, y2) < 2304.0) { api.set_freeze(vcid, 0); api.set_vel(vcid, vst.vx, -5.0); api.play_sound(vcid, 37); } } } }
            Event::Tick { .. } => { if users.values().any(|u| u.active) || segs.iter().any(|&(s, ..)| s == 5 || s == 6) { api.track_positions(true); } }
            Event::Leave { .. } => { if let Some(cid) = api.last_leave_cid { users.remove(&cid); } if !api.names.is_empty() { users.retain(|k, _| api.names.contains_key(k)); } let nf = segs.iter().any(|&(s, ..)| s == 5 || s == 6); api.track_positions(users.values().any(|u| u.active) || nf); api.enable_fast_tick(nf); }
            _ => {}
        }
        
    });
}
