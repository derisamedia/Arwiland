use chrono::Local;
use gtk4::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::lyrics::{provider_accent, LyricWord};
use crate::mpris::{query_media_status, MediaStatus};
use crate::notifications::IslandNotification;
use crate::state::AppState;
use crate::sysinfo::{CpuTracker, SystemMetrics};

pub const CANVAS_WIDTH: i32 = 980;
pub const CANVAS_HEIGHT: i32 = 380;

pub const COMPACT_H: f64 = 38.0;
pub const VOL_OSD_W: f64 = 370.0;
pub const VOL_OSD_H: f64 = 52.0;
pub const NOTIF_TOAST_H: f64 = 74.0;
pub const EXPANDED_W: f64 = 620.0;
pub const EXPANDED_H: f64 = 322.0;

pub type CallbackSlot = Rc<RefCell<Option<Box<dyn Fn()>>>>;

pub fn new_callback_slot() -> CallbackSlot {
    Rc::new(RefCell::new(None))
}

pub fn invoke_slot(slot: &CallbackSlot) {
    if let Some(f) = slot.borrow().as_ref() {
        f();
    }
}

#[derive(Clone)]
pub struct BgSharedData {
    pub media: MediaStatus,
    pub metrics: SystemMetrics,
    pub media_seq: u64,
}

impl Default for BgSharedData {
    fn default() -> Self {
        Self {
            media: MediaStatus::default(),
            metrics: SystemMetrics {
                cpu_usage: 0.0,
                ram_usage: 0.0,
                ram_used_gb: 0.0,
                ram_total_gb: 8.0,
                battery_percent: None,
                battery_charging: false,
                volume_percent: 75,
                volume_muted: false,
                uptime_str: "0j 0m".to_string(),
            },
            media_seq: 0,
        }
    }
}

pub fn spawn_background_monitor() -> Arc<Mutex<BgSharedData>> {
    let shared = Arc::new(Mutex::new(BgSharedData::default()));
    let shared_clone = shared.clone();

    thread::spawn(move || {
        let mut cpu_tracker = CpuTracker::new();
        let mut last_metrics_at = Instant::now() - Duration::from_secs(5);
        let mut seq: u64 = 0;

        loop {
            let media = query_media_status();
            seq = seq.wrapping_add(1);

            let maybe_metrics = if last_metrics_at.elapsed() >= Duration::from_millis(450) {
                last_metrics_at = Instant::now();
                Some(cpu_tracker.sample_metrics())
            } else {
                None
            };

            {
                let mut guard = shared_clone.lock().unwrap();
                guard.media = media;
                guard.media_seq = seq;
                if let Some(m) = maybe_metrics {
                    guard.metrics = m;
                }
            }

            #[cfg(target_os = "linux")]
            thread::sleep(Duration::from_millis(150));
            #[cfg(not(target_os = "linux"))]
            thread::sleep(Duration::from_millis(450));
        }
    });

    shared
}

pub struct RuntimeContext {
    pub app_state: AppState,
    pub expanded: bool,
    pub is_hovered: bool,
    pub cur_w: f64,
    pub cur_h: f64,
    pub vel_w: f64,
    pub vel_h: f64,
    pub target_w: f64,
    pub target_h: f64,
    pub dynamic_compact_w: f64,
    pub last_frame_us: i64,
    pub last_input_w: i32,
    pub last_input_h: i32,
    pub pomo_running: bool,
    pub pomo_break_mode: bool,
    pub pomo_remaining_secs: u32,
    pub pomo_total_secs: u32,
    pub last_art_path: Option<String>,
    pub cached_media: MediaStatus,
    pub last_seen_seq: u64,
    pub interp_pos_sec: f64,
    pub cur_lyric: String,
    pub prev_lyric: String,
    pub prev_prev_lyric: String,
    pub next_lyric: String,
    pub lyric_words: Vec<LyricWord>,
    pub lyric_is_word_synced: bool,
    pub active_word_idx: usize,
    pub last_word_idx: usize,
    pub word_prog: f64,
    pub lyric_anim_prog: f64,
    pub lyric_line_prog: f64,
    pub lyric_provider_badge: String,
    pub lyric_provider_detail: String,
    pub lyric_accent_rgb: (f64, f64, f64),
    pub wave_phase: f64,
    pub clock_str: String,
    pub next_prayer_summary: String,
    pub active_notif_toast: Option<IslandNotification>,
    pub notif_toast_remaining: f64,
    pub notif_toast_total: f64,
    pub notif_anim_prog: f64,
    pub notif_scroll_phase: f64,
    pub notif_compact_w: f64,
    pub unread_notif_count: usize,
    pub last_notif_seq: u64,
    pub prev_battery_charging: Option<bool>,
    pub prev_battery_low_warned: bool,
    pub prev_volume_pct: Option<u32>,
    pub prev_volume_muted: Option<bool>,
    pub vol_osd_remaining: f64,
    pub vol_osd_anim_prog: f64,
    pub vol_osd_dragging: bool,
    pub disp_volume_pct: f64,
    pub cur_volume_pct: u32,
    pub cur_volume_muted: bool,
    pub last_vol_set_at: Option<Instant>,
    pub last_notified_prayer: String,
    pub tab_spring_pos: [f64; 6],
    pub tab_spring_vel: [f64; 6],
    pub tab_spring_target: [f64; 6],
    pub active_tab_id: String,
    pub expanded_target_h: f64,
}

