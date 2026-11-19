#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod album_art;
mod lyrics;
mod mpris;
mod notifications;
mod prayer;
mod runtime;
mod state;
mod sysinfo;
mod ui_renderer;
#[cfg(target_os = "windows")]
mod win32_window;
#[cfg(target_os = "linux")]
mod x11_window;

use chrono::{Local, Timelike};
use std::path::PathBuf;
#[cfg(not(target_os = "windows"))]
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tiny_skia::Pixmap;

use album_art::AlbumArtManager;
use lyrics::{cycle_lyric_provider, provider_accent, LyricsManager};
use mpris::{seek_media_to, send_mpris_command};
use notifications::NotificationHub;
use prayer::{
    calculate_prayer_schedule_for_region, resolve_region_from_state, search_regions_local,
    search_regions_online, PrayerRegion, INDONESIAN_CITIES,
};
use runtime::{
    spawn_background_monitor, ActiveTab, InputField, RuntimeContext, CANVAS_HEIGHT, CANVAS_WIDTH,
    COMPACT_H, EXPANDED_W, NOTIF_TOAST_H, VOL_OSD_H, VOL_OSD_W,
};
use state::{format_file_size, set_autostart_enabled, ShelfFile, TodoItem};
use sysinfo::{get_live_volume_fast, set_system_volume, toggle_system_mute};
use ui_renderer::{render_island, FontEngine, UiAction, UiHitMap};
#[cfg(target_os = "windows")]
use win32_window::{Win32OverlayWindow as OverlayWindow, WindowEvent};
#[cfg(target_os = "linux")]
use x11_window::{WindowEvent, X11OverlayWindow as OverlayWindow};

static RUNNING: AtomicBool = AtomicBool::new(true);

#[cfg(unix)]
extern "C" fn handle_signal(_sig: libc::c_int) {
    RUNNING.store(false, Ordering::SeqCst);
}

fn install_signal_handlers() {
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGTERM, handle_signal as *const () as usize);
        libc::signal(libc::SIGINT, handle_signal as *const () as usize);
    }
}

fn play_alert_sound() {
    #[cfg(target_os = "windows")]
    {
        thread::spawn(win32_window::play_alert_beep);
    }
    #[cfg(not(target_os = "windows"))]
    {
        thread::spawn(|| {
            let candidates = [
                "/usr/share/sounds/freedesktop/stereo/alarm-clock-elapsed.oga",
                "/usr/share/sounds/freedesktop/stereo/complete.oga",
                "/usr/share/sounds/freedesktop/stereo/message.oga",
            ];
            for p in candidates {
                if std::path::Path::new(p).exists() {
                    if Command::new("paplay").arg(p).status().is_ok() {
                        return;
                    }
                    if Command::new("pw-play").arg(p).status().is_ok() {
                        return;
                    }
                }
            }
        });
    }
}

fn update_target_size(ctx: &mut RuntimeContext) {
    if ctx.expanded {
        let exp_h = ctx.compute_tab_expanded_h();
        ctx.expanded_target_h = exp_h;
        ctx.target_w = EXPANDED_W;
        ctx.target_h = exp_h;
    } else if ctx.vol_osd_remaining > 0.0 {
        ctx.target_w = VOL_OSD_W;
        ctx.target_h = VOL_OSD_H;
    } else if ctx.active_notif_toast.is_some() && ctx.notif_toast_remaining > 0.0 {
        ctx.target_w = ctx.notif_compact_w;
        ctx.target_h = NOTIF_TOAST_H;
    } else {
        ctx.target_w = ctx.dynamic_compact_w;
        ctx.target_h = COMPACT_H;
    }
}

fn expand_island(ctx: &mut RuntimeContext, notif_hub: &NotificationHub) {
    if ctx.expanded {
        return;
    }
    if ctx.active_notif_toast.is_some() && ctx.notif_toast_remaining > 0.0 {
        ctx.active_notif_toast = None;
        ctx.notif_toast_remaining = 0.0;
        ctx.set_active_tab(ActiveTab::Notif);
        notif_hub.mark_all_read();
    }
    ctx.expanded = true;
    update_target_size(ctx);
}

fn apply_prayer_region(ctx: &mut RuntimeContext, reg: PrayerRegion) {
    ctx.app_state.selected_city = reg.name.clone();
    ctx.app_state.prayer_lat = Some(reg.lat);
    ctx.app_state.prayer_lon = Some(reg.lon);
    ctx.app_state.prayer_tz_offset = Some(reg.tz_offset);
    ctx.app_state.prayer_tz_label = Some(reg.tz_label.clone());
    ctx.app_state.save();

    if let Some(pos) = ctx
        .prayer_region_options
        .iter()
        .position(|r| r.name.eq_ignore_ascii_case(&reg.name))
    {
        ctx.prayer_selected_idx = pos;
    } else {
        ctx.prayer_region_options.insert(0, reg.clone());
        ctx.prayer_selected_idx = 0;
    }

    ctx.prayer_schedule = calculate_prayer_schedule_for_region(&reg);
    ctx.next_prayer_summary = format!(
        "{} {}",
        ctx.prayer_schedule.next_prayer_name, ctx.prayer_schedule.next_prayer_time
    );
}

fn submit_todo(ctx: &mut RuntimeContext) {
    let txt = ctx.todo_draft.trim().to_string();
    if txt.is_empty() {
        return;
    }
    let next_id = ctx
        .app_state
        .todos
        .iter()
        .map(|t| t.id)
        .max()
        .unwrap_or(0)
        + 1;
    ctx.app_state.todos.insert(
        0,
        TodoItem {
            id: next_id,
            text: txt,
            completed: false,
        },
    );
    ctx.todo_draft.clear();
    ctx.app_state.save();
    update_target_size(ctx);
}

fn submit_prayer_search(
    ctx: &mut RuntimeContext,
    pending_prayer_result: &Arc<Mutex<Option<(String, Vec<PrayerRegion>)>>>,
) {
    let q = ctx.prayer_search_draft.trim().to_string();
    if q.is_empty() {
        return;
    }
    let local = search_regions_local(&q);
    if let Some(first) = local.into_iter().next() {
        ctx.prayer_search_status = format!("Dipilih: {}", first.name);
        ctx.prayer_search_draft.clear();
        apply_prayer_region(ctx, first);
        return;
    }
    ctx.prayer_search_status = format!("Mencari '{}'...", q);
    let slot = pending_prayer_result.clone();
    thread::spawn(move || {
        let online = search_regions_online(&q);
        if let Ok(mut guard) = slot.lock() {
            *guard = Some((q, online));
        }
    });
}

