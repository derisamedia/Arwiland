use chrono::Local;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::album_art::AlbumArt;
use crate::lyrics::LyricWord;
use crate::mpris::{query_media_status, MediaStatus};
use crate::notifications::IslandNotification;
use crate::prayer::{PrayerRegion, PrayerSchedule};
use crate::state::{resolve_accent_palette, AppState};
use crate::sysinfo::{CpuTracker, SystemMetrics};

pub const CANVAS_WIDTH: i32 = 880;
pub const CANVAS_HEIGHT: i32 = 340;

pub const COMPACT_H: f64 = 38.0;
pub const VOL_OSD_W: f64 = 370.0;
pub const VOL_OSD_H: f64 = 52.0;
pub const NOTIF_TOAST_H: f64 = 74.0;
pub const EXPANDED_W: f64 = 620.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ActiveTab {
    Media,
    Notif,
    Focus,
    Todo,
    Shelf,
    Prayer,
    Settings,
}

impl ActiveTab {
    pub fn seg_index(self) -> Option<usize> {
        match self {
            ActiveTab::Media => Some(0),
            ActiveTab::Notif => Some(1),
            ActiveTab::Focus => Some(2),
            ActiveTab::Todo => Some(3),
            ActiveTab::Shelf => Some(4),
            ActiveTab::Prayer => Some(5),
            ActiveTab::Settings => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InputField {
    Todo,
    PrayerSearch,
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
            let is_playing = media.playing;
            seq = seq.wrapping_add(1);

            let maybe_metrics = if last_metrics_at.elapsed() >= Duration::from_millis(1000) {
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

            let sleep_ms = if is_playing { 350 } else { 800 };
            thread::sleep(Duration::from_millis(sleep_ms));
        }
    });

    shared
}

pub struct RuntimeContext {
    pub app_state: AppState,
    pub expanded: bool,
    pub is_hovered: bool,
    pub active_tab: ActiveTab,
    pub prev_tab_before_settings: ActiveTab,

    pub cur_w: f64,
    pub cur_h: f64,
    pub vel_w: f64,
    pub vel_h: f64,
    pub target_w: f64,
    pub target_h: f64,
    pub expanded_target_h: f64,
    pub dynamic_compact_w: f64,
    pub last_input_w: i32,
    pub last_input_h: i32,

    // Elastic segmented tab spring physics (exact match to original)
    pub tab_spring_pos: [f64; 6],
    pub tab_spring_vel: [f64; 6],
    pub tab_spring_target: [f64; 6],

    // Directional bounce-spring slide transition for expanded tab content
    pub page_slide_x: f64,
    pub page_slide_vel: f64,
    pub page_slide_fade: f64,

    // Pomodoro & Alarm
    pub pomo_running: bool,
    pub pomo_break_mode: bool,
    pub pomo_remaining_secs: u32,
    pub pomo_total_secs: u32,
    pub alarm_ringing: bool,
    pub alarm_ring_remaining_secs: u32,
    pub last_triggered_alarm_key: String,

    // Media & Lyrics
    pub cached_media: MediaStatus,
    pub album_art: Option<Arc<AlbumArt>>,
    pub last_art_seq: u64,
    pub cached_metrics: SystemMetrics,
    pub last_seen_seq: u64,
    pub interp_pos_sec: f64,
    pub media_seek_dragging: bool,
    pub last_media_seek_at: Option<Instant>,

    pub prev_prev_lyric: String,
    pub prev_lyric: String,
    pub cur_lyric: String,
    pub next_lyric: String,
    pub lyric_words: Vec<LyricWord>,
    pub lyric_is_word_synced: bool,
    pub active_word_idx: usize,
    pub last_word_idx: usize,
    pub word_prog: f64,
    pub lyric_anim_prog: f64,
    pub media_lyric_anim_prog: f64,
    pub lyric_line_prog: f64,
    pub lyric_provider_badge: String,
    pub lyric_accent_rgb: (f64, f64, f64),
    pub wave_phase: f64,