impl RuntimeContext {
    pub fn new() -> Self {
        let app_state = AppState::load();
        let cw = app_state.compact_width.clamp(250, 420) as f64;
        let accent = provider_accent(&app_state.lyric_provider);
        let unread = app_state.notifications.iter().filter(|n| !n.read).count();
        Self {
            app_state,
            expanded: false,
            is_hovered: false,
            cur_w: cw,
            cur_h: COMPACT_H,
            vel_w: 0.0,
            vel_h: 0.0,
            target_w: cw,
            target_h: COMPACT_H,
            dynamic_compact_w: cw,
            last_frame_us: 0,
            last_input_w: 0,
            last_input_h: 0,
            pomo_running: false,
            pomo_break_mode: false,
            pomo_remaining_secs: 25 * 60,
            pomo_total_secs: 25 * 60,
            last_art_path: None,
            cached_media: MediaStatus::default(),
            last_seen_seq: 0,
            interp_pos_sec: 0.0,
            cur_lyric: String::new(),
            prev_lyric: String::new(),
            prev_prev_lyric: String::new(),
            next_lyric: String::new(),
            lyric_words: Vec::new(),
            lyric_is_word_synced: false,
            active_word_idx: 0,
            last_word_idx: 0,
            word_prog: 0.0,
            lyric_anim_prog: 1.0,
            lyric_line_prog: 0.0,
            lyric_provider_badge: "AUTO".to_string(),
            lyric_provider_detail: "Siap memuat lirik multi-provider".to_string(),
            lyric_accent_rgb: accent,
            wave_phase: 0.0,
            clock_str: Local::now().format("%H:%M").to_string(),
            next_prayer_summary: "Dzuhur 11:42".to_string(),
            active_notif_toast: None,
            notif_toast_remaining: 0.0,
            notif_toast_total: 5.0,
            notif_anim_prog: 1.0,
            notif_scroll_phase: 0.0,
            notif_compact_w: 420.0,
            unread_notif_count: unread,
            last_notif_seq: 0,
            prev_battery_charging: None,
            prev_battery_low_warned: false,
            prev_volume_pct: None,
            prev_volume_muted: None,
            vol_osd_remaining: 0.0,
            vol_osd_anim_prog: 1.0,
            vol_osd_dragging: false,
            disp_volume_pct: 75.0,
            cur_volume_pct: 75,
            cur_volume_muted: false,
            last_vol_set_at: None,
            last_notified_prayer: String::new(),
            tab_spring_pos: [1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            tab_spring_vel: [0.0; 6],
            tab_spring_target: [1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            active_tab_id: "media".to_string(),
            expanded_target_h: 282.0,
        }
    }
}

impl RuntimeContext {
    pub fn compute_tab_expanded_h(&self) -> f64 {
        match self.active_tab_id.as_str() {
            "media" => 282.0,
            "notif" => {
                let n = self.app_state.notifications.len();
                if n == 0 {
                    206.0
                } else {
                    (148.0 + (n.min(3) as f64) * 56.0).clamp(204.0, 310.0)
                }
            }
            "focus" => 226.0,
            "todo" => {
                let n = self.app_state.todos.len();
                if n == 0 {
                    188.0
                } else {
                    (150.0 + (n.min(4) as f64) * 38.0).clamp(188.0, 298.0)
                }
            }
            "shelf" => {
                let n = self.app_state.shelf_files.len();
                if n == 0 {
                    188.0
                } else {
                    (150.0 + (n.min(4) as f64) * 38.0).clamp(188.0, 298.0)
                }
            }
            "prayer" => 216.0,
            _ => EXPANDED_H,
        }
    }
}

pub fn make_icon_tab(icon_name: &str, text: &str) -> (gtk4::Button, gtk4::Label, gtk4::DrawingArea) {
    let btn = gtk4::Button::new();
    btn.set_tooltip_text(Some(text));
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    row.set_halign(gtk4::Align::Center);
    row.set_valign(gtk4::Align::Center);
    let img = gtk4::Image::from_icon_name(icon_name);
    img.set_pixel_size(12);
    let lbl = gtk4::Label::new(Some(text));
    lbl.set_visible(false);
    let canvas = gtk4::DrawingArea::new();
    canvas.set_content_height(16);
    canvas.set_content_width(0);
    canvas.set_valign(gtk4::Align::Center);
    row.append(&img);
    row.append(&canvas);
    btn.set_child(Some(&row));
    btn.add_css_class("tab-pill");
    (btn, lbl, canvas)
}

pub fn set_icon_pill_content(btn: &gtk4::Button, icon_name: &str, text: &str) {
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 5);
    row.set_halign(gtk4::Align::Center);
    let img = gtk4::Image::from_icon_name(icon_name);
    img.set_pixel_size(13);
    let lbl = gtk4::Label::new(Some(text));
    row.append(&img);
    row.append(&lbl);
    btn.set_child(Some(&row));
}

pub fn make_icon_pill_btn(icon_name: &str, text: &str, primary: bool) -> gtk4::Button {
    let btn = gtk4::Button::new();
    set_icon_pill_content(&btn, icon_name, text);
    btn.add_css_class("pill-btn");
    if primary {
        btn.add_css_class("primary");
    }
    btn
}

pub fn make_circle_icon_btn(
    icon_name: &str,
    pixel_size: i32,
    tooltip: Option<&str>,
) -> gtk4::Button {
    let btn = gtk4::Button::new();
    let img = gtk4::Image::from_icon_name(icon_name);
    img.set_pixel_size(pixel_size);
    btn.set_child(Some(&img));
    btn.add_css_class("top-icon-btn");
    btn.set_valign(gtk4::Align::Center);
    if let Some(tip) = tooltip {
        btn.set_tooltip_text(Some(tip));
    }
    btn
}