fn main() {
    install_signal_handlers();

    let initial_state = state::AppState::load();
    let init_is_bottom = initial_state.island_position == "bottom";
    let mut win = match OverlayWindow::new(
        CANVAS_WIDTH as u16,
        CANVAS_HEIGHT as u16,
        init_is_bottom,
        initial_state.y_offset,
    ) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("[Arwiland] {e}");
            std::process::exit(1);
        }
    };

    let mut fonts = match FontEngine::load() {
        Ok(f) => f,
        Err(e) => {
            eprintln!("[Arwiland] {e}");
            std::process::exit(1);
        }
    };

    #[cfg(target_os = "linux")]
    unsafe {
        extern "C" {
            fn malloc_trim(pad: usize) -> libc::c_int;
        }
        malloc_trim(0);
    }
    #[cfg(target_os = "windows")]
    win32_window::trim_working_set();

    let mut pixmap = Pixmap::new(CANVAS_WIDTH as u32, CANVAS_HEIGHT as u32)
        .expect("Failed to allocate Pixmap");

    let init_region = resolve_region_from_state(
        &initial_state.selected_city,
        initial_state.prayer_lat,
        initial_state.prayer_lon,
        initial_state.prayer_tz_offset,
        initial_state.prayer_tz_label.as_deref(),
    );
    let mut region_options = vec![init_region.clone()];
    for c in INDONESIAN_CITIES {
        if !region_options
            .iter()
            .any(|r| r.name.eq_ignore_ascii_case(c.name))
        {
            region_options.push(c.to_region());
        }
    }
    let initial_prayer = calculate_prayer_schedule_for_region(&init_region);

    let mut ctx = RuntimeContext::new(initial_state, initial_prayer, region_options);
    let bg_mon = spawn_background_monitor();
    let lyrics_mgr = LyricsManager::new();
    let art_mgr = AlbumArtManager::new();
    let notif_hub = NotificationHub::new(ctx.app_state.notifications.clone());

    let pending_picked_files: Arc<Mutex<Vec<PathBuf>>> = Arc::new(Mutex::new(Vec::new()));
    let pending_prayer_result: Arc<Mutex<Option<(String, Vec<PrayerRegion>)>>> =
        Arc::new(Mutex::new(None));

    let mut hitmap = UiHitMap::default();
    let mut events: Vec<WindowEvent> = Vec::with_capacity(32);

    let monitor_hz = win.refresh_rate_hz().clamp(60, 120);
    let startup_at = Instant::now();
    #[cfg(target_os = "windows")]
    let mut post_init_trimmed = false;
    #[cfg(target_os = "linux")]
    let mut startup_resync_stage: u8 = 0;
    let mut last_tick = Instant::now();
    let mut last_sec_tick = Instant::now();
    let mut last_present_at = Instant::now()
        .checked_sub(Duration::from_secs(1))
        .unwrap_or_else(Instant::now);
    let mut hover_expand_at: Option<Instant> = None;
    let mut leave_collapse_at: Option<Instant> = None;
    let mut sample_notif_idx: usize = 0;
    let mut needs_redraw = true;

    win.set_input_region(ctx.cur_w.round() as i32 + 8, ctx.cur_h.round() as i32 + 6);

    while RUNNING.load(Ordering::Relaxed) {
        let now = Instant::now();
        let dt = now.duration_since(last_tick).as_secs_f64().clamp(0.001, 0.033);
        last_tick = now;
        let mut urgent_redraw = false;

        #[cfg(target_os = "windows")]
        if !post_init_trimmed && startup_at.elapsed() >= Duration::from_secs(3) {
            post_init_trimmed = true;
            win32_window::trim_working_set();
        }

        // On Linux (especially GNOME Wayland/XWayland autostart), re-sync monitor geometry,
        // map_window, StackMode::ABOVE and repaint after the desktop shell finishes initializing.
        #[cfg(target_os = "linux")]
        if startup_resync_stage < 3 {
            let next_ms = match startup_resync_stage {
                0 => 1500,
                1 => 4500,
                _ => 9000,
            };
            if startup_at.elapsed() >= Duration::from_millis(next_ms) {
                startup_resync_stage += 1;
                win.sync_monitor_and_raise();
                let iw = ctx.cur_w.round() as i32 + 8;
                let ih = ctx.cur_h.round() as i32 + 6;
                ctx.last_input_w = iw;
                ctx.last_input_h = ih;
                win.set_input_region(iw, ih);
                needs_redraw = true;
                urgent_redraw = true;
            }
        }

        // 0. Drain async file picker & online prayer search results
        if let Ok(mut picked) = pending_picked_files.try_lock() {
            if !picked.is_empty() {
                for p in picked.drain(..) {
                    let path_str = p.to_string_lossy().to_string();
                    if !ctx.app_state.shelf_files.iter().any(|f| f.path == path_str) {
                        let name = p
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| path_str.clone());
                        let size_bytes = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
                        ctx.app_state.shelf_files.insert(
                            0,
                            ShelfFile {
                                name,
                                path: path_str,
                                size_label: format_file_size(size_bytes),
                            },
                        );
                    }
                }
                ctx.app_state.save();
                update_target_size(&mut ctx);
                needs_redraw = true;
            }
        }
        if let Ok(mut pr) = pending_prayer_result.try_lock() {
            if let Some((q, list)) = pr.take() {
                if let Some(first) = list.into_iter().next() {
                    ctx.prayer_search_status = format!("Ditemukan: {}", first.name);
                    ctx.prayer_search_draft.clear();
                    apply_prayer_region(&mut ctx, first);
                } else {
                    ctx.prayer_search_status = format!("'{}' tidak ditemukan", q);
                }
                needs_redraw = true;
            }
        }

        // 1. Drain Window Events
        win.drain_events(&mut events);
        for ev in events.drain(..) {
            match ev {
                WindowEvent::Expose => {
                    needs_redraw = true;
                    urgent_redraw = true;
                }
                WindowEvent::MonitorChanged => {
                    win.sync_monitor_and_raise();
                    let iw = ctx.cur_w.round() as i32 + 8;
                    let ih = ctx.cur_h.round() as i32 + 6;
                    ctx.last_input_w = iw;
                    ctx.last_input_h = ih;
                    win.set_input_region(iw, ih);
                    needs_redraw = true;
                    urgent_redraw = true;
                }
                WindowEvent::PointerEnter(x, y) => {
                    ctx.is_hovered = true;
                    ctx.hover_x = x as f32;
                    ctx.hover_y = y as f32;
                    leave_collapse_at = None;
                    win.raise();
                    if !ctx.expanded && ctx.app_state.hover_expand && ctx.vol_osd_remaining <= 0.0 {
                        hover_expand_at = Some(now);
                    }
                    needs_redraw = true;
                    urgent_redraw = true;
                }
                WindowEvent::PointerLeave => {
                    ctx.is_hovered = false;
                    ctx.hover_x = -1.0;
                    ctx.hover_y = -1.0;
                    hover_expand_at = None;
                    if ctx.expanded
                        && !ctx.app_state.pinned
                        && ctx.app_state.auto_collapse
                        && !ctx.media_seek_dragging
                        && !ctx.vol_osd_dragging
                        && ctx.focused_input.is_none()
                    {
                        leave_collapse_at = Some(now);
                    }
                    needs_redraw = true;
                    urgent_redraw = true;
                }
                WindowEvent::PointerMove(x, y) => {
                    let fx = x as f32;
                    let fy = y as f32;
                    ctx.hover_x = fx;
                    ctx.hover_y = fy;

                    if ctx.vol_osd_dragging && hitmap.vol_bar.w > 1.0 {
                        let frac = ((fx - hitmap.vol_bar.x) / hitmap.vol_bar.w).clamp(0.0, 1.0);
                        let pct = (frac * 100.0).round() as u32;
                        ctx.cur_volume_pct = pct;
                        ctx.disp_volume_pct = pct as f64;
                        ctx.cur_volume_muted = false;
                        ctx.last_vol_set_at = Some(now);
                        if !ctx.expanded {
                            ctx.vol_osd_remaining = 2.2;
                        }
                        set_system_volume(pct);
                        needs_redraw = true;
                    } else if ctx.media_seek_dragging
                        && hitmap.seek_bar.w > 1.0
                        && ctx.cached_media.length_secs > 0
                    {
                        let frac = ((fx - hitmap.seek_bar.x) / hitmap.seek_bar.w).clamp(0.0, 1.0);
                        ctx.interp_pos_sec = frac as f64 * ctx.cached_media.length_secs as f64;
                        needs_redraw = true;
                    } else if ctx.expanded {
                        // Coalesced to display frame rate (prevents 500-1000Hz mouse DWM stutter on Windows)
                        needs_redraw = true;
                    }
                }
                WindowEvent::ButtonDown(btn, x, y) => {
                    urgent_redraw = true;
                    let fx = x as f32;
                    let fy = y as f32;
                    ctx.hover_x = fx;
                    ctx.hover_y = fy;
                    leave_collapse_at = None;
                    hover_expand_at = None;

                    match btn {
                        1 => {
                            if !ctx.expanded {
                                if let Some(act) = hitmap.hit_test(fx, fy) {
                                    match act {
                                        UiAction::VolBar => {
                                            ctx.vol_osd_dragging = true;
                                            let frac = ((fx - hitmap.vol_bar.x) / hitmap.vol_bar.w)
                                                .clamp(0.0, 1.0);
                                            let pct = (frac * 100.0).round() as u32;
                                            ctx.cur_volume_pct = pct;
                                            ctx.disp_volume_pct = pct as f64;
                                            ctx.cur_volume_muted = false;
                                            ctx.last_vol_set_at = Some(now);
                                            ctx.vol_osd_remaining = 2.2;
                                            set_system_volume(pct);
                                        }
                                        UiAction::VolMute => {
                                            ctx.cur_volume_muted = !ctx.cur_volume_muted;
                                            ctx.last_vol_set_at = Some(now);
                                            ctx.vol_osd_remaining = 2.2;
                                            toggle_system_mute();
                                        }
                                        _ => expand_island(&mut ctx, &notif_hub),
                                    }
                                } else {
                                    expand_island(&mut ctx, &notif_hub);
                                }
                                needs_redraw = true;
                            } else if let Some(act) = hitmap.hit_test(fx, fy) {
                                match act {
                                    UiAction::SelectTab(tab) => {
                                        ctx.set_active_tab(tab);
                                        if tab == ActiveTab::Notif {
                                            notif_hub.mark_all_read();
                                        }
                                        update_target_size(&mut ctx);
                                    }
                                    UiAction::ToggleStartup => {
                                        let next = !ctx.app_state.run_on_startup;
                                        ctx.app_state.run_on_startup = next;
                                        set_autostart_enabled(next);
                                        ctx.app_state.save();
                                    }
                                    UiAction::ToggleSettings => {
                                        if ctx.active_tab == ActiveTab::Settings {
                                            let back = ctx.prev_tab_before_settings;
                                            ctx.set_active_tab(back);
                                        } else {
                                            ctx.set_active_tab(ActiveTab::Settings);
                                        }
                                        update_target_size(&mut ctx);
                                    }
                                    UiAction::TogglePin => {
                                        ctx.app_state.pinned = !ctx.app_state.pinned;
                                        ctx.app_state.save();
                                    }
                                    UiAction::Quit => {
                                        RUNNING.store(false, Ordering::SeqCst);
                                    }
                                    UiAction::MediaPrev => {
                                        thread::spawn(|| {
                                            let _ = send_mpris_command("Previous");
                                        });
                                    }
                                    UiAction::MediaPlayPause => {
                                        thread::spawn(|| {
                                            let _ = send_mpris_command("PlayPause");
                                        });
                                    }
                                    UiAction::MediaNext => {
                                        thread::spawn(|| {
                                            let _ = send_mpris_command("Next");
                                        });
                                    }
                                    UiAction::MediaSeekBar => {
                                        if ctx.cached_media.length_secs > 0 && hitmap.seek_bar.w > 1.0 {
                                            ctx.media_seek_dragging = true;
                                            let frac = ((fx - hitmap.seek_bar.x) / hitmap.seek_bar.w)
                                                .clamp(0.0, 1.0);
                                            ctx.interp_pos_sec =
                                                frac as f64 * ctx.cached_media.length_secs as f64;
                                        }
                                    }
                                    UiAction::VolBar => {
                                        ctx.vol_osd_dragging = true;
                                        let frac = ((fx - hitmap.vol_bar.x) / hitmap.vol_bar.w)
                                            .clamp(0.0, 1.0);
                                        let pct = (frac * 100.0).round() as u32;
                                        ctx.cur_volume_pct = pct;
                                        ctx.disp_volume_pct = pct as f64;
                                        ctx.cur_volume_muted = false;
                                        ctx.last_vol_set_at = Some(now);
                                        set_system_volume(pct);
                                    }
                                    UiAction::VolMute => {
                                        ctx.cur_volume_muted = !ctx.cur_volume_muted;
                                        ctx.last_vol_set_at = Some(now);
                                        toggle_system_mute();
                                    }
                                    UiAction::NotifTest => {
                                        let samples = [
                                            ("WhatsApp", "user-available-symbolic", "Budi Santoso", "Halo! Notifikasi WhatsApp tampil mulus di Living Notch Arwiland.", 1u8, "chat"),
                                            ("Spotify", "audio-x-generic-symbolic", "Lagu Berikutnya Diputar", "Sinkronisasi lirik & spektrum audio aktif di Dynamic Island.", 1u8, "media"),
                                            ("Keamanan Sistem", "dialog-warning-symbolic", "Peringatan Baterai & Daya", "Contoh notifikasi prioritas penting (Urgent) di Notch.", 2u8, "device"),
                                            ("Jadwal Sholat", "weather-clear-night-symbolic", "Waktu Sholat Segera Tiba", "10 menit menuju waktu sholat berikutnya.", 1u8, "prayer"),
                                        ];
                                        let (app, icon, sum, body, urg, cat) =
                                            samples[sample_notif_idx % samples.len()];
                                        sample_notif_idx = (sample_notif_idx + 1) % samples.len();
                                        ctx.expanded = false;
                                        ctx.is_hovered = false;
                                        update_target_size(&mut ctx);
                                        notif_hub.push_notification(app, icon, sum, body, urg, cat);
                                    }
                                    UiAction::NotifMarkRead => {
                                        notif_hub.mark_all_read();
                                    }
                                    UiAction::NotifClear => {
                                        notif_hub.clear_all();
                                        ctx.notif_history.clear();
                                        ctx.unread_notif_count = 0;
                                        ctx.notif_scroll_idx = 0;
                                        update_target_size(&mut ctx);
                                    }
                                    UiAction::NotifRemove(id) => {
                                        notif_hub.remove_by_id(id);
                                    }
                                    UiAction::PomoAdjust(delta_min) => {
                                        let cur_min = if ctx.pomo_break_mode {
                                            ctx.app_state.pomo_break_mins as i32
                                        } else {
                                            ctx.app_state.pomo_focus_mins as i32
                                        };
                                        let next_min = (cur_min + delta_min).clamp(1, 180) as u32;
                                        if ctx.pomo_break_mode {
                                            ctx.app_state.pomo_break_mins = next_min;
                                        } else {
                                            ctx.app_state.pomo_focus_mins = next_min;
                                        }
                                        ctx.pomo_total_secs = next_min * 60;
                                        if !ctx.pomo_running {
                                            ctx.pomo_remaining_secs = ctx.pomo_total_secs;
                                        }
                                        ctx.app_state.save();
                                    }
                                    UiAction::PomoStartPause => {
                                        ctx.pomo_running = !ctx.pomo_running;
                                    }
                                    UiAction::PomoReset => {
                                        ctx.pomo_running = false;
                                        let mins = if ctx.pomo_break_mode {
                                            ctx.app_state.pomo_break_mins
                                        } else {
                                            ctx.app_state.pomo_focus_mins
                                        };
                                        ctx.pomo_total_secs = mins.clamp(1, 180) * 60;
                                        ctx.pomo_remaining_secs = ctx.pomo_total_secs;
                                    }
                                    UiAction::PomoToggleMode => {
                                        ctx.pomo_running = false;
                                        ctx.pomo_break_mode = !ctx.pomo_break_mode;
                                        let mins = if ctx.pomo_break_mode {
                                            ctx.app_state.pomo_break_mins
                                        } else {
                                            ctx.app_state.pomo_focus_mins
                                        };
                                        ctx.pomo_total_secs = mins.clamp(1, 180) * 60;
                                        ctx.pomo_remaining_secs = ctx.pomo_total_secs;
                                    }
                                    UiAction::AlarmAdjustHour(dh) => {
                                        let h = (ctx.app_state.alarm_hour as i32 + dh).rem_euclid(24) as u32;
                                        ctx.app_state.alarm_hour = h;
                                        ctx.app_state.save();
                                    }
                                    UiAction::AlarmAdjustMin(dm) => {
                                        let total = (ctx.app_state.alarm_hour as i32 * 60
                                            + ctx.app_state.alarm_min as i32
                                            + dm)
                                            .rem_euclid(24 * 60) as u32;
                                        ctx.app_state.alarm_hour = total / 60;
                                        ctx.app_state.alarm_min = total % 60;
                                        ctx.app_state.save();
                                    }
                                    UiAction::AlarmToggle => {
                                        if ctx.alarm_ringing {
                                            ctx.alarm_ringing = false;
                                            ctx.alarm_ring_remaining_secs = 0;
                                        } else {
                                            ctx.app_state.alarm_enabled = !ctx.app_state.alarm_enabled;
                                            ctx.app_state.save();
                                        }
                                    }
                                    UiAction::AlarmSnooze5 => {
                                        let now_dt = Local::now();
                                        let total = (now_dt.hour() * 60 + now_dt.minute() + 5) % (24 * 60);
                                        ctx.app_state.alarm_hour = total / 60;
                                        ctx.app_state.alarm_min = total % 60;
                                        ctx.app_state.alarm_enabled = true;
                                        ctx.alarm_ringing = false;
                                        ctx.app_state.save();
                                    }
                                    UiAction::FocusInput(field) => {
                                        ctx.focused_input = Some(field);
                                        ctx.cursor_blink = 0.0;
                                        win.grab_keyboard_focus();
                                    }
                                    UiAction::TodoAdd => {
                                        submit_todo(&mut ctx);
                                    }
                                    UiAction::TodoToggle(idx) => {
                                        if let Some(item) = ctx.app_state.todos.get_mut(idx) {
                                            item.completed = !item.completed;
                                            ctx.app_state.save();
                                        }
                                    }
                                    UiAction::TodoDelete(idx) => {
                                        if idx < ctx.app_state.todos.len() {
                                            ctx.app_state.todos.remove(idx);
                                            let max_sc = ctx.app_state.todos.len().saturating_sub(4);
                                            ctx.todo_scroll_idx = ctx.todo_scroll_idx.min(max_sc);
                                            ctx.app_state.save();
                                            update_target_size(&mut ctx);
                                        }
                                    }
                                    UiAction::ShelfPickFile => {
                                        let slot = pending_picked_files.clone();
                                        thread::spawn(move || {
                                            #[cfg(target_os = "windows")]
                                            let paths = win32_window::pick_files_dialog();

                                            #[cfg(not(target_os = "windows"))]
                                            let paths: Vec<PathBuf> = {
                                                let out = Command::new("zenity")
                                                    .args(["--file-selection", "--multiple", "--separator=\n", "--title=Pilih File untuk Rak Arwiland"])
                                                    .output()
                                                    .or_else(|_| {
                                                        Command::new("kdialog")
                                                            .args(["--getopenfilename", "--multiple", "--separate-output"])
                                                            .output()
                                                    });
                                                if let Ok(o) = out {
                                                    if o.status.success() {
                                                        let txt = String::from_utf8_lossy(&o.stdout);
                                                        txt.lines()
                                                            .map(|s| s.trim())
                                                            .filter(|s| !s.is_empty())
                                                            .map(PathBuf::from)
                                                            .collect()
                                                    } else {
                                                        Vec::new()
                                                    }
                                                } else {
                                                    Vec::new()
                                                }
                                            };

                                            if !paths.is_empty() {
                                                if let Ok(mut guard) = slot.lock() {
                                                    guard.extend(paths);
                                                }
                                            }
                                        });
                                    }
                                    UiAction::ShelfClear => {
                                        ctx.app_state.shelf_files.clear();
                                        ctx.shelf_scroll_idx = 0;
                                        ctx.app_state.save();
                                        update_target_size(&mut ctx);
                                    }
                                    UiAction::ShelfOpen(idx) => {
                                        if let Some(f) = ctx.app_state.shelf_files.get(idx) {
                                            let p = f.path.clone();
                                            #[cfg(target_os = "windows")]
                                            win32_window::open_file_default(&p);
                                            #[cfg(not(target_os = "windows"))]
                                            let _ = Command::new("xdg-open").arg(p).spawn();
                                        }
                                    }
                                    UiAction::ShelfRemove(idx) => {
                                        if idx < ctx.app_state.shelf_files.len() {
                                            ctx.app_state.shelf_files.remove(idx);
                                            let max_sc = ctx.app_state.shelf_files.len().saturating_sub(4);
                                            ctx.shelf_scroll_idx = ctx.shelf_scroll_idx.min(max_sc);
                                            ctx.app_state.save();
                                            update_target_size(&mut ctx);
                                        }
                                    }
                                    UiAction::PrayerCycleCity => {
                                        if !ctx.prayer_region_options.is_empty() {
                                            let next_idx = (ctx.prayer_selected_idx + 1)
                                                % ctx.prayer_region_options.len();
                                            let reg = ctx.prayer_region_options[next_idx].clone();
                                            ctx.prayer_selected_idx = next_idx;
                                            apply_prayer_region(&mut ctx, reg);
                                        }
                                    }
                                    UiAction::PrayerSearchSubmit => {
                                        submit_prayer_search(&mut ctx, &pending_prayer_result);
                                    }
                                    UiAction::SettingCycleIslandStyle => {
                                        ctx.app_state.island_style = match ctx.app_state.island_style.as_str() {
                                            "notch" => "pill".to_string(),
                                            "pill" => "dock".to_string(),
                                            _ => "notch".to_string(),
                                        };
                                        ctx.app_state.save();
                                    }
                                    UiAction::SettingTogglePosition => {
                                        ctx.app_state.island_position =
                                            if ctx.app_state.island_position == "bottom" {
                                                "top".to_string()
                                            } else {
                                                "bottom".to_string()
                                            };
                                        ctx.app_state.save();
                                        let is_bottom = ctx.app_state.island_position == "bottom";
                                        win.set_position(is_bottom, ctx.app_state.y_offset);
                                        let iw = ctx.cur_w.round() as i32 + 8;
                                        let ih = ctx.cur_h.round() as i32 + 6;
                                        ctx.last_input_w = iw;
                                        ctx.last_input_h = ih;
                                        win.set_input_region(iw, ih);
                                    }
                                    UiAction::SettingYOffset(delta) => {
                                        ctx.app_state.y_offset =
                                            (ctx.app_state.y_offset + delta).clamp(0, 64);
                                        ctx.app_state.save();
                                        let is_bottom = ctx.app_state.island_position == "bottom";
                                        win.set_position(is_bottom, ctx.app_state.y_offset);
                                    }
                                    UiAction::SettingCycleAnimPreset => {
                                        ctx.app_state.anim_preset = match ctx.app_state.anim_preset.as_str() {
                                            "smooth" => "bounce".to_string(),
                                            "bounce" => "snappy".to_string(),
                                            _ => "smooth".to_string(),
                                        };
                                        ctx.app_state.save();
                                    }
                                    UiAction::SettingToggleHoverExpand => {
                                        ctx.app_state.hover_expand = !ctx.app_state.hover_expand;
                                        ctx.app_state.save();
                                    }
                                    UiAction::SettingCycleLyricProvider(delta) => {
                                        ctx.app_state.lyric_provider =
                                            cycle_lyric_provider(&ctx.app_state.lyric_provider, delta)
                                                .to_string();
                                        ctx.lyric_accent_rgb =
                                            provider_accent(&ctx.app_state.lyric_provider);
                                        lyrics_mgr.clear_cache();
                                        ctx.prev_prev_lyric.clear();
                                        ctx.prev_lyric.clear();
                                        ctx.cur_lyric.clear();
                                        ctx.next_lyric.clear();
                                        ctx.lyric_words.clear();
                                        ctx.active_word_idx = 0;
                                        ctx.last_word_idx = 0;
                                        ctx.app_state.save();
                                    }
                                    UiAction::SettingCycleLyricStyle => {
                                        ctx.app_state.lyric_style = match ctx.app_state.lyric_style.as_str() {
                                            "word_bounce" => "sweep".to_string(),
                                            "sweep" => "cinema_glow".to_string(),
                                            _ => "word_bounce".to_string(),
                                        };
                                        ctx.app_state.save();
                                    }
                                    UiAction::SettingCompactWidth(delta) => {
                                        ctx.app_state.compact_width =
                                            (ctx.app_state.compact_width + delta).clamp(250, 420);
                                        ctx.app_state.save();
                                    }
                                    UiAction::SettingLyricOffset(delta) => {
                                        ctx.app_state.lyric_offset_ms =
                                            (ctx.app_state.lyric_offset_ms + delta).clamp(-3000, 3000);
                                        ctx.app_state.save();
                                    }
                                    UiAction::SettingToggleNotif => {
                                        ctx.app_state.notif_enabled = !ctx.app_state.notif_enabled;
                                        if !ctx.app_state.notif_enabled {
                                            ctx.active_notif_toast = None;
                                            ctx.notif_toast_remaining = 0.0;
                                            update_target_size(&mut ctx);
                                        }
                                        ctx.app_state.save();
                                    }
                                }
                                needs_redraw = true;
                            } else {
                                // Clicked compact bar while expanded: collapse smoothly
                                ctx.focused_input = None;
                                let is_bottom = ctx.app_state.island_position == "bottom";
                                let on_compact_bar = if is_bottom {
                                    fy >= (CANVAS_HEIGHT as f32 - COMPACT_H as f32)
                                } else {
                                    fy <= COMPACT_H as f32
                                };
                                if on_compact_bar {
                                    ctx.expanded = false;
                                    update_target_size(&mut ctx);
                                    needs_redraw = true;
                                }
                            }
                        }
                        3 => {
                            thread::spawn(|| {
                                let _ = send_mpris_command("PlayPause");
                            });
                        }
                        // Scroll Up
                        4 => {
                            if ctx.expanded {
                                match ctx.active_tab {
                                    ActiveTab::Notif => {
                                        ctx.notif_scroll_idx = ctx.notif_scroll_idx.saturating_sub(1);
                                    }
                                    ActiveTab::Todo => {
                                        ctx.todo_scroll_idx = ctx.todo_scroll_idx.saturating_sub(1);
                                    }
                                    ActiveTab::Shelf => {
                                        ctx.shelf_scroll_idx = ctx.shelf_scroll_idx.saturating_sub(1);
                                    }
                                    _ => {
                                        let next = (ctx.cur_volume_pct + 5).min(100);
                                        ctx.cur_volume_pct = next;
                                        ctx.cur_volume_muted = false;
                                        ctx.last_vol_set_at = Some(now);
                                        set_system_volume(next);
                                    }
                                }
                            } else {
                                let next = (ctx.cur_volume_pct + 5).min(100);
                                ctx.cur_volume_pct = next;
                                ctx.cur_volume_muted = false;
                                ctx.last_vol_set_at = Some(now);
                                if ctx.vol_osd_remaining <= 0.0 {
                                    ctx.vol_osd_anim_prog = 0.0;
                                }
                                ctx.vol_osd_remaining = 2.0;
                                update_target_size(&mut ctx);
                                set_system_volume(next);
                            }
                            needs_redraw = true;
                        }
                        // Scroll Down
                        5 => {
                            if ctx.expanded {
                                match ctx.active_tab {
                                    ActiveTab::Notif => {
                                        if ctx.notif_scroll_idx + 3 < ctx.notif_history.len() {
                                            ctx.notif_scroll_idx += 1;
                                        }
                                    }
                                    ActiveTab::Todo => {
                                        if ctx.todo_scroll_idx + 4 < ctx.app_state.todos.len() {
                                            ctx.todo_scroll_idx += 1;
                                        }
                                    }
                                    ActiveTab::Shelf => {
                                        if ctx.shelf_scroll_idx + 4 < ctx.app_state.shelf_files.len() {
                                            ctx.shelf_scroll_idx += 1;
                                        }
                                    }
                                    _ => {
                                        let next = ctx.cur_volume_pct.saturating_sub(5);
                                        ctx.cur_volume_pct = next;
                                        ctx.cur_volume_muted = false;
                                        ctx.last_vol_set_at = Some(now);
                                        set_system_volume(next);
                                    }
                                }
                            } else {
                                let next = ctx.cur_volume_pct.saturating_sub(5);
                                ctx.cur_volume_pct = next;
                                ctx.cur_volume_muted = false;
                                ctx.last_vol_set_at = Some(now);
                                if ctx.vol_osd_remaining <= 0.0 {
                                    ctx.vol_osd_anim_prog = 0.0;
                                }
                                ctx.vol_osd_remaining = 2.0;
                                update_target_size(&mut ctx);
                                set_system_volume(next);
                            }
                            needs_redraw = true;
                        }
                        _ => {}
                    }
                }
                WindowEvent::ButtonUp(1) => {
                    urgent_redraw = true;
                    let was_dragging = ctx.vol_osd_dragging || ctx.media_seek_dragging;
                    if ctx.vol_osd_dragging {
                        ctx.vol_osd_dragging = false;
                        needs_redraw = true;
                    }
                    if ctx.media_seek_dragging {
                        ctx.media_seek_dragging = false;
                        ctx.last_media_seek_at = Some(now);
                        let bus = ctx.cached_media.active_bus.clone();
                        let tid = ctx.cached_media.track_id.clone();
                        let target = ctx.interp_pos_sec;
                        thread::spawn(move || {
                            let _ = seek_media_to(bus.as_deref(), tid.as_deref(), target);
                        });
                        needs_redraw = true;
                    }
                    if was_dragging
                        && !ctx.is_hovered
                        && ctx.expanded
                        && !ctx.app_state.pinned
                        && ctx.app_state.auto_collapse
                        && ctx.focused_input.is_none()
                    {
                        leave_collapse_at = Some(now);
                    }
                }
                WindowEvent::ButtonUp(_) => {}
                WindowEvent::KeyChar(ch) => {
                    urgent_redraw = true;
                    match ctx.focused_input {
                        Some(InputField::Todo) => {
                            if ctx.todo_draft.len() < 120 {
                                ctx.todo_draft.push(ch);
                                ctx.cursor_blink = 0.0;
                                needs_redraw = true;
                            }
                        }
                        Some(InputField::PrayerSearch) => {
                            if ctx.prayer_search_draft.len() < 60 {
                                ctx.prayer_search_draft.push(ch);
                                ctx.cursor_blink = 0.0;
                                needs_redraw = true;
                            }
                        }
                        None => {}
                    }
                }
                WindowEvent::KeyBackspace => {
                    urgent_redraw = true;
                    match ctx.focused_input {
                        Some(InputField::Todo) => {
                            ctx.todo_draft.pop();
                            needs_redraw = true;
                        }
                        Some(InputField::PrayerSearch) => {
                            ctx.prayer_search_draft.pop();
                            needs_redraw = true;
                        }
                        None => {}
                    }
                }
                WindowEvent::KeyEnter => {
                    urgent_redraw = true;
                    match ctx.focused_input {
                        Some(InputField::Todo) => {
                            submit_todo(&mut ctx);
                            needs_redraw = true;
                        }
                        Some(InputField::PrayerSearch) => {
                            submit_prayer_search(&mut ctx, &pending_prayer_result);
                            needs_redraw = true;
                        }
                        None => {}
                    }
                }
                WindowEvent::KeyEscape => {
                    urgent_redraw = true;
                    if ctx.focused_input.is_some() {
                        ctx.focused_input = None;
                        if !ctx.is_hovered
                            && ctx.expanded
                            && !ctx.app_state.pinned
                            && ctx.app_state.auto_collapse
                        {
                            leave_collapse_at = Some(now);
                        }
                        needs_redraw = true;
                    } else if ctx.expanded {
                        ctx.expanded = false;
                        update_target_size(&mut ctx);
                        needs_redraw = true;
                    }
                }
            }
        }

        // Hover-to-expand & auto-collapse timers
        if let Some(ht) = hover_expand_at {
            if ctx.is_hovered && !ctx.expanded && ht.elapsed() >= Duration::from_millis(120) {
                hover_expand_at = None;
                expand_island(&mut ctx, &notif_hub);
                needs_redraw = true;
            }
        }
        if let Some(lt) = leave_collapse_at {
            if !ctx.is_hovered && ctx.expanded && lt.elapsed() >= Duration::from_millis(360) {
                leave_collapse_at = None;
                ctx.expanded = false;
                update_target_size(&mut ctx);
                needs_redraw = true;
            }
        }

        if ctx.focused_input.is_some() {
            ctx.cursor_blink += dt * 6.0;
            needs_redraw = true;
        }

        // 2. 1-Second Ticker: Clock, Prayer Schedule, Pomodoro, Alarm
        if last_sec_tick.elapsed() >= Duration::from_secs(1) {
            last_sec_tick = now;
            let now_dt = Local::now();
            let new_clock = now_dt.format("%H:%M").to_string();
            if new_clock != ctx.clock_str {
                ctx.clock_str = new_clock;
                needs_redraw = true;
            }

            let region = resolve_region_from_state(
                &ctx.app_state.selected_city,
                ctx.app_state.prayer_lat,
                ctx.app_state.prayer_lon,
                ctx.app_state.prayer_tz_offset,
                ctx.app_state.prayer_tz_label.as_deref(),
            );
            ctx.prayer_schedule = calculate_prayer_schedule_for_region(&region);
            let new_prayer_sum = format!(
                "{} {}",
                ctx.prayer_schedule.next_prayer_name, ctx.prayer_schedule.next_prayer_time
            );
            if new_prayer_sum != ctx.next_prayer_summary {
                ctx.next_prayer_summary = new_prayer_sum;
                needs_redraw = true;
            }
            if ctx.expanded && ctx.active_tab == ActiveTab::Prayer {
                needs_redraw = true;
            }

            // Prayer time arrival notification (once per prayer time per day)
            let today_prefix = now_dt.format("%Y-%m-%d").to_string();
            for (p_name, p_time, _) in &ctx.prayer_schedule.prayers {
                let key = format!("{}-{}-{}", today_prefix, p_name, p_time);
                if *p_time == ctx.clock_str && ctx.last_notified_prayer != key {
                    ctx.last_notified_prayer = key;
                    if ctx.app_state.notif_enabled && ctx.app_state.system_event_notifs {
                        notif_hub.push_notification(
                            "Jadwal Sholat",
                            "weather-clear-night-symbolic",
                            &format!("Waktu {} Telah Tiba", p_name),
                            &format!(
                                "Pukul {} {} untuk wilayah {}",
                                p_time, ctx.prayer_schedule.tz_label, ctx.prayer_schedule.city
                            ),
                            2,
                            "prayer",
                        );
                        play_alert_sound();
                    }
                    break;
                }
            }

            // Pomodoro tick
            if ctx.pomo_running {
                if ctx.pomo_remaining_secs > 1 {
                    ctx.pomo_remaining_secs -= 1;
                } else {
                    ctx.pomo_running = false;
                    if !ctx.pomo_break_mode {
                        ctx.app_state.pomo_completed_sessions += 1;
                        ctx.app_state.save();
                        notif_hub.push_notification(
                            "Fokus",
                            "alarm-symbolic",
                            "Sesi Fokus Selesai!",
                            "Saatnya istirahat sejenak sebelum sesi berikutnya.",
                            1,
                            "timer",
                        );
                    } else {
                        notif_hub.push_notification(
                            "Rehat",
                            "alarm-symbolic",
                            "Waktu Istirahat Selesai",
                            "Siap kembali fokus!",
                            1,
                            "timer",
                        );
                    }
                    play_alert_sound();
                    ctx.pomo_break_mode = !ctx.pomo_break_mode;
                    let mins = if ctx.pomo_break_mode {
                        ctx.app_state.pomo_break_mins
                    } else {
                        ctx.app_state.pomo_focus_mins
                    };
                    ctx.pomo_total_secs = mins.clamp(1, 180) * 60;
                    ctx.pomo_remaining_secs = ctx.pomo_total_secs;
                }
                needs_redraw = true;
            }

            // Alarm check
            if ctx.app_state.alarm_enabled
                && now_dt.hour() == ctx.app_state.alarm_hour
                && now_dt.minute() == ctx.app_state.alarm_min
            {
                let key = now_dt.format("%Y-%m-%d %H:%M").to_string();
                if ctx.last_triggered_alarm_key != key {
                    ctx.last_triggered_alarm_key = key;
                    ctx.alarm_ringing = true;
                    ctx.alarm_ring_remaining_secs = 45;
                    play_alert_sound();
                    notif_hub.push_notification(
                        "Alarm Pengingat",
                        "alarm-symbolic",
                        &format!(
                            "Alarm {:02}:{:02} Berbunyi!",
                            ctx.app_state.alarm_hour, ctx.app_state.alarm_min
                        ),
                        "Klik tab Fokus untuk mematikan atau tunda 5 menit.",
                        2,
                        "alarm",
                    );
                    needs_redraw = true;
                }
            }
            if ctx.alarm_ringing {
                if ctx.alarm_ring_remaining_secs > 1 {
                    ctx.alarm_ring_remaining_secs -= 1;
                    if ctx.alarm_ring_remaining_secs % 5 == 0 {
                        play_alert_sound();
                    }
                } else {
                    ctx.alarm_ringing = false;
                }
            }
        }

        // 3. Sync background MPRIS & System Metrics
        let recently_seeked = ctx
            .last_media_seek_at
            .map(|t| t.elapsed() < Duration::from_millis(1800))
            .unwrap_or(false);
        if let Ok(guard) = bg_mon.try_lock() {
            if guard.media_seq != ctx.last_seen_seq {
                ctx.last_seen_seq = guard.media_seq;
                let new_m = guard.media.clone();
                let track_changed = ctx.cached_media.title != new_m.title
                    || ctx.cached_media.artist != new_m.artist;
                let art_changed = ctx.cached_media.art_path != new_m.art_path
                    || (ctx.cached_media.art_image.is_none() && new_m.art_image.is_some());
                let play_changed = ctx.cached_media.playing != new_m.playing;

                let missing_timeline = new_m.playing
                    && !track_changed
                    && !play_changed
                    && !new_m.timeline_live
                    && new_m.position_f64 <= 0.001
                    && ctx.interp_pos_sec > 0.5;

                if track_changed
                    || (!missing_timeline
                        && !ctx.media_seek_dragging
                        && !recently_seeked
                        && ((ctx.interp_pos_sec - new_m.position_f64).abs() > 0.85
                            || play_changed
                            || !new_m.playing))
                {
                    ctx.interp_pos_sec = new_m.position_f64;
                }
                if track_changed {
                    let had_lyric = !ctx.cur_lyric.is_empty();
                    ctx.prev_prev_lyric.clear();
                    ctx.prev_lyric.clear();
                    ctx.cur_lyric.clear();
                    ctx.next_lyric.clear();
                    ctx.lyric_words.clear();
                    ctx.active_word_idx = 0;
                    ctx.last_word_idx = 0;
                    ctx.lyric_anim_prog = 1.0;
                    ctx.media_lyric_anim_prog = 1.0;
                    if had_lyric && ctx.expanded && ctx.active_tab == ActiveTab::Media {
                        update_target_size(&mut ctx);
                    }
                }
                if track_changed
                    || art_changed
                    || play_changed
                    || (ctx.expanded && ctx.active_tab == ActiveTab::Settings)
                {
                    needs_redraw = true;
                }
                ctx.cached_media = new_m;
                ctx.cached_metrics = guard.metrics.clone();
                ctx.album_art = art_mgr.resolve(
                    ctx.cached_media.has_player,
                    &ctx.cached_media.title,
                    &ctx.cached_media.artist,
                    ctx.cached_media.art_path.as_deref(),
                    ctx.cached_media.art_image.as_ref(),
                );
            }
        }
        let art_seq = art_mgr.seq();
        if art_seq != ctx.last_art_seq {
            ctx.last_art_seq = art_seq;
            ctx.album_art = art_mgr.resolve(
                ctx.cached_media.has_player,
                &ctx.cached_media.title,
                &ctx.cached_media.artist,
                ctx.cached_media.art_path.as_deref(),
                ctx.cached_media.art_image.as_ref(),
            );
            if ctx.expanded && ctx.active_tab == ActiveTab::Media {
                needs_redraw = true;
            }
        }

        // 4. Sync Event-Driven Volume & Mute Atomics
        let recently_dragged = ctx
            .last_vol_set_at
            .map(|t| t.elapsed() < Duration::from_millis(260))
            .unwrap_or(false);
        if !ctx.vol_osd_dragging && !recently_dragged {
            let (hw_vol, hw_mute, hw_ready) = get_live_volume_fast();
            if hw_ready {
                let is_first = ctx.prev_volume_pct.is_none();
                let changed = ctx
                    .prev_volume_pct
                    .map(|p| p.abs_diff(hw_vol) >= 1)
                    .unwrap_or(false)
                    || ctx.prev_volume_muted != Some(hw_mute);

                ctx.cur_volume_pct = hw_vol;
                ctx.cur_volume_muted = hw_mute;
                ctx.prev_volume_pct = Some(hw_vol);
                ctx.prev_volume_muted = Some(hw_mute);

                if is_first {
                    ctx.disp_volume_pct = hw_vol as f64;
                } else if changed {
                    if !ctx.expanded {
                        if ctx.vol_osd_remaining <= 0.0 {
                            ctx.vol_osd_anim_prog = 0.0;
                            ctx.vel_w += 95.0;
                            ctx.vel_h += 38.0;
                        }
                        ctx.vol_osd_remaining = 2.2;
                        update_target_size(&mut ctx);
                    }
                    needs_redraw = true;
                }
            }
        }

        if !ctx.vol_osd_dragging {
            let diff = ctx.cur_volume_pct as f64 - ctx.disp_volume_pct;
            if diff.abs() > 0.04 {
                ctx.disp_volume_pct += diff * (dt * 32.0).min(1.0);
                needs_redraw = true;
            } else {
                ctx.disp_volume_pct = ctx.cur_volume_pct as f64;
            }
        }
        if ctx.vol_osd_remaining > 0.0 {
            ctx.vol_osd_anim_prog = (ctx.vol_osd_anim_prog + dt * 12.0).min(1.0);
            if !ctx.vol_osd_dragging {
                ctx.vol_osd_remaining = (ctx.vol_osd_remaining - dt).max(0.0);
                if ctx.vol_osd_remaining <= 0.0 {
                    update_target_size(&mut ctx);
                }
            }
            needs_redraw = true;
        }

        // 5. Notification Hub & Live Toast
        let hub_seq = notif_hub.seq();
        if hub_seq != ctx.last_notif_seq {
            ctx.last_notif_seq = hub_seq;
            ctx.notif_history = notif_hub.get_history();
            ctx.unread_notif_count = notif_hub.unread_count();
            let max_sc = ctx.notif_history.len().saturating_sub(3);
            ctx.notif_scroll_idx = ctx.notif_scroll_idx.min(max_sc);
            if ctx.app_state.notifications != ctx.notif_history {
                ctx.app_state.notifications = ctx.notif_history.clone();
                ctx.app_state.save();
            }
            if ctx.expanded && ctx.active_tab == ActiveTab::Notif {
                update_target_size(&mut ctx);
            }
            needs_redraw = true;
        }

        if ctx.app_state.notif_enabled {
            let can_replace = ctx.active_notif_toast.is_none()
                || ctx.notif_toast_remaining <= 0.0
                || (ctx.notif_toast_total - ctx.notif_toast_remaining) >= 1.4;
            if can_replace {
                if let Some(toast) = notif_hub.pop_next_toast() {
                    let dur = ctx.app_state.notif_duration_secs.clamp(2, 12) as f64;
                    let sw = fonts.measure_width(&toast.summary, 12.8, true);
                    let bw = fonts.measure_width(&toast.body, 11.6, false).min(380.0);
                    ctx.notif_compact_w = (110.0 + sw.max(bw) as f64).clamp(360.0, 560.0);
                    ctx.notif_toast_total = dur;
                    ctx.notif_toast_remaining = dur;
                    ctx.notif_anim_prog = 0.0;
                    ctx.notif_scroll_phase = 0.0;
                    ctx.vel_w += 95.0;
                    ctx.vel_h += 38.0;
                    ctx.active_notif_toast = Some(toast);
                    if !ctx.expanded && ctx.vol_osd_remaining <= 0.0 {
                        update_target_size(&mut ctx);
                    }
                    win.raise();
                    needs_redraw = true;
                    urgent_redraw = true;
                }
            }
            if ctx.active_notif_toast.is_some() && ctx.notif_toast_remaining > 0.0 {
                ctx.notif_anim_prog = (ctx.notif_anim_prog + dt * 3.8).min(1.0);
                ctx.notif_scroll_phase += dt;
                ctx.notif_toast_remaining = (ctx.notif_toast_remaining - dt).max(0.0);
                if ctx.notif_toast_remaining <= 0.0 {
                    ctx.active_notif_toast = None;
                    update_target_size(&mut ctx);
                }
                needs_redraw = true;
            }
        }

        // 6. Media Interpolation & Smooth 3-Line Rolling Karaoke Lyrics
        let is_playing = ctx.cached_media.has_player && ctx.cached_media.playing;
        if is_playing || (ctx.cached_media.has_player && (ctx.media_seek_dragging || recently_seeked)) {
            if is_playing && !ctx.media_seek_dragging {
                let rate = ctx.cached_media.playback_rate.clamp(0.25, 4.0);
                let max_len = ctx.cached_media.length_secs as f64;
                if max_len > 0.5 {
                    ctx.interp_pos_sec = (ctx.interp_pos_sec + dt * rate).min(max_len);
                } else {
                    ctx.interp_pos_sec += dt * rate;
                }
                ctx.wave_phase += dt * 5.8;
            }

            let snap = lyrics_mgr.get_lyric_snapshot(
                &ctx.cached_media.title,
                &ctx.cached_media.artist,
                ctx.interp_pos_sec,
                ctx.cached_media.length_secs as f64,
                &ctx.app_state.lyric_provider,
                ctx.app_state.lyric_offset_ms,
            );

            if snap.current_text != ctx.cur_lyric {
                let was_empty = ctx.cur_lyric.is_empty();
                ctx.prev_prev_lyric = ctx.prev_lyric.clone();
                ctx.prev_lyric = if snap.prev_line_text.is_empty() {
                    ctx.cur_lyric.clone()
                } else {
                    snap.prev_line_text
                };
                ctx.cur_lyric = snap.current_text;
                ctx.lyric_anim_prog = 0.0;
                ctx.media_lyric_anim_prog = 0.0;
                if was_empty != ctx.cur_lyric.is_empty()
                    && ctx.expanded
                    && ctx.active_tab == ActiveTab::Media
                {
                    update_target_size(&mut ctx);
                }
            } else if !snap.prev_line_text.is_empty() {
                ctx.prev_lyric = snap.prev_line_text;
            }
            ctx.next_lyric = snap.next_line_text;
            ctx.lyric_words = snap.words;
            ctx.lyric_is_word_synced = snap.is_word_synced;
            ctx.active_word_idx = snap.active_word_idx;
            ctx.word_prog = snap.word_progress;
            ctx.lyric_line_prog = snap.line_progress;
            ctx.lyric_provider_badge = snap.provider_badge;
            ctx.lyric_accent_rgb = snap.accent_rgb;

            if ctx.lyric_anim_prog < 1.0 {
                ctx.lyric_anim_prog = (ctx.lyric_anim_prog + dt * 5.6).min(1.0);
            }
            if ctx.media_lyric_anim_prog < 1.0 {
                ctx.media_lyric_anim_prog = (ctx.media_lyric_anim_prog + dt * 2.4).min(1.0);
            }

            // Dynamic compact notch width breathing smoothly with lyrics
            let max_lw = ctx.app_state.max_lyric_width.clamp(480, 820) as f64;
            let disp = if !ctx.cur_lyric.is_empty() {
                ctx.cur_lyric.as_str()
            } else {
                ctx.cached_media.title.as_str()
            };
            let text_w = fonts.measure_width(disp, 12.4, true) as f64;
            let ideal_w = (text_w + 150.0).clamp(280.0, max_lw);
            if (ctx.dynamic_compact_w - ideal_w).abs() > 6.0 {
                ctx.dynamic_compact_w = ideal_w;
                if !ctx.expanded && ctx.vol_osd_remaining <= 0.0 && ctx.notif_toast_remaining <= 0.0 {
                    update_target_size(&mut ctx);
                }
            }
            needs_redraw = true;
        } else {
            let base_w = ctx.app_state.compact_width.clamp(250, 420) as f64;
            if (ctx.dynamic_compact_w - base_w).abs() > 1.0 {
                ctx.dynamic_compact_w = base_w;
                update_target_size(&mut ctx);
            }
        }

        // 7. Exact Original Spring Physics (Smooth / Bounce / Snappy + Compact Breathing)
        let mut animating_spring = false;
        let dw = ctx.target_w - ctx.cur_w;
        let dh = ctx.target_h - ctx.cur_h;
        if dw.abs() > 0.25 || dh.abs() > 0.25 || ctx.vel_w.abs() > 0.4 || ctx.vel_h.abs() > 0.4 {
            let preset = ctx.app_state.anim_preset.as_str();
            let compact_breathing = !ctx.expanded && (ctx.cur_h - COMPACT_H).abs() < 2.0;
            let growing_w = ctx.target_w > ctx.cur_w + 1.0;

            let (omega_w, zeta_w, omega_h, zeta_h) = if !ctx.expanded && ctx.vol_osd_remaining > 0.0
            {
                (29.5, 0.90, 31.0, 0.92)
            } else if compact_breathing {
                match preset {
                    "bounce" => (if growing_w { 16.5 } else { 11.5 }, 0.66, 21.0, 0.70),
                    "snappy" => (if growing_w { 21.0 } else { 15.5 }, 0.88, 28.0, 0.90),
                    _ => (if growing_w { 15.8 } else { 10.2 }, 0.88, 20.0, 0.88),
                }
            } else {
                match preset {
                    "bounce" => (21.0, 0.66, 20.0, 0.70),
                    "snappy" => (29.0, 0.88, 27.0, 0.90),
                    _ => (19.5, 0.86, 19.5, 0.88),
                }
            };

            let sub_dt = dt * 0.5;
            for _ in 0..2 {
                let acc_w =
                    (omega_w * omega_w) * (ctx.target_w - ctx.cur_w) - (2.0 * zeta_w * omega_w) * ctx.vel_w;
                ctx.vel_w += acc_w * sub_dt;
                if compact_breathing {
                    ctx.vel_w = ctx.vel_w.clamp(-650.0, 650.0);
                }
                ctx.cur_w = (ctx.cur_w + ctx.vel_w * sub_dt).clamp(180.0, CANVAS_WIDTH as f64 - 4.0);

                let acc_h =
                    (omega_h * omega_h) * (ctx.target_h - ctx.cur_h) - (2.0 * zeta_h * omega_h) * ctx.vel_h;
                ctx.vel_h += acc_h * sub_dt;
                ctx.cur_h = (ctx.cur_h + ctx.vel_h * sub_dt).clamp(COMPACT_H, CANVAS_HEIGHT as f64 - 2.0);
            }
            animating_spring = true;
            needs_redraw = true;
        } else if dw.abs() > 0.005 || dh.abs() > 0.005 {
            ctx.cur_w = ctx.target_w;
            ctx.cur_h = ctx.target_h;
            ctx.vel_w = 0.0;
            ctx.vel_h = 0.0;
            needs_redraw = true;
        }

        // Elastic spring physics for 6 segmented tab pills (exact original omega/zeta!)
        for i in 0..6 {
            let target = ctx.tab_spring_target[i];
            let pos = ctx.tab_spring_pos[i];
            let vel = ctx.tab_spring_vel[i];
            if (target - pos).abs() > 0.002 || vel.abs() > 0.01 {
                let (omega, zeta) = if target > 0.5 { (22.0, 0.64) } else { (25.0, 0.88) };
                let sub_dt = dt * 0.5;
                for _ in 0..2 {
                    let err = ctx.tab_spring_target[i] - ctx.tab_spring_pos[i];
                    let acc = (omega * omega) * err - (2.0 * zeta * omega) * ctx.tab_spring_vel[i];
                    ctx.tab_spring_vel[i] += acc * sub_dt;
                    ctx.tab_spring_pos[i] += ctx.tab_spring_vel[i] * sub_dt;
                }
                if ctx.tab_spring_pos[i] < 0.0 && target == 0.0 {
                    ctx.tab_spring_pos[i] = 0.0;
                    ctx.tab_spring_vel[i] = 0.0;
                }
                animating_spring = true;
                needs_redraw = true;
            } else if (pos - target).abs() > 0.0001 || vel.abs() > 0.0 {
                ctx.tab_spring_pos[i] = target;
                ctx.tab_spring_vel[i] = 0.0;
                needs_redraw = true;
            }
        }

        // Directional bounce-spring physics for switching tab pages
        if ctx.page_slide_x.abs() > 0.15 || ctx.page_slide_vel.abs() > 0.4 {
            let omega = 22.0;
            let zeta = 0.64; // crisp elastic bounce overshoot & smooth settle
            let sub_dt = dt * 0.5;
            for _ in 0..2 {
                let acc = -(omega * omega) * ctx.page_slide_x - (2.0 * zeta * omega) * ctx.page_slide_vel;
                ctx.page_slide_vel += acc * sub_dt;
                ctx.page_slide_x += ctx.page_slide_vel * sub_dt;
            }
            animating_spring = true;
            needs_redraw = true;
        } else if ctx.page_slide_x.abs() > 0.001 || ctx.page_slide_vel.abs() > 0.0 {
            ctx.page_slide_x = 0.0;
            ctx.page_slide_vel = 0.0;
            needs_redraw = true;
        }
        if ctx.page_slide_fade < 1.0 {
            ctx.page_slide_fade = (ctx.page_slide_fade + dt * 7.0).min(1.0);
            animating_spring = true;
            needs_redraw = true;
        }

        let iw = ctx.cur_w.round() as i32 + 8;
        let ih = ctx.cur_h.round() as i32 + 6;
        if (ctx.last_input_w - iw).abs() >= 4 || (ctx.last_input_h - ih).abs() >= 4 {
            ctx.last_input_w = iw;
            ctx.last_input_h = ih;
            win.set_input_region(iw, ih);
        }

        // 8. Frame-Paced Render & Blit (60..120 FPS adaptive, coalescing high-Hz mouse moves)
        let active_anim = animating_spring
            || is_playing
            || ctx.vol_osd_remaining > 0.0
            || ctx.notif_toast_remaining > 0.0
            || ctx.lyric_anim_prog < 1.0
            || ctx.media_lyric_anim_prog < 1.0
            || hover_expand_at.is_some()
            || leave_collapse_at.is_some()
            || ctx.focused_input.is_some();

        let target_fps = if animating_spring
            || ctx.vol_osd_remaining > 0.0
            || ctx.notif_toast_remaining > 0.0
            || ctx.lyric_anim_prog < 1.0
            || ctx.media_lyric_anim_prog < 1.0
        {
            monitor_hz
        } else {
            60
        };
        let frame_budget = Duration::from_micros(1_000_000 / target_fps as u64);
        let since_present = now.duration_since(last_present_at);

        if needs_redraw
            && (urgent_redraw || since_present + Duration::from_micros(500) >= frame_budget)
        {
            let active_h = render_island(&mut pixmap, &mut fonts, &ctx, &mut hitmap);
            win.present(&mut pixmap, active_h);
            last_present_at = now;
            needs_redraw = false;
        }

        // 9. Adaptive high-resolution sleep: exact frame pacing when active, 0% CPU when idle
        let wait_us: u64 = if needs_redraw {
            frame_budget
                .saturating_sub(last_present_at.elapsed())
                .as_micros() as u64
        } else if active_anim {
            frame_budget
                .saturating_sub(last_present_at.elapsed())
                .as_micros()
                .max(1000) as u64
        } else {
            250_000
        };
        win.wait_events_us(wait_us);
    }
}