    // Clock & Prayer
    pub clock_str: String,
    pub prayer_schedule: PrayerSchedule,
    pub next_prayer_summary: String,
    pub prayer_region_options: Vec<PrayerRegion>,
    pub prayer_selected_idx: usize,
    pub last_notified_prayer: String,

    // Notifications
    pub active_notif_toast: Option<IslandNotification>,
    pub notif_toast_remaining: f64,
    pub notif_toast_total: f64,
    pub notif_anim_prog: f64,
    pub notif_scroll_phase: f64,
    pub notif_compact_w: f64,
    pub notif_history: Vec<IslandNotification>,
    pub unread_notif_count: usize,
    pub last_notif_seq: u64,

    // Volume OSD
    pub prev_volume_pct: Option<u32>,
    pub prev_volume_muted: Option<bool>,
    pub vol_osd_remaining: f64,
    pub vol_osd_anim_prog: f64,
    pub vol_osd_dragging: bool,
    pub disp_volume_pct: f64,
    pub cur_volume_pct: u32,
    pub cur_volume_muted: bool,
    pub last_vol_set_at: Option<Instant>,

    // Interactive Text Input & List Scroll
    pub focused_input: Option<InputField>,
    pub todo_draft: String,
    pub prayer_search_draft: String,
    pub prayer_search_status: String,
    pub cursor_blink: f64,
    pub notif_scroll_idx: usize,
    pub todo_scroll_idx: usize,
    pub shelf_scroll_idx: usize,

