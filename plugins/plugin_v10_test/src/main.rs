use ddnet_sdk::prelude::*;
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

fn resolve_cid(api: &ServerApi, token: &str) -> Option<i32> {
    if let Ok(cid) = token.parse::<i32>() {
        if api.player_cids.values().any(|&c| c == cid) {
            return Some(cid);
        }
    }
    api.player_cids
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(token))
        .map(|(_, &cid)| cid)
}

fn name_by_cid(api: &ServerApi, cid: i32) -> Option<String> {
    api.player_cids
        .iter()
        .find(|(_, c)| **c == cid)
        .map(|(n, _)| n.clone())
}

fn main() {
    let mut api = ServerApi::new();
    api.set_manifest("11.0.0", &["tile_edit"]);
    api.register_admin_plugin(
        "v10test",
        "!v10test | !v10step4 | !impulse [vx] [vy] | !hookwatch | !skinto <id|nick> [skin]",
    );

    let loads = Rc::new(Cell::new(0u32));
    let reloads = Rc::new(Cell::new(0u32));
    let sched_hits = Rc::new(Cell::new(0u32));
    let watchers = Rc::new(RefCell::new(HashSet::<String>::new()));

    let l_clone = Rc::clone(&loads);
    api.on_load(move |_a| l_clone.set(l_clone.get() + 1));
    let r_clone = Rc::clone(&reloads);
    api.on_reload(move |_a| r_clone.set(r_clone.get() + 1));
    api.on_unload(|_a| {});

    let s_clone = Rc::clone(&sched_hits);
    let w_clone = Rc::clone(&watchers);
    api.run(move |api, ev| match ev {
        Event::Leave { player, .. } => {
            w_clone.borrow_mut().remove(&player);
        }
        Event::HookAttach {
            cid,
            pos,
            hooked_cid,
        } => {
            if let Some(name) = name_by_cid(api, cid) {
                if w_clone.borrow().contains(&name) {
                    api.whisper(
                        &name,
                        &format!(
                            "[HookAttach] hooked_cid={:?} pos=({:.0},{:.0})",
                            hooked_cid, pos.x, pos.y
                        ),
                    );
                }
            }
        }
        Event::HookDetach {
            cid,
            prev_hooked_cid,
        } => {
            if let Some(name) = name_by_cid(api, cid) {
                if w_clone.borrow().contains(&name) {
                    api.whisper(
                        &name,
                        &format!("[HookDetach] prev_hooked_cid={:?}", prev_hooked_cid),
                    );
                }
            }
        }
        Event::Scheduled { callback_id } => {
            s_clone.set(s_clone.get() + 1);
            if callback_id == 402 {
                api.cancel_schedule(402);
            }
        }
        Event::DbValue { req_id, value } if req_id.starts_with("v10_") => {
            api.say(&format!("[v10_step4] db {}={}", req_id, value));
        }
        Event::Chat {
            player, message, ..
        } => {
            if !api.is_staff(&player) {
                return;
            }
            let cid = api.player_id(&player).map(|p| p.0).unwrap_or(-1);
            let parts: Vec<&str> = message.trim().split_whitespace().collect();
            match parts.first().copied().unwrap_or("") {
                "!v10test" => {
                    let pid = api.player_id(&player).unwrap_or(PlayerId(-1));
                    api.whisper(
                        &player,
                        &format!(
                            "[v11.0] api={} cid={} tile_edit={} loads={} reloads={} sched={}",
                            ddnet_sdk::plugin_api_version(),
                            pid,
                            api.has_permission("tile_edit"),
                            loads.get(),
                            reloads.get(),
                            sched_hits.get()
                        ),
                    );
                }
                "!v10step4" => {
                    let pid = api.player_id(&player).unwrap_or(PlayerId(0));
                    let hud_ok = api.set_hud_line(pid, 1, "v10.0 HUD slot 1").is_ok()
                        && api.send_progress_bar(pid, "STEP4", 7, 10, 10).is_ok();
                    let proj_ok = api
                        .spawn_projectile(
                            9001,
                            pid,
                            Vec2::new(100.0, 100.0),
                            Vec2::new(5.0, 0.0),
                            0.0,
                            1500,
                        )
                        .is_ok();
                    api.despawn_projectile(9001);
                    let perm_denied = matches!(
                        api.force_input(pid, 1, 0, 0, 0),
                        Err(SdkError::PermissionDenied { .. })
                    );
                    api.db_add_by_account("v10test_acc", "coins", 10);
                    api.db_transfer_by_account("v10_xfer", "v10test_acc", "v10test_acc2", "coins", 5);
                    api.state_set("last_tester", &player);
                    api.state_get("v10_st", "last_tester");
                    api.schedule_after(200, 401);
                    api.schedule_every(300, 402);
                    api.whisper(
                        &player,
                        &format!(
                            "[v10_step4] hud={} proj={} perm_guard={} sched_hits={}",
                            hud_ok,
                            proj_ok,
                            perm_denied,
                            sched_hits.get()
                        ),
                    );
                }
                "!impulse" => {
                    let vx: f32 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let vy: f32 = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(-18.0);
                    api.apply_impulse(cid, vx, vy);
                    api.whisper(
                        &player,
                        &format!("[BlockB] apply_impulse({:.1}, {:.1})", vx, vy),
                    );
                }
                "!hookwatch" => {
                    let mut w = w_clone.borrow_mut();
                    let on = if w.contains(&player) {
                        w.remove(&player);
                        false
                    } else {
                        w.insert(player.clone());
                        true
                    };
                    api.whisper(&player, &format!("[BlockB] hookwatch={}", on));
                }
                "!skinto" => {
                    let Some(target_tok) = parts.get(1) else {
                        api.whisper(&player, "Использование: !skinto <cid|nick> [skin]");
                        return;
                    };
                    let Some(target) = resolve_cid(api, target_tok) else {
                        api.whisper(&player, "Игрок не найден");
                        return;
                    };
                    if let Some(&skin) = parts.get(2) {
                        api.set_skin_to(cid, target, skin);
                        api.whisper(
                            &player,
                            &format!("[BlockB] set_skin_to({} -> {}, {})", cid, target, skin),
                        );
                    } else {
                        api.clear_skin_to(cid, target);
                        api.whisper(
                            &player,
                            &format!("[BlockB] clear_skin_to({} -> {})", cid, target),
                        );
                    }
                }
                "!vtext" => {
                    let arg = parts.get(1..).map(|v| v.join(" ")).unwrap_or_else(|| "DDNETPR V11!".to_string());
                    if arg.eq_ignore_ascii_case("clear") || arg.eq_ignore_ascii_case("off") {
                        api.clear_visual_texts();
                        api.track_positions(false);
                        api.whisper(&player, "[BlockB] clear_visual_texts()");
                    } else if let Some(pos) = api.states.get(&cid).copied() {
                        api.spawn_visual_text(0, pos.x, pos.y - 110.0, 1.25, 0, &arg);
                        api.states.remove(&cid);
                        api.track_positions(false);
                        api.whisper(&player, &format!("[BlockB] spawn_visual_text: {}", arg));
                    } else {
                        api.track_positions(true);
                        api.whisper(&player, "[BlockB] Позиция ещё не получена, повтори команду");
                    }
                }
                _ => {}
            }
        }
        _ => {}
    });
}