    pub hover_x: f32,
    pub hover_y: f32,
}

impl RuntimeContext {
    pub fn new(
        app_state: AppState,
        prayer_schedule: PrayerSchedule,
        prayer_region_options: Vec<PrayerRegion>,
    ) -> Self {
        let cw = app_state.compact_width.clamp(250, 420) as f64;
        let accent = resolve_accent_palette(&app_state.accent_color).rgb_f64();
        let unread = app_state.notifications.iter().filter(|n| !n.read).count();
        let focus_secs = app_state.pomo_focus_mins.clamp(1, 180) * 60;
        let next_prayer_summary = format!(
            "{} {}",
            prayer_schedule.next_prayer_name, prayer_schedule.next_prayer_time
        );
        let notif_history = app_state.notifications.clone();
        Self {
            app_state,
            expanded: false,
            is_hovered: false,
            active_tab: ActiveTab::Media,
            prev_tab_before_settings: ActiveTab::Media,
            cur_w: cw,
            cur_h: COMPACT_H,
            vel_w: 0.0,
            vel_h: 0.0,
            target_w: cw,
            target_h: COMPACT_H,
            expanded_target_h: 276.0,
            dynamic_compact_w: cw,
            last_input_w: 0,
            last_input_h: 0,
            tab_spring_pos: [1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            tab_spring_vel: [0.0; 6],
            tab_spring_target: [1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            page_slide_x: 0.0,
            page_slide_vel: 0.0,
            page_slide_fade: 1.0,
            pomo_running: false,
            pomo_break_mode: false,
            pomo_remaining_secs: focus_secs,
            pomo_total_secs: focus_secs,
            alarm_ringing: false,
            alarm_ring_remaining_secs: 0,
            last_triggered_alarm_key: String::new(),
            cached_media: MediaStatus::default(),
            album_art: None,
            last_art_seq: 0,
            cached_metrics: SystemMetrics::default(),
            last_seen_seq: 0,
            interp_pos_sec: 0.0,
            media_seek_dragging: false,
            last_media_seek_at: None,
            prev_prev_lyric: String::new(),
            prev_lyric: String::new(),
            cur_lyric: String::new(),
            next_lyric: String::new(),
            lyric_words: Vec::new(),
            lyric_is_word_synced: false,
            active_word_idx: 0,
            last_word_idx: 0,
            word_prog: 0.0,
            lyric_anim_prog: 1.0,
            media_lyric_anim_prog: 1.0,
            lyric_line_prog: 0.0,
            lyric_provider_badge: "AUTO".to_string(),
            lyric_accent_rgb: accent,
            wave_phase: 0.0,
            clock_str: Local::now().format("%H:%M").to_string(),
            prayer_schedule,
            next_prayer_summary,
            prayer_region_options,
            prayer_selected_idx: 0,
            last_notified_prayer: String::new(),
            active_notif_toast: None,
            notif_toast_remaining: 0.0,
            notif_toast_total: 5.0,
            notif_anim_prog: 1.0,
            notif_scroll_phase: 0.0,
            notif_compact_w: 420.0,
            notif_history,
            unread_notif_count: unread,
            last_notif_seq: 0,
            prev_volume_pct: None,
            prev_volume_muted: None,
            vol_osd_remaining: 0.0,
            vol_osd_anim_prog: 1.0,
            vol_osd_dragging: false,
            disp_volume_pct: 75.0,
            cur_volume_pct: 75,
            cur_volume_muted: false,
            last_vol_set_at: None,
            focused_input: None,
            todo_draft: String::new(),
            prayer_search_draft: String::new(),
            prayer_search_status: String::new(),
            cursor_blink: 0.0,
            notif_scroll_idx: 0,
            todo_scroll_idx: 0,
            shelf_scroll_idx: 0,
            hover_x: -1.0,
            hover_y: -1.0,
        }
    }

    pub fn set_active_tab(&mut self, tab: ActiveTab) {
        let old_tab = self.active_tab;
        if tab != ActiveTab::Settings {
            self.prev_tab_before_settings = tab;
        }
        self.active_tab = tab;
        self.focused_input = None;

        if old_tab != tab {
            let order = |t: ActiveTab| -> i32 {
                match t {
                    ActiveTab::Media => 0,
                    ActiveTab::Notif => 1,
                    ActiveTab::Focus => 2,
                    ActiveTab::Todo => 3,
                    ActiveTab::Shelf => 4,
                    ActiveTab::Prayer => 5,
                    ActiveTab::Settings => 6,
                }
            };
            let dir = if order(tab) >= order(old_tab) { 1.0 } else { -1.0 };
            // Moving right -> enter from right to left (+dir); moving left -> enter from left to right (-dir)
            self.page_slide_x = dir * 72.0;
            self.page_slide_vel = -dir * 220.0;
            self.page_slide_fade = 0.25;
        }

        for i in 0..6 {
            if tab.seg_index() == Some(i) {
                self.tab_spring_target[i] = 1.0;
                self.tab_spring_vel[i] += 2.2;
            } else {
                self.tab_spring_target[i] = 0.0;
            }
        }
    }

    pub fn compute_tab_expanded_h(&self) -> f64 {
        match self.active_tab {
            ActiveTab::Media => {
                if self.cur_lyric.is_empty() {
                    196.0
                } else {
                    278.0
                }
            }
            ActiveTab::Notif => {
                let n = self.notif_history.len();
                if n == 0 {
                    198.0
                } else {
                    (146.0 + n.min(3) as f64 * 48.0).clamp(198.0, 298.0)
                }
            }
            ActiveTab::Focus => 232.0,
            ActiveTab::Todo => {
                let n = self.app_state.todos.len();
                if n == 0 {
                    196.0
                } else {
                    (148.0 + n.min(4) as f64 * 34.0).clamp(196.0, 296.0)
                }
            }
            ActiveTab::Shelf => {
                let n = self.app_state.shelf_files.len();
                if n == 0 {
                    196.0
                } else {
                    (148.0 + n.min(4) as f64 * 34.0).clamp(196.0, 296.0)
                }
            }
            ActiveTab::Prayer => 248.0,
            ActiveTab::Settings => 334.0,
        }
    }
}
