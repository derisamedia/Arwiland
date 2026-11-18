#![windows_subsystem = "windows"]

mod lyrics;
mod mpris;
mod notifications;
mod prayer;
mod renderer;
mod runtime;
mod settings_ui;
mod state;
mod style;
mod sysinfo;
mod x11_helper;

use chrono::{Local, Timelike};
use gtk4::gdk;
use gtk4::glib;
use gtk4::prelude::*;
use std::cell::RefCell;
use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use lyrics::LyricsManager;
use mpris::{seek_media_to, send_mpris_command};
use notifications::{set_system_notification_banners_hidden, NotificationHub};
use prayer::{
    calculate_prayer_schedule_for_region, resolve_region_from_state, search_regions_local,
    search_regions_online, PrayerRegion, INDONESIAN_CITIES,
};
use renderer::{
    draw_island_canvas, draw_media_card_lyrics_and_progress, draw_media_card_visualizer,
    draw_pango_layout_at, measure_unconstrained_text, measure_words_exact,
};
use runtime::{
    invoke_slot, make_circle_icon_btn, make_icon_pill_btn, make_icon_tab, new_callback_slot,
    set_icon_pill_content, spawn_background_monitor, CallbackSlot, RuntimeContext, CANVAS_HEIGHT,
    CANVAS_WIDTH, COMPACT_H, EXPANDED_W, NOTIF_TOAST_H, VOL_OSD_H, VOL_OSD_W,
};
use settings_ui::build_settings_window;
use state::{format_file_size, ShelfFile, TodoItem};
use style::APP_CSS;
use sysinfo::{get_live_volume_fast, set_system_volume, toggle_system_mute};
use x11_helper::{
    configure_pre_map_overlay, grab_keyboard_focus, setup_top_center_overlay,
    update_pill_input_region,
};

fn main() {
    #[cfg(target_os = "linux")]
    if std::env::var("DISPLAY").is_ok() {
        std::env::set_var("GDK_BACKEND", "x11");
    }
    if std::env::var("GSK_RENDERER").is_err() {
        std::env::set_var("GSK_RENDERER", "cairo");
    }

    let app = gtk4::Application::builder()
        .application_id("id.arwiland.DynamicIsland")
        .build();

    app.connect_activate(build_ui);
    app.run();
}

fn load_css() {
    let provider = gtk4::CssProvider::new();
    provider.load_from_data(APP_CSS);
    if let Some(display) = gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_USER + 200,
        );
    }
}

fn play_alarm_chime() {
    thread::spawn(|| {
        #[cfg(target_os = "linux")]
        {
            let sound_path = "/usr/share/sounds/freedesktop/stereo/alarm-clock-elapsed.oga";
            if std::path::Path::new(sound_path).exists() {
                if Command::new("paplay")
                    .arg(sound_path)
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false)
                {
                    return;
                }
                if Command::new("pw-play")
                    .arg(sound_path)
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false)
                {
                    return;
                }
            }
            let _ = Command::new("canberra-gtk-play")
                .args(["-i", "alarm-clock-elapsed"])
                .status();
        }
        #[cfg(target_os = "macos")]
        {
            let _ = Command::new("afplay")
                .arg("/System/Library/Sounds/Glass.aiff")
                .status();
        }
    });
}

fn stop_alarm_chime() {
    thread::spawn(|| {
        #[cfg(target_os = "linux")]
        {
            let _ = Command::new("pkill")
                .args(["-f", "alarm-clock-elapsed"])
                .status();
        }
    });
}

fn build_ui(app: &gtk4::Application) {
    if let Some(existing_win) = app.active_window() {
        existing_win.present();
        return;
    }

    load_css();

    let ctx = Rc::new(RefCell::new(RuntimeContext::new()));
    let notif_hub = NotificationHub::new(ctx.borrow().app_state.notifications.clone());
    set_system_notification_banners_hidden(
        ctx.borrow().app_state.notif_enabled && ctx.borrow().app_state.suppress_system_banners,
    );
    let lyrics_mgr = Rc::new(LyricsManager::new());
    let bg_monitor = spawn_background_monitor();
    let init_cw = ctx.borrow().app_state.compact_width.clamp(250, 420);

    let window = gtk4::ApplicationWindow::builder()
        .application(app)
        .title("Arwiland")
        .decorated(false)
        .resizable(false)
        .default_width(CANVAS_WIDTH)
        .default_height(CANVAS_HEIGHT)
        .build();
    window.remove_css_class("background");
    window.add_css_class("transparent-window");

    let canvas_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    canvas_box.set_size_request(CANVAS_WIDTH, CANVAS_HEIGHT);
    canvas_box.set_halign(gtk4::Align::Center);
    canvas_box.set_valign(gtk4::Align::Start);

    let island_overlay = gtk4::Overlay::new();
    island_overlay.set_size_request(CANVAS_WIDTH, CANVAS_HEIGHT);
    island_overlay.set_halign(gtk4::Align::Center);
    island_overlay.set_valign(gtk4::Align::Start);

    // =========================================================================
    // SUB-PIXEL VSYNC CAIRO + PANGO RENDERER (Notch + Live Notifications + Lyrics)
    // =========================================================================
    let bg_canvas = gtk4::DrawingArea::new();
    bg_canvas.set_size_request(CANVAS_WIDTH, CANVAS_HEIGHT);
    bg_canvas.set_hexpand(true);
    bg_canvas.set_vexpand(true);
    {
        let ctx_bg = ctx.clone();
        bg_canvas.set_draw_func(move |area, cr, width, _height| {
            let st = ctx_bg.borrow();
            draw_island_canvas(area, cr, width, &st);
        });
    }
    island_overlay.set_child(Some(&bg_canvas));

    // =========================================================================
    // 1. TOP BAR HIT REGION
    // =========================================================================
    let compact_hit_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    compact_hit_box.set_size_request(EXPANDED_W as i32, COMPACT_H as i32);
    compact_hit_box.set_halign(gtk4::Align::Center);
    compact_hit_box.set_valign(gtk4::Align::Start);
    island_overlay.add_overlay(&compact_hit_box);

    // =========================================================================
    // 2. EXPANDED BODY INSIDE ZERO-RESISTANCE VIEWPORT (`ScrolledWindow`)
    // =========================================================================
    let expanded_viewport = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::External)
        .vscrollbar_policy(gtk4::PolicyType::External)
        .propagate_natural_width(false)
        .propagate_natural_height(false)
        .min_content_width(0)
        .min_content_height(0)
        .build();
    expanded_viewport.set_halign(gtk4::Align::Center);
    expanded_viewport.set_valign(gtk4::Align::Start);
    expanded_viewport.set_margin_top(COMPACT_H as i32);
    expanded_viewport.set_size_request(
        (EXPANDED_W as i32) - 40,
        175 - (COMPACT_H as i32) - 4,
    );
    expanded_viewport.set_visible(false);
    expanded_viewport.set_opacity(0.0);

    let expanded_body = gtk4::Box::new(gtk4::Orientation::Vertical, 7);
    expanded_body.set_size_request((EXPANDED_W as i32) - 42, -1);
    expanded_body.set_halign(gtk4::Align::Center);
    expanded_body.set_valign(gtk4::Align::Start);
    expanded_body.set_margin_bottom(8);

    // Navigation Header with 6 Core Tabs + Uniform Circular Action Buttons
    let header_bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    header_bar.set_margin_top(4);

    let nav_strip = gtk4::Box::new(gtk4::Orientation::Horizontal, 2);
    nav_strip.add_css_class("nav-strip");

    let tab_defs = [
        ("media", "audio-x-generic-symbolic", "Media"),
        (
            "notif",
            "preferences-system-notifications-symbolic",
            "Notif",
        ),
        ("focus", "alarm-symbolic", "Fokus"),
        ("todo", "view-list-symbolic", "Tugas"),
        ("shelf", "folder-symbolic", "Shelf"),
        ("prayer", "weather-clear-night-symbolic", "Sholat"),
    ];

    let stack = gtk4::Stack::new();
    stack.set_transition_type(gtk4::StackTransitionType::SlideLeftRight);
    stack.set_transition_duration(220);
    stack.set_vhomogeneous(false);
    stack.set_hhomogeneous(true);
    stack.set_interpolate_size(false);
    stack.set_vexpand(false);

    let mut seg_buttons: Vec<(String, gtk4::Button, gtk4::Label, gtk4::DrawingArea)> = Vec::new();
    let mut notif_tab_btn_opt: Option<gtk4::Button> = None;
    let mut notif_tab_lbl_opt: Option<gtk4::Label> = None;
    let mut notif_tab_canvas_opt: Option<gtk4::DrawingArea> = None;

    for (idx, (id, icon, label)) in tab_defs.iter().enumerate() {
        let (btn, lbl, canvas) = make_icon_tab(icon, label);
        {
            let ctx_draw = ctx.clone();
            let lbl_draw = lbl.clone();
            let btn_draw = btn.clone();
            let tab_idx = idx;
            canvas.set_draw_func(move |area, cr, _w, h| {
                let p = ctx_draw.borrow().tab_spring_pos[tab_idx];
                if p <= 0.01 {
                    return;
                }
                let text = lbl_draw.text();
                let layout = area.create_pango_layout(Some(text.as_str()));
                let mut fd = gtk4::pango::FontDescription::from_string("Inter, Cantarell, Sans 8.3");
                fd.set_weight(gtk4::pango::Weight::Bold);
                layout.set_font_description(Some(&fd));
                let (_tw, th) = layout.pixel_size();

                let alpha = ((p - 0.06) / 0.55).clamp(0.0, 1.0);
                let is_active = btn_draw.has_css_class("active");
                let has_unread = btn_draw.has_css_class("has-unread");
                if is_active {
                    cr.set_source_rgba(0.035, 0.035, 0.043, alpha);
                } else if has_unread {
                    cr.set_source_rgba(0.204, 0.827, 0.600, alpha);
                } else {
                    cr.set_source_rgba(0.631, 0.631, 0.667, alpha);
                }

                let slide_x = 5.0 * (p.clamp(0.0, 1.15));
                let y = ((h as f64 - th as f64) * 0.5).round();
                draw_pango_layout_at(cr, &layout, slide_x, y);
            });
        }
        if idx == 0 {
            btn.add_css_class("active");
            let text_px = measure_unconstrained_text(&canvas, label, true);
            canvas.set_content_width((text_px + 6.0).round() as i32);
        } else {
            canvas.set_content_width(0);
        }
        if *id == "notif" {
            notif_tab_btn_opt = Some(btn.clone());
            notif_tab_lbl_opt = Some(lbl.clone());
            notif_tab_canvas_opt = Some(canvas.clone());
        }
        nav_strip.append(&btn);
        seg_buttons.push((id.to_string(), btn, lbl, canvas));
    }
    let notif_tab_btn = notif_tab_btn_opt.unwrap();
    let notif_tab_lbl = notif_tab_lbl_opt.unwrap();
    let notif_tab_canvas = notif_tab_canvas_opt.unwrap();

    let header_spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    header_spacer.set_hexpand(true);

    let startup_btn = make_circle_icon_btn(
        "system-run-symbolic",
        14,
        Some("Jalankan Otomatis Saat Startup (Run on Startup)"),
    );
    if ctx.borrow().app_state.run_on_startup {
        startup_btn.add_css_class("pinned");
    }

    let settings_btn = make_circle_icon_btn(
        "emblem-system-symbolic",
        14,
        Some("Buka Jendela Pengaturan Arwiland"),
    );
    let pin_btn = make_circle_icon_btn(
        "view-pin-symbolic",
        14,
        Some("Pin / Kunci agar tetap terbuka"),
    );
    if ctx.borrow().app_state.pinned {
        pin_btn.add_css_class("pinned");
    }
    let quit_btn = make_circle_icon_btn("window-close-symbolic", 14, Some("Keluar dari Arwiland"));

    header_bar.append(&nav_strip);
    header_bar.append(&header_spacer);
    header_bar.append(&startup_btn);
    header_bar.append(&settings_btn);
    header_bar.append(&pin_btn);
    header_bar.append(&quit_btn);

    expanded_body.append(&header_bar);
    expanded_body.append(&stack);

    // =========================================================================
    // PAGE 1: MEDIA PLAYER (With Progress Bar & 3-Line Karaoke Lyrics)
    // =========================================================================
    let media_page = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    media_page.add_css_class("glass-card");

    let media_top_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 14);
    media_top_row.set_valign(gtk4::Align::Center);

    let album_stack = gtk4::Stack::new();
    album_stack.add_css_class("album-frame");
    album_stack.set_size_request(60, 60);
    album_stack.set_valign(gtk4::Align::Center);
    album_stack.set_overflow(gtk4::Overflow::Hidden);

    let fallback_art_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    fallback_art_box.set_valign(gtk4::Align::Center);
    fallback_art_box.set_halign(gtk4::Align::Center);
    let fallback_art_icon = gtk4::Image::from_icon_name("audio-x-generic-symbolic");
    fallback_art_icon.set_pixel_size(26);
    fallback_art_box.append(&fallback_art_icon);

    let album_picture = gtk4::Picture::new();
    album_picture.set_content_fit(gtk4::ContentFit::Cover);
    album_picture.set_size_request(60, 60);

    album_stack.add_named(&fallback_art_box, Some("fallback"));
    album_stack.add_named(&album_picture, Some("art"));
    album_stack.set_visible_child_name("fallback");

    let track_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    track_vbox.set_hexpand(true);
    track_vbox.set_valign(gtk4::Align::Center);

    let media_source_lbl = gtk4::Label::new(Some("GOOGLE CHROME • AKTIF"));
    media_source_lbl.add_css_class("tag-label");
    media_source_lbl.set_halign(gtk4::Align::Start);

    let media_title_lbl = gtk4::Label::new(Some("Memuat media..."));
    media_title_lbl.add_css_class("heading-label");
    media_title_lbl.set_halign(gtk4::Align::Start);
    media_title_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);

    let media_artist_lbl = gtk4::Label::new(Some("MPRIS2 Linux Audio"));
    media_artist_lbl.add_css_class("sub-label");
    media_artist_lbl.set_halign(gtk4::Align::Start);
    media_artist_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);

    track_vbox.append(&media_source_lbl);
    track_vbox.append(&media_title_lbl);
    track_vbox.append(&media_artist_lbl);

    let ctrl_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    ctrl_box.set_valign(gtk4::Align::Center);

    let btn_prev = gtk4::Button::new();
    let prev_img = gtk4::Image::from_icon_name("media-skip-backward-symbolic");
    prev_img.set_pixel_size(15);
    btn_prev.set_child(Some(&prev_img));
    btn_prev.add_css_class("media-btn");
    btn_prev.set_tooltip_text(Some("Lagu Sebelumnya"));

    let btn_play = gtk4::Button::new();
    let play_img = gtk4::Image::from_icon_name("media-playback-start-symbolic");
    play_img.set_pixel_size(16);
    btn_play.set_child(Some(&play_img));
    btn_play.add_css_class("media-btn");
    btn_play.add_css_class("play-primary");
    btn_play.set_tooltip_text(Some("Putar / Jeda"));

    let btn_next = gtk4::Button::new();
    let next_img = gtk4::Image::from_icon_name("media-skip-forward-symbolic");
    next_img.set_pixel_size(15);
    btn_next.set_child(Some(&next_img));
    btn_next.add_css_class("media-btn");
    btn_next.set_tooltip_text(Some("Lagu Berikutnya"));

    let media_vis_canvas = gtk4::DrawingArea::new();
    media_vis_canvas.set_content_width(28);
    media_vis_canvas.set_content_height(26);
    media_vis_canvas.set_valign(gtk4::Align::Center);
    media_vis_canvas.set_margin_start(4);
    {
        let ctx_vis = ctx.clone();
        media_vis_canvas.set_draw_func(move |area, cr, w, h| {
            let st = ctx_vis.borrow();
            draw_media_card_visualizer(area, cr, w, h, &st);
        });
    }

    ctrl_box.append(&btn_prev);
    ctrl_box.append(&btn_play);
    ctrl_box.append(&btn_next);
    ctrl_box.append(&media_vis_canvas);

    media_top_row.append(&album_stack);
    media_top_row.append(&track_vbox);
    media_top_row.append(&ctrl_box);

    let media_lyric_canvas = gtk4::DrawingArea::new();
    media_lyric_canvas.set_content_height(106);
    media_lyric_canvas.set_hexpand(true);
    {
        let ctx_ml = ctx.clone();
        media_lyric_canvas.set_draw_func(move |area, cr, w, h| {
            let st = ctx_ml.borrow();
            draw_media_card_lyrics_and_progress(area, cr, w, h, &st);
        });
    }

    media_page.append(&media_top_row);
    media_page.append(&media_lyric_canvas);
    stack.add_named(&media_page, Some("media"));

    // =========================================================================
    // PAGE 2: NOTIFIKASI (Pusat Notifikasi Aplikasi & Sistem di Dynamic Island)
    // =========================================================================
    let notif_page = gtk4::Box::new(gtk4::Orientation::Vertical, 7);
    notif_page.add_css_class("glass-card");

    let notif_head = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    notif_head.set_valign(gtk4::Align::Center);

    let notif_head_icon = gtk4::Image::from_icon_name("preferences-system-notifications-symbolic");
    notif_head_icon.set_pixel_size(13);
    notif_head_icon.add_css_class("pill-icon");

    let notif_head_title = gtk4::Label::new(Some("PUSAT NOTIFIKASI"));
    notif_head_title.add_css_class("tag-label");

    let notif_status_lbl = gtk4::Label::new(Some("0 Belum Dibaca"));
    notif_status_lbl.add_css_class("provider-badge");

    let notif_head_spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    notif_head_spacer.set_hexpand(true);

    let btn_test_notif = make_icon_pill_btn(
        "preferences-system-notifications-symbolic",
        "Tes di Notch",
        true,
    );
    btn_test_notif.set_tooltip_text(Some("Kirim contoh notifikasi langsung ke dalam Notch"));

    let btn_mark_read = make_icon_pill_btn("object-select-symbolic", "Baca Semua", false);
    let btn_clear_notifs = make_icon_pill_btn("user-trash-symbolic", "Bersihkan", false);

    notif_head.append(&notif_head_icon);
    notif_head.append(&notif_head_title);
    notif_head.append(&notif_status_lbl);
    notif_head.append(&notif_head_spacer);
    notif_head.append(&btn_test_notif);
    notif_head.append(&btn_mark_read);
    notif_head.append(&btn_clear_notifs);

    let notif_scroll = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .propagate_natural_height(true)
        .max_content_height(155)
        .build();
    let notif_list_box = gtk4::Box::new(gtk4::Orientation::Vertical, 5);
    notif_scroll.set_child(Some(&notif_list_box));

    notif_page.append(&notif_head);
    notif_page.append(&notif_scroll);
    stack.add_named(&notif_page, Some("notif"));

    // =========================================================================
    // PAGE 3: FOKUS (Pomodoro + Alarm Pengingat)
    // =========================================================================
    let focus_page = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);

    let pomo_card = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    pomo_card.add_css_class("glass-card");
    pomo_card.set_hexpand(true);

    let pomo_top = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    let pomo_icon = gtk4::Image::from_icon_name("alarm-symbolic");
    pomo_icon.set_pixel_size(13);
    pomo_icon.add_css_class("pill-icon");
    let pomo_tag = gtk4::Label::new(Some("POMODORO FOKUS"));
    pomo_tag.add_css_class("tag-label");
    pomo_tag.set_hexpand(true);
    pomo_tag.set_halign(gtk4::Align::Start);

    let pomo_sess_lbl = gtk4::Label::new(Some(&format!(
        "{} Sesi",
        ctx.borrow().app_state.pomo_completed_sessions
    )));
    pomo_sess_lbl.add_css_class("muted-label");
    pomo_top.append(&pomo_icon);
    pomo_top.append(&pomo_tag);
    pomo_top.append(&pomo_sess_lbl);

    let pomo_digits = gtk4::Label::new(Some(&format!(
        "{:02}:00",
        ctx.borrow().app_state.pomo_focus_mins
    )));
    pomo_digits.add_css_class("timer-display");
    pomo_digits.set_halign(gtk4::Align::Start);
    pomo_digits.set_hexpand(true);

    let pomo_timer_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
    pomo_timer_row.set_valign(gtk4::Align::Center);

    let btn_pomo_sub5 = gtk4::Button::with_label("-5m");
    btn_pomo_sub5.add_css_class("pill-btn");
    btn_pomo_sub5.set_tooltip_text(Some("Kurangi waktu 5 menit"));

    let btn_pomo_sub1 = gtk4::Button::with_label("-1m");
    btn_pomo_sub1.add_css_class("pill-btn");
    btn_pomo_sub1.set_tooltip_text(Some("Kurangi waktu 1 menit"));

    let btn_pomo_add1 = gtk4::Button::with_label("+1m");
    btn_pomo_add1.add_css_class("pill-btn");
    btn_pomo_add1.set_tooltip_text(Some("Tambah waktu 1 menit"));

    let btn_pomo_add5 = gtk4::Button::with_label("+5m");
    btn_pomo_add5.add_css_class("pill-btn");
    btn_pomo_add5.set_tooltip_text(Some("Tambah waktu 5 menit"));

    pomo_timer_row.append(&pomo_digits);
    pomo_timer_row.append(&btn_pomo_sub5);
    pomo_timer_row.append(&btn_pomo_sub1);
    pomo_timer_row.append(&btn_pomo_add1);
    pomo_timer_row.append(&btn_pomo_add5);

    let pomo_bar = gtk4::ProgressBar::new();
    pomo_bar.set_fraction(1.0);

    let pomo_btns = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    let btn_pomo_start = make_icon_pill_btn("media-playback-start-symbolic", "Mulai", true);
    let btn_pomo_reset = make_icon_pill_btn("view-refresh-symbolic", "Reset", false);
    let btn_pomo_mode = gtk4::Button::with_label(&format!(
        "Rehat {}m",
        ctx.borrow().app_state.pomo_break_mins
    ));
    btn_pomo_mode.add_css_class("pill-btn");

    pomo_btns.append(&btn_pomo_start);
    pomo_btns.append(&btn_pomo_reset);
    pomo_btns.append(&btn_pomo_mode);

    pomo_card.append(&pomo_top);
    pomo_card.append(&pomo_timer_row);
    pomo_card.append(&pomo_bar);
    pomo_card.append(&pomo_btns);

    let alarm_card = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    alarm_card.add_css_class("glass-card");
    alarm_card.set_hexpand(true);

    let alarm_top = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    let alarm_icon = gtk4::Image::from_icon_name("preferences-system-time-symbolic");
    alarm_icon.set_pixel_size(13);
    alarm_icon.add_css_class("pill-icon");
    let alarm_tag = gtk4::Label::new(Some("ALARM PENGINGAT"));
    alarm_tag.add_css_class("tag-label");
    alarm_tag.set_hexpand(true);
    alarm_tag.set_halign(gtk4::Align::Start);

    let alarm_status_lbl = gtk4::Label::new(Some(if ctx.borrow().app_state.alarm_enabled {
        "AKTIF"
    } else {
        "NONAKTIF"
    }));
    alarm_status_lbl.add_css_class("muted-label");
    alarm_top.append(&alarm_icon);
    alarm_top.append(&alarm_tag);
    alarm_top.append(&alarm_status_lbl);

    let alarm_digits = gtk4::Label::new(Some(&format!(
        "{:02}:{:02}",
        ctx.borrow().app_state.alarm_hour.min(23),
        ctx.borrow().app_state.alarm_min.min(59)
    )));
    alarm_digits.add_css_class("timer-display");
    alarm_digits.set_halign(gtk4::Align::Start);
    alarm_digits.set_hexpand(true);

    let alarm_timer_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
    alarm_timer_row.set_valign(gtk4::Align::Center);

    let btn_alarm_add_h = gtk4::Button::with_label("+1j");
    btn_alarm_add_h.add_css_class("pill-btn");
    btn_alarm_add_h.set_tooltip_text(Some("Tambah 1 jam"));

    let btn_alarm_sub_m5 = gtk4::Button::with_label("-5m");
    btn_alarm_sub_m5.add_css_class("pill-btn");
    btn_alarm_sub_m5.set_tooltip_text(Some("Kurangi 5 menit"));

    let btn_alarm_add_m1 = gtk4::Button::with_label("+1m");
    btn_alarm_add_m1.add_css_class("pill-btn");
    btn_alarm_add_m1.set_tooltip_text(Some("Tambah 1 menit"));

    let btn_alarm_add_m5 = gtk4::Button::with_label("+5m");
    btn_alarm_add_m5.add_css_class("pill-btn");
    btn_alarm_add_m5.set_tooltip_text(Some("Tambah 5 menit"));

    alarm_timer_row.append(&alarm_digits);
    alarm_timer_row.append(&btn_alarm_add_h);
    alarm_timer_row.append(&btn_alarm_sub_m5);
    alarm_timer_row.append(&btn_alarm_add_m1);
    alarm_timer_row.append(&btn_alarm_add_m5);

    let alarm_bar = gtk4::ProgressBar::new();
    alarm_bar.set_fraction(if ctx.borrow().app_state.alarm_enabled {
        1.0
    } else {
        0.0
    });

    let alarm_btns = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    let init_alarm_on = ctx.borrow().app_state.alarm_enabled;
    let btn_alarm_toggle = make_icon_pill_btn(
        if init_alarm_on {
            "media-playback-stop-symbolic"
        } else {
            "alarm-symbolic"
        },
        if init_alarm_on { "Matikan" } else { "Aktifkan" },
        true,
    );
    let btn_alarm_snooze = gtk4::Button::with_label("Tunda 5m");
    btn_alarm_snooze.add_css_class("pill-btn");
    btn_alarm_snooze.set_tooltip_text(Some("Setel alarm 5 menit dari waktu sekarang"));

    let btn_alarm_sub_h = gtk4::Button::with_label("-1j");
    btn_alarm_sub_h.add_css_class("pill-btn");
    btn_alarm_sub_h.set_tooltip_text(Some("Kurangi 1 jam"));

    alarm_btns.append(&btn_alarm_toggle);
    alarm_btns.append(&btn_alarm_snooze);
    alarm_btns.append(&btn_alarm_sub_h);

    alarm_card.append(&alarm_top);
    alarm_card.append(&alarm_timer_row);
    alarm_card.append(&alarm_bar);
    alarm_card.append(&alarm_btns);

    focus_page.append(&pomo_card);
    focus_page.append(&alarm_card);
    stack.add_named(&focus_page, Some("focus"));

    // =========================================================================
    // PAGE 4: TUGAS (Daily To-Do List)
    // =========================================================================
    let todo_page = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    todo_page.add_css_class("glass-card");

    let todo_top = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    let todo_entry = gtk4::Entry::new();
    todo_entry.set_placeholder_text(Some("Tulis tugas atau catatan baru..."));
    todo_entry.add_css_class("island-entry");
    todo_entry.set_hexpand(true);

    let btn_todo_add = make_icon_pill_btn("list-add-symbolic", "Tambah", true);

    todo_top.append(&todo_entry);
    todo_top.append(&btn_todo_add);

    let todo_scroll = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .propagate_natural_height(true)
        .max_content_height(155)
        .build();
    let todo_list_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    todo_scroll.set_child(Some(&todo_list_box));

    todo_page.append(&todo_top);
    todo_page.append(&todo_scroll);
    stack.add_named(&todo_page, Some("todo"));

    // =========================================================================
    // PAGE 5: SHELF (File Dropzone)
    // =========================================================================
    let shelf_page = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    shelf_page.add_css_class("glass-card");

    let drop_bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    drop_bar.add_css_class("dropzone-area");

    let drop_icon = gtk4::Image::from_icon_name("folder-download-symbolic");
    drop_icon.set_pixel_size(16);
    drop_icon.add_css_class("pill-icon");

    let drop_lbl = gtk4::Label::new(Some("Tarik file ke sini atau pilih file"));
    drop_lbl.add_css_class("sub-label");
    drop_lbl.set_hexpand(true);
    drop_lbl.set_halign(gtk4::Align::Start);
    drop_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);

    let btn_pick_file = make_icon_pill_btn("document-open-symbolic", "Pilih", true);
    let btn_clear_shelf = make_icon_pill_btn("user-trash-symbolic", "Hapus", false);

    drop_bar.append(&drop_icon);
    drop_bar.append(&drop_lbl);
    drop_bar.append(&btn_pick_file);
    drop_bar.append(&btn_clear_shelf);

    let shelf_scroll = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .propagate_natural_height(true)
        .max_content_height(155)
        .build();
    let shelf_list_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    shelf_scroll.set_child(Some(&shelf_list_box));

    shelf_page.append(&drop_bar);
    shelf_page.append(&shelf_scroll);
    stack.add_named(&shelf_page, Some("shelf"));

    // =========================================================================
    // PAGE 6: SHOLAT (Jadwal Sholat & Hitung Mundur + Pilih Kota/Kecamatan)
    // =========================================================================
    let prayer_page = gtk4::Box::new(gtk4::Orientation::Vertical, 7);
    prayer_page.add_css_class("glass-card");

    let prayer_head = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    let prayer_info_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    prayer_info_vbox.set_hexpand(true);

    let prayer_title_lbl = gtk4::Label::new(Some("JADWAL SHOLAT"));
    prayer_title_lbl.add_css_class("tag-label");
    prayer_title_lbl.set_halign(gtk4::Align::Start);
    prayer_title_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);

    let prayer_next_lbl = gtk4::Label::new(Some("Dzuhur pukul 11:42 (-04:55:00)"));
    prayer_next_lbl.add_css_class("heading-label");
    prayer_next_lbl.set_halign(gtk4::Align::Start);
    prayer_next_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);

    prayer_info_vbox.append(&prayer_title_lbl);
    prayer_info_vbox.append(&prayer_next_lbl);

    let init_prayer_region = {
        let st = ctx.borrow();
        resolve_region_from_state(
            &st.app_state.selected_city,
            st.app_state.prayer_lat,
            st.app_state.prayer_lon,
            st.app_state.prayer_tz_offset,
            st.app_state.prayer_tz_label.as_deref(),
        )
    };
    let active_region_options: Rc<RefCell<Vec<PrayerRegion>>> = Rc::new(RefCell::new({
        let mut list = vec![init_prayer_region.clone()];
        for c in INDONESIAN_CITIES {
            if !list.iter().any(|r| r.name.eq_ignore_ascii_case(c.name)) {
                list.push(c.to_region());
            }
        }
        list
    }));
    let init_dd_labels: Vec<String> = active_region_options
        .borrow()
        .iter()
        .map(|r| format!("{} ({})", r.name, r.tz_label))
        .collect();
    let init_dd_refs: Vec<&str> = init_dd_labels.iter().map(|s| s.as_str()).collect();
    let region_str_list = gtk4::StringList::new(&init_dd_refs);
    let city_dropdown =
        gtk4::DropDown::new(Some(region_str_list.clone()), gtk4::Expression::NONE);
    city_dropdown.set_selected(0);
    city_dropdown.set_valign(gtk4::Align::Center);

    prayer_head.append(&prayer_info_vbox);
    prayer_head.append(&city_dropdown);

    let prayer_search_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    let prayer_search_entry = gtk4::Entry::new();
    prayer_search_entry.set_placeholder_text(Some(
        "Cari kota atau kecamatan (mis: Kebayoran Baru, Coblong, Gubeng)...",
    ));
    prayer_search_entry.add_css_class("island-entry");
    prayer_search_entry.set_hexpand(true);

    let btn_prayer_search = make_icon_pill_btn("system-search-symbolic", "Cari", true);
    btn_prayer_search.set_tooltip_text(Some(
        "Cari jadwal sholat sesuai Kota / Kabupaten / Kecamatan",
    ));

    prayer_search_row.append(&prayer_search_entry);
    prayer_search_row.append(&btn_prayer_search);

    let sholat_strip = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    sholat_strip.set_homogeneous(true);
    sholat_strip.set_valign(gtk4::Align::Center);

    let mut sholat_pills: Vec<(gtk4::Box, gtk4::Label, gtk4::Label)> = Vec::new();
    for _ in 0..6 {
        let p_box = gtk4::Box::new(gtk4::Orientation::Vertical, 3);
        p_box.add_css_class("sholat-card");
        p_box.set_valign(gtk4::Align::Center);

        let n_lbl = gtk4::Label::new(Some("Subuh"));
        n_lbl.add_css_class("sholat-name");
        let t_lbl = gtk4::Label::new(Some("04:08"));
        t_lbl.add_css_class("sholat-time");

        p_box.append(&n_lbl);
        p_box.append(&t_lbl);
        sholat_strip.append(&p_box);
        sholat_pills.push((p_box, n_lbl, t_lbl));
    }

    prayer_page.append(&prayer_head);
    prayer_page.append(&prayer_search_row);
    prayer_page.append(&sholat_strip);
    stack.add_named(&prayer_page, Some("prayer"));

    expanded_viewport.set_child(Some(&expanded_body));
    island_overlay.add_overlay(&expanded_viewport);
    canvas_box.append(&island_overlay);
    window.set_child(Some(&canvas_box));

    // =========================================================================
    // EXTERNAL SETTINGS WINDOW (`Pengaturan Arwiland`)
    // =========================================================================
    let settings_handles = build_settings_window(
        app,
        &ctx,
        &lyrics_mgr,
        &notif_hub,
        &window,
        &bg_canvas,
        &startup_btn,
    );
    let settings_win = settings_handles.window;
    let ext_provider_status_lbl = settings_handles.ext_provider_status_lbl;
    let btn_ext_test_notif = settings_handles.btn_ext_test_notif;
    let startup_chk = settings_handles.startup_chk;
    let ext_pomo_focus_scale = settings_handles.pomo_focus_scale;
    let ext_pomo_break_scale = settings_handles.pomo_break_scale;
    let ext_prayer_badge_lbl = settings_handles.ext_prayer_badge_lbl;

    {
        let sw = settings_win.clone();
        settings_btn.connect_clicked(move |_| {
            sw.present();
        });

        let st_chk1 = startup_chk;
        startup_btn.connect_clicked(move |_| {
            st_chk1.set_active(!st_chk1.is_active());
        });
    }

    // =========================================================================
    // NOTIFICATION CENTER LIST BUILDER (`rebuild_notifs`)
    // =========================================================================
    let rebuild_notifs: CallbackSlot = new_callback_slot();
    {
        let hub_n = notif_hub.clone();
        let ctx_n = ctx.clone();
        let box_n = notif_list_box.clone();
        let stat_lbl = notif_status_lbl.clone();
        let tab_btn = notif_tab_btn.clone();
        let tab_lbl = notif_tab_lbl.clone();
        let tab_canvas = notif_tab_canvas.clone();
        let self_n = rebuild_notifs.clone();

        *rebuild_notifs.borrow_mut() = Some(Box::new(move || {
            while let Some(child) = box_n.first_child() {
                box_n.remove(&child);
            }
            let history = hub_n.get_history();
            let unread = hub_n.unread_count();

            let notif_tab_p = {
                let mut st = ctx_n.borrow_mut();
                st.unread_notif_count = unread;
                st.app_state.notifications = history.clone();
                st.app_state.save();
                let exp_h = st.compute_tab_expanded_h();
                st.expanded_target_h = exp_h;
                if st.expanded {
                    st.target_h = exp_h;
                }
                st.tab_spring_pos[1]
            };

            if unread > 0 {
                stat_lbl.set_label(&format!("{} Baru", unread));
                tab_btn.add_css_class("has-unread");
                tab_lbl.set_label(&format!("Notif ({})", unread.min(99)));
            } else {
                stat_lbl.set_label(&format!("{} Riwayat", history.len()));
                tab_btn.remove_css_class("has-unread");
                tab_lbl.set_label("Notif");
            }
            if notif_tab_p > 0.01 {
                let base_w =
                    measure_unconstrained_text(&tab_canvas, tab_lbl.text().as_str(), true) + 6.0;
                tab_canvas.set_content_width((base_w * notif_tab_p.max(0.0)).round() as i32);
                tab_canvas.queue_draw();
            }

            if history.is_empty() {
                let empty_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
                empty_box.set_valign(gtk4::Align::Center);
                empty_box.set_halign(gtk4::Align::Center);
                empty_box.set_margin_top(6);
                empty_box.set_margin_bottom(6);
                let e_icon =
                    gtk4::Image::from_icon_name("preferences-system-notifications-symbolic");
                e_icon.set_pixel_size(24);
                e_icon.add_css_class("pill-icon");
                let e_title = gtk4::Label::new(Some("Belum ada notifikasi masuk"));
                e_title.add_css_class("pill-primary-text");
                let e_sub = gtk4::Label::new(Some(
                    "Semua notifikasi aplikasi & sistem Linux otomatis tampil di Notch.",
                ));
                e_sub.add_css_class("sub-label");
                empty_box.append(&e_icon);
                empty_box.append(&e_title);
                empty_box.append(&e_sub);
                box_n.append(&empty_box);
                return;
            }

            for item in history {
                let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
                row.add_css_class("notif-item-card");
                if item.urgency >= 2 {
                    row.add_css_class("critical");
                } else if !item.read {
                    row.add_css_class("unread");
                }

                let badge_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
                badge_box.set_valign(gtk4::Align::Center);
                badge_box.set_halign(gtk4::Align::Center);
                badge_box.add_css_class("notif-icon-badge");
                let b_cls = item.badge_color_class();
                if b_cls != "emerald" {
                    badge_box.add_css_class(b_cls);
                }
                let n_img = gtk4::Image::from_icon_name(item.resolved_icon_name());
                n_img.set_pixel_size(15);
                badge_box.append(&n_img);

                let text_col = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
                text_col.set_hexpand(true);
                text_col.set_valign(gtk4::Align::Center);

                let meta_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
                let app_lbl = gtk4::Label::new(Some(&item.app_name.to_uppercase()));
                app_lbl.add_css_class("tag-label");
                meta_row.append(&app_lbl);

                if item.urgency >= 2 {
                    let urg_lbl = gtk4::Label::new(Some("PENTING"));
                    urg_lbl.add_css_class("notif-urgent-tag");
                    meta_row.append(&urg_lbl);
                }

                let meta_spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
                meta_spacer.set_hexpand(true);
                meta_row.append(&meta_spacer);

                let time_lbl = gtk4::Label::new(Some(&item.timestamp_str));
                time_lbl.add_css_class("muted-label");
                meta_row.append(&time_lbl);

                let sum_lbl = gtk4::Label::new(Some(&item.summary));
                sum_lbl.add_css_class("pill-primary-text");
                sum_lbl.set_halign(gtk4::Align::Start);
                sum_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);

                text_col.append(&meta_row);
                text_col.append(&sum_lbl);

                if !item.body.is_empty() && item.body != item.summary {
                    let body_lbl = gtk4::Label::new(Some(&item.body));
                    body_lbl.add_css_class("sub-label");
                    body_lbl.set_halign(gtk4::Align::Start);
                    body_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                    row.set_tooltip_text(Some(&format!("{}: {}", item.summary, item.body)));
                    text_col.append(&body_lbl);
                } else {
                    row.set_tooltip_text(Some(&item.summary));
                }

                let del_btn =
                    make_circle_icon_btn("window-close-symbolic", 12, Some("Hapus notifikasi ini"));

                let id_del = item.id;
                let hub_d = hub_n.clone();
                let reb_d = self_n.clone();
                del_btn.connect_clicked(move |_| {
                    hub_d.remove_by_id(id_del);
                    invoke_slot(&reb_d);
                });

                row.append(&badge_box);
                row.append(&text_col);
                row.append(&del_btn);
                box_n.append(&row);
            }
        }));
    }
    invoke_slot(&rebuild_notifs);

    // =========================================================================
    // HARDWARE VSYNC FRAME-CLOCK SPRING PHYSICS & LYRIC ENGINE (`add_tick_callback`)
    // =========================================================================
    let all_seg_tabs = Rc::new(seg_buttons.clone());
    let set_expanded_state: Rc<dyn Fn(bool)> = {
        let ctx_e = ctx.clone();
        let vp_c = expanded_viewport.clone();
        let win_c = window.clone();
        let hit_c = compact_hit_box.clone();
        let st_c = stack.clone();
        let tabs_c = all_seg_tabs.clone();
        let hub_e = notif_hub.clone();

        Rc::new(move |expand: bool| {
            let mut switch_to_notif = false;
            {
                let mut st = ctx_e.borrow_mut();
                if st.expanded == expand {
                    return;
                }
                st.expanded = expand;
                if expand {
                    if st.active_notif_toast.is_some() && st.notif_toast_remaining > 0.0 {
                        st.active_notif_toast = None;
                        st.notif_toast_remaining = 0.0;
                        switch_to_notif = true;
                        st.active_tab_id = "notif".to_string();
                    }
                    let exp_h = st.compute_tab_expanded_h();
                    st.expanded_target_h = exp_h;
                    st.target_w = EXPANDED_W;
                    st.target_h = exp_h;
                    vp_c.set_visible(true);
                    hit_c.set_size_request(EXPANDED_W as i32, COMPACT_H as i32);
                    update_pill_input_region(
                        &win_c,
                        CANVAS_WIDTH,
                        EXPANDED_W as i32 + 12,
                        exp_h as i32 + 8,
                    );
                    st.last_input_w = EXPANDED_W as i32;
                    st.last_input_h = exp_h as i32;
                } else if st.vol_osd_remaining > 0.0 {
                    st.target_w = VOL_OSD_W;
                    st.target_h = VOL_OSD_H;
                    hit_c.set_size_request(EXPANDED_W as i32, VOL_OSD_H as i32);
                } else if st.active_notif_toast.is_some() && st.notif_toast_remaining > 0.0 {
                    st.target_w = st.notif_compact_w;
                    st.target_h = NOTIF_TOAST_H;
                    hit_c.set_size_request(st.notif_compact_w as i32, NOTIF_TOAST_H as i32);
                } else {
                    st.target_w = st.dynamic_compact_w;
                    st.target_h = COMPACT_H;
                    hit_c.set_size_request(EXPANDED_W as i32, COMPACT_H as i32);
                }
            }
            if switch_to_notif {
                st_c.set_visible_child_name("notif");
                let mut st = ctx_e.borrow_mut();
                for (i, (id, btn, _lbl, _canvas)) in tabs_c.iter().enumerate() {
                    if id == "notif" {
                        btn.add_css_class("active");
                        if i < 6 {
                            st.tab_spring_target[i] = 1.0;
                            st.tab_spring_vel[i] += 2.2;
                        }
                    } else {
                        btn.remove_css_class("active");
                        if i < 6 {
                            st.tab_spring_target[i] = 0.0;
                        }
                    }
                }
                hub_e.mark_all_read();
            }
        })
    };

    {
        let ctx_tick = ctx.clone();
        let l_mgr = lyrics_mgr.clone();
        let bg_mon = bg_monitor.clone();
        let hub_tick = notif_hub.clone();
        let reb_n_tick = rebuild_notifs.clone();
        let vp_s = expanded_viewport.clone();
        let body_s = expanded_body.clone();
        let hit_s = compact_hit_box.clone();
        let bg_s = bg_canvas.clone();
        let m_vis_s = media_vis_canvas.clone();
        let m_lyr_s = media_lyric_canvas.clone();
        let win_s = window.clone();
        let ext_prov_lbl = ext_provider_status_lbl.clone();
        let tabs_tick = all_seg_tabs.clone();
        let hdr_s = header_bar.clone();
        let st_tick = stack.clone();

        island_overlay.add_tick_callback(move |_overlay_widget, frame_clock| {
            let now_us = frame_clock.frame_time();
            let mut rebuild_notif_ui = false;
            let mut tab_updates: Vec<(usize, f64, bool)> = Vec::new();

            let needs_redraw = {
                let mut st = ctx_tick.borrow_mut();

                let dt = if st.last_frame_us == 0 {
                    0.016
                } else {
                    ((now_us - st.last_frame_us) as f64 / 1_000_000.0).clamp(0.001, 0.033)
                };
                st.last_frame_us = now_us;

                if st.expanded {
                    let w_req = (EXPANDED_W as i32) - 42;
                    let (_, nat_hdr_h, _, _) =
                        hdr_s.measure(gtk4::Orientation::Vertical, w_req);
                    let nat_page_h = st_tick
                        .visible_child()
                        .map(|c| c.measure(gtk4::Orientation::Vertical, w_req).1)
                        .unwrap_or(84);
                    if nat_page_h > 20 {
                        let exact_exp_h = (COMPACT_H
                            + 4.0
                            + nat_hdr_h as f64
                            + 7.0
                            + nat_page_h as f64
                            + 16.0)
                            .clamp(158.0, 340.0);
                        st.expanded_target_h = exact_exp_h;
                        if (st.target_h - exact_exp_h).abs() > 0.5 {
                            st.target_h = exact_exp_h;
                        }
                    }
                }

                let recently_seeked = st
                    .last_media_seek_at
                    .map(|t| t.elapsed() < Duration::from_millis(1800))
                    .unwrap_or(false);
                let mut media_state_dirty = false;
                if let Ok(guard) = bg_mon.try_lock() {
                    if guard.media_seq != st.last_seen_seq {
                        st.last_seen_seq = guard.media_seq;
                        let new_m = guard.media.clone();
                        let track_changed = st.cached_media.title != new_m.title
                            || st.cached_media.artist != new_m.artist;
                        let play_state_changed = st.cached_media.playing != new_m.playing;
                        if track_changed
                            || (!st.media_seek_dragging
                                && !recently_seeked
                                && ((st.interp_pos_sec - new_m.position_f64).abs() > 0.85
                                    || play_state_changed
                                    || !new_m.playing))
                        {
                            st.interp_pos_sec = new_m.position_f64;
                        }
                        if track_changed || play_state_changed {
                            media_state_dirty = true;
                        }
                        if track_changed {
                            st.prev_prev_lyric.clear();
                            st.prev_lyric.clear();
                            st.cur_lyric.clear();
                            st.next_lyric.clear();
                            st.lyric_words.clear();
                            st.active_word_idx = 0;
                            st.last_word_idx = 0;
                            st.lyric_anim_prog = 1.0;
                        }
                        st.cached_media = new_m;
                    }
                }

                // Check real-time Volume & Mute changes via lock-free 45ms atomic monitor
                let recently_dragged = st
                    .last_vol_set_at
                    .map(|t| t.elapsed() < Duration::from_millis(260))
                    .unwrap_or(false);
                if !st.vol_osd_dragging && !recently_dragged {
                    let (hw_vol, hw_mute, hw_ready) = get_live_volume_fast();
                    if hw_ready {
                        let is_first = st.prev_volume_pct.is_none();
                        let vol_changed = st
                            .prev_volume_pct
                            .map(|p| p.abs_diff(hw_vol) >= 1)
                            .unwrap_or(false)
                            || st.prev_volume_muted != Some(hw_mute);

                        st.cur_volume_pct = hw_vol;
                        st.cur_volume_muted = hw_mute;
                        st.prev_volume_pct = Some(hw_vol);
                        st.prev_volume_muted = Some(hw_mute);

                        if is_first {
                            st.disp_volume_pct = hw_vol as f64;
                        } else if vol_changed && !st.expanded {
                            if st.vol_osd_remaining <= 0.0 {
                                st.vol_osd_anim_prog = 0.0;
                                st.vel_w += 95.0;
                                st.vel_h += 38.0;
                            }
                            st.vol_osd_remaining = 2.2;
                            st.target_w = VOL_OSD_W;
                            st.target_h = VOL_OSD_H;
                            hit_s.set_size_request(EXPANDED_W as i32, VOL_OSD_H as i32);
                        }
                    }
                }

                let is_playing = st.cached_media.has_player && st.cached_media.playing;
                let mut redraw = media_state_dirty;

                // Smoothly interpolate displayed volume bar when not actively dragging
                if !st.vol_osd_dragging {
                    let vol_diff = st.cur_volume_pct as f64 - st.disp_volume_pct;
                    if vol_diff.abs() > 0.04 {
                        st.disp_volume_pct += vol_diff * (dt * 32.0).min(1.0);
                        redraw = true;
                    } else {
                        st.disp_volume_pct = st.cur_volume_pct as f64;
                    }
                }

                if st.vol_osd_remaining > 0.0 {
                    st.vol_osd_anim_prog = (st.vol_osd_anim_prog + dt * 12.0).min(1.0);
                    if !st.vol_osd_dragging {
                        st.vol_osd_remaining = (st.vol_osd_remaining - dt).max(0.0);
                    }
                    redraw = true;

                    if !st.expanded {
                        if st.vol_osd_remaining > 0.0 {
                            st.target_w = VOL_OSD_W;
                            st.target_h = VOL_OSD_H;
                        } else if st.active_notif_toast.is_some() && st.notif_toast_remaining > 0.0
                        {
                            st.target_w = st.notif_compact_w;
                            st.target_h = NOTIF_TOAST_H;
                            hit_s.set_size_request(st.notif_compact_w as i32, NOTIF_TOAST_H as i32);
                        } else {
                            st.target_w = st.dynamic_compact_w;
                            st.target_h = COMPACT_H;
                            hit_s.set_size_request(EXPANDED_W as i32, COMPACT_H as i32);
                        }
                    }
                }

                // 1. Check Notification Hub updates & pop Live Toast into Notch
                let cur_hub_seq = hub_tick.seq();
                if cur_hub_seq != st.last_notif_seq {
                    st.last_notif_seq = cur_hub_seq;
                    rebuild_notif_ui = true;
                    redraw = true;
                }

                if !st.app_state.notif_enabled {
                    while hub_tick.pop_next_toast().is_some() {}
                    if st.active_notif_toast.is_some() {
                        st.active_notif_toast = None;
                        st.notif_toast_remaining = 0.0;
                        if !st.expanded {
                            st.target_w = st.dynamic_compact_w;
                            st.target_h = COMPACT_H;
                            hit_s.set_size_request(EXPANDED_W as i32, COMPACT_H as i32);
                        }
                        redraw = true;
                    }
                } else {
                    let can_replace = st.active_notif_toast.is_none()
                        || st.notif_toast_remaining <= 0.0
                        || (st.notif_toast_total - st.notif_toast_remaining) >= 1.4;
                    if can_replace {
                        if let Some(next_toast) = hub_tick.pop_next_toast() {
                            let dur = st.app_state.notif_duration_secs.clamp(2, 12) as f64;

                            let sum_px =
                                measure_unconstrained_text(&bg_s, &next_toast.summary, true);
                            let body_px =
                                measure_unconstrained_text(&bg_s, &next_toast.body, false)
                                    .min(390.0);
                            let tag_px = measure_unconstrained_text(
                                &bg_s,
                                &format!("{} • 00:00  Notch • 00:00", next_toast.app_name),
                                true,
                            );
                            let content_px = sum_px.max(body_px).max(tag_px);
                            let ideal_nw = (112.0 + content_px).clamp(360.0, 560.0);

                            st.notif_compact_w = ideal_nw;
                            st.notif_toast_total = dur;
                            st.notif_toast_remaining = dur;
                            st.notif_anim_prog = 0.0;
                            st.notif_scroll_phase = 0.0;
                            st.vel_w += 95.0;
                            st.vel_h += 38.0;
                            st.active_notif_toast = Some(next_toast);

                            if !st.expanded && st.vol_osd_remaining <= 0.0 {
                                st.target_w = ideal_nw;
                                st.target_h = NOTIF_TOAST_H;
                                hit_s.set_size_request(ideal_nw as i32, NOTIF_TOAST_H as i32);
                            }
                            redraw = true;
                        }
                    }

                    if st.active_notif_toast.is_some() && st.notif_toast_remaining > 0.0 {
                        st.wave_phase += dt * 4.2;
                        st.notif_anim_prog = (st.notif_anim_prog + dt * 3.8).min(1.0);
                        st.notif_scroll_phase += dt;
                        st.notif_toast_remaining = (st.notif_toast_remaining - dt).max(0.0);
                        redraw = true;

                        if !st.expanded && st.vol_osd_remaining <= 0.0 {
                            st.target_w = st.notif_compact_w;
                            st.target_h = NOTIF_TOAST_H;
                        }

                        if st.notif_toast_remaining <= 0.0 {
                            st.active_notif_toast = None;
                            if !st.expanded && st.vol_osd_remaining <= 0.0 {
                                st.target_w = st.dynamic_compact_w;
                                st.target_h = COMPACT_H;
                                hit_s.set_size_request(EXPANDED_W as i32, COMPACT_H as i32);
                            }
                        }
                    }
                }

                let has_active_toast = !st.expanded
                    && (st.vol_osd_remaining > 0.0
                        || (st.active_notif_toast.is_some() && st.notif_toast_remaining > 0.0));

                if is_playing
                    || (st.cached_media.has_player && (st.media_seek_dragging || recently_seeked))
                {
                    if is_playing && !st.media_seek_dragging {
                        let max_len = st.cached_media.length_secs as f64;
                        if max_len > 0.5 {
                            st.interp_pos_sec = (st.interp_pos_sec + dt).min(max_len);
                        } else {
                            st.interp_pos_sec += dt;
                        }
                        if !has_active_toast {
                            st.wave_phase += dt * 5.8;
                        }
                    }
                    redraw = true;

                    let prov_pref = st.app_state.lyric_provider.clone();
                    let offset_ms = st.app_state.lyric_offset_ms;
                    let max_lw = st.app_state.max_lyric_width.clamp(480, 900) as f64;

                    let snap = l_mgr.get_lyric_snapshot(
                        &st.cached_media.title,
                        &st.cached_media.artist,
                        st.interp_pos_sec,
                        st.cached_media.length_secs as f64,
                        &prov_pref,
                        offset_ms,
                    );

                    st.lyric_line_prog = snap.line_progress;
                    st.lyric_is_word_synced = snap.is_word_synced;
                    st.active_word_idx = snap.active_word_idx;
                    st.word_prog = snap.word_progress;
                    st.lyric_accent_rgb = snap.accent_rgb;
                    st.next_lyric = snap.next_line_text.clone();

                    if st.lyric_provider_badge != snap.provider_badge
                        || st.lyric_provider_detail != snap.provider_detail
                    {
                        st.lyric_provider_badge = snap.provider_badge.clone();
                        st.lyric_provider_detail = snap.provider_detail.clone();
                        ext_prov_lbl.set_label(&snap.provider_detail);
                    }

                    if st.active_word_idx != st.last_word_idx {
                        st.last_word_idx = st.active_word_idx;
                        if st.lyric_is_word_synced
                            && !st.expanded
                            && !has_active_toast
                            && st.app_state.lyric_style == "word_bounce"
                        {
                            st.vel_w += 22.0;
                        }
                    }

                    if snap.current_text != st.cur_lyric {
                        st.prev_prev_lyric = st.prev_lyric.clone();
                        st.prev_lyric = snap.prev_line_text.clone();
                        st.cur_lyric = snap.current_text.clone();
                        st.lyric_words = snap.words;
                        st.last_word_idx = 0;
                        st.lyric_anim_prog = 0.0;

                        let lyric_px = measure_words_exact(&bg_s, &st.cur_lyric, &st.lyric_words);
                        let time_px = measure_unconstrained_text(&bg_s, &st.clock_str, true);
                        let ideal_w = (108.0 + time_px + lyric_px).clamp(260.0, max_lw);
                        st.dynamic_compact_w = ideal_w;

                        if !st.expanded && !has_active_toast {
                            st.target_w = ideal_w;
                        }
                    }

                    if st.lyric_anim_prog < 1.0 {
                        st.lyric_anim_prog = (st.lyric_anim_prog + dt * 3.1).min(1.0);
                    }
                } else {
                    let base_cw = st.app_state.compact_width.clamp(250, 420) as f64;
                    st.dynamic_compact_w = base_cw;
                    if !st.expanded && !has_active_toast {
                        st.target_w = base_cw;
                    }
                }

                let dw = st.target_w - st.cur_w;
                let dh = st.target_h - st.cur_h;

                if dw.abs() > 0.05
                    || dh.abs() > 0.05
                    || st.vel_w.abs() > 0.4
                    || st.vel_h.abs() > 0.4
                {
                    let preset = st.app_state.anim_preset.as_str();
                    let compact_breathing = !st.expanded && (st.cur_h - COMPACT_H).abs() < 2.0;
                    let growing_w = st.target_w > st.cur_w + 1.0;

                    let (omega_w, zeta_w, omega_h, zeta_h) =
                        if !st.expanded && st.vol_osd_remaining > 0.0 {
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
                        let err_w = st.target_w - st.cur_w;
                        let acc_w =
                            (omega_w * omega_w) * err_w - (2.0 * zeta_w * omega_w) * st.vel_w;
                        st.vel_w += acc_w * sub_dt;
                        if compact_breathing {
                            st.vel_w = st.vel_w.clamp(-650.0, 650.0);
                        }
                        st.cur_w += st.vel_w * sub_dt;

                        let err_h = st.target_h - st.cur_h;
                        let acc_h =
                            (omega_h * omega_h) * err_h - (2.0 * zeta_h * omega_h) * st.vel_h;
                        st.vel_h += acc_h * sub_dt;
                        st.cur_h += st.vel_h * sub_dt;
                    }

                    redraw = true;

                    let exp_h = st.expanded_target_h.max(160.0);
                    let raw_prog =
                        ((st.cur_h - COMPACT_H) / (exp_h - COMPACT_H)).clamp(0.0, 1.0);
                    let opacity = if st.expanded {
                        ((raw_prog - 0.12) / 0.88).clamp(0.0, 1.0)
                    } else {
                        ((raw_prog - 0.22) / 0.78).clamp(0.0, 1.0)
                    };
                    vp_s.set_opacity(opacity);

                    let slide_px = ((1.0 - raw_prog) * 6.0).round() as i32;
                    body_s.set_margin_top(slide_px);

                    let dyn_vp_h = ((st.cur_h - COMPACT_H) - 4.0).round().max(80.0) as i32;
                    vp_s.set_size_request((EXPANDED_W as i32) - 40, dyn_vp_h);

                    if !st.expanded && raw_prog < 0.03 {
                        vp_s.set_visible(false);
                    }

                    let w_i = st.cur_w.round() as i32;
                    let h_i = st.cur_h.round() as i32;
                    if (st.last_input_w - w_i).abs() >= 6 || (st.last_input_h - h_i).abs() >= 6 {
                        st.last_input_w = w_i;
                        st.last_input_h = h_i;
                        update_pill_input_region(&win_s, CANVAS_WIDTH, w_i + 8, h_i + 6);
                    }
                } else if (st.cur_w - st.target_w).abs() > 0.005
                    || (st.cur_h - st.target_h).abs() > 0.005
                {
                    st.cur_w = st.target_w;
                    st.cur_h = st.target_h;
                    st.vel_w = 0.0;
                    st.vel_h = 0.0;
                    redraw = true;
                    body_s.set_margin_top(0);

                    if st.expanded {
                        let dyn_vp_h = ((st.cur_h - COMPACT_H) - 4.0).round().max(80.0) as i32;
                        vp_s.set_size_request((EXPANDED_W as i32) - 40, dyn_vp_h);
                    }

                    if !st.expanded {
                        vp_s.set_visible(false);
                        vp_s.set_opacity(0.0);
                    } else {
                        vp_s.set_opacity(1.0);
                    }

                    let w_i = st.cur_w.round() as i32;
                    let h_i = st.cur_h.round() as i32;
                    if st.last_input_w != w_i || st.last_input_h != h_i {
                        st.last_input_w = w_i;
                        st.last_input_h = h_i;
                        update_pill_input_region(&win_s, CANVAS_WIDTH, w_i + 8, h_i + 6);
                    }
                }

                // Elastic spring physics for segmented tab pills
                for i in 0..6 {
                    let target = st.tab_spring_target[i];
                    let pos = st.tab_spring_pos[i];
                    let vel = st.tab_spring_vel[i];
                    if (target - pos).abs() > 0.002 || vel.abs() > 0.01 {
                        let (omega, zeta) = if target > 0.5 {
                            (22.0, 0.64) // smooth elastic spring expansion
                        } else {
                            (25.0, 0.88) // clean smooth collapse
                        };
                        let sub_dt = dt * 0.5;
                        for _ in 0..2 {
                            let err = st.tab_spring_target[i] - st.tab_spring_pos[i];
                            let acc = (omega * omega) * err
                                - (2.0 * zeta * omega) * st.tab_spring_vel[i];
                            st.tab_spring_vel[i] += acc * sub_dt;
                            st.tab_spring_pos[i] += st.tab_spring_vel[i] * sub_dt;
                        }
                        if st.tab_spring_pos[i] < 0.0 && target == 0.0 {
                            st.tab_spring_pos[i] = 0.0;
                            st.tab_spring_vel[i] = 0.0;
                        }
                        tab_updates.push((i, st.tab_spring_pos[i], false));
                    } else if (pos - target).abs() > 0.0001 || vel.abs() > 0.0 {
                        st.tab_spring_pos[i] = target;
                        st.tab_spring_vel[i] = 0.0;
                        tab_updates.push((i, target, true));
                    }
                }

                redraw
            };

            for (idx, p, settled) in tab_updates {
                if let Some((_, _, lbl, canvas)) = tabs_tick.get(idx) {
                    if settled && p <= 0.001 {
                        canvas.set_content_width(0);
                        canvas.queue_draw();
                    } else {
                        let text = lbl.text();
                        let base_w = measure_unconstrained_text(canvas, text.as_str(), true) + 6.0;
                        let target_px = if settled && p >= 0.999 {
                            base_w.round() as i32
                        } else {
                            (base_w * p.max(0.0)).round() as i32
                        };
                        if canvas.content_width() != target_px {
                            canvas.set_content_width(target_px);
                        }
                        canvas.queue_draw();
                    }
                }
            }

            if rebuild_notif_ui {
                invoke_slot(&reb_n_tick);
            }

            if needs_redraw {
                bg_s.queue_draw();
                m_vis_s.queue_draw();
                m_lyr_s.queue_draw();
            }

            glib::ControlFlow::Continue
        });
    }

    // =========================================================================
    // NOTIFICATION CENTER HANDLERS
    // =========================================================================
    {
        let trigger_sample_notif: Rc<dyn Fn()> = {
            let hub_t = notif_hub.clone();
            let ctx_t = ctx.clone();
            let set_exp_t = set_expanded_state.clone();
            let sample_idx = Rc::new(RefCell::new(0usize));

            Rc::new(move || {
                {
                    let mut st = ctx_t.borrow_mut();
                    st.is_hovered = false;
                }
                set_exp_t(false);

                let samples = [
                    (
                        "WhatsApp",
                        "user-available-symbolic",
                        "Budi Santoso",
                        "Halo! Notifikasi WhatsApp & aplikasi apa pun kini langsung tampil rapi di dalam Notch Arwiland.",
                        1u8,
                        "chat",
                    ),
                    (
                        "Spotify",
                        "audio-x-generic-symbolic",
                        "Lagu Berikutnya Diputar",
                        "Hindia — Evaluasi • Sinkronisasi lirik & spektrum audio aktif di Dynamic Island.",
                        1u8,
                        "media",
                    ),
                    (
                        "Keamanan Sistem",
                        "dialog-warning-symbolic",
                        "Peringatan Baterai & Daya",
                        "Contoh notifikasi prioritas penting (Urgent) dengan aksen merah muda di Notch.",
                        2u8,
                        "device",
                    ),
                    (
                        "Jadwal Sholat",
                        "weather-clear-night-symbolic",
                        "Waktu Sholat Segera Tiba",
                        "10 menit menuju waktu sholat berikutnya. Siapkan diri untuk beribadah.",
                        1u8,
                        "prayer",
                    ),
                ];

                let idx = {
                    let mut c = sample_idx.borrow_mut();
                    let cur = *c % samples.len();
                    *c = (*c + 1) % samples.len();
                    cur
                };
                let (app_n, icon_n, sum_n, body_n, urg_n, cat_n) = samples[idx];
                hub_t.push_notification(app_n, icon_n, sum_n, body_n, urg_n, cat_n);
            })
        };

        let tn1 = trigger_sample_notif.clone();
        btn_test_notif.connect_clicked(move |_| tn1());
        let tn2 = trigger_sample_notif;
        btn_ext_test_notif.connect_clicked(move |_| tn2());

        let hub_mr = notif_hub.clone();
        let reb_mr = rebuild_notifs.clone();
        btn_mark_read.connect_clicked(move |_| {
            hub_mr.mark_all_read();
            invoke_slot(&reb_mr);
        });

        let hub_cl = notif_hub.clone();
        let reb_cl = rebuild_notifs.clone();
        btn_clear_notifs.connect_clicked(move |_| {
            hub_cl.clear_all();
            invoke_slot(&reb_cl);
        });
    }

    // =========================================================================
    // DYNAMIC LIST BUILDERS (To-Do & File Shelf)
    // =========================================================================
    let rebuild_todos: CallbackSlot = new_callback_slot();
    {
        let ctx_t = ctx.clone();
        let box_t = todo_list_box.clone();
        let self_t = rebuild_todos.clone();

        *rebuild_todos.borrow_mut() = Some(Box::new(move || {
            while let Some(child) = box_t.first_child() {
                box_t.remove(&child);
            }
            let list = {
                let mut st = ctx_t.borrow_mut();
                let exp_h = st.compute_tab_expanded_h();
                st.expanded_target_h = exp_h;
                if st.expanded {
                    st.target_h = exp_h;
                }
                st.app_state.todos.clone()
            };
            if list.is_empty() {
                let lbl = gtk4::Label::new(Some("Belum ada tugas hari ini."));
                lbl.add_css_class("sub-label");
                lbl.set_margin_top(6);
                lbl.set_margin_bottom(4);
                box_t.append(&lbl);
                return;
            }
            for item in list {
                let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
                row.add_css_class("list-row-card");
                let chk = gtk4::CheckButton::new();
                chk.set_active(item.completed);

                let lbl = gtk4::Label::new(Some(&item.text));
                lbl.set_hexpand(true);
                lbl.set_halign(gtk4::Align::Start);
                lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                if item.completed {
                    lbl.add_css_class("muted-label");
                } else {
                    lbl.add_css_class("pill-primary-text");
                }

                let del = make_circle_icon_btn("user-trash-symbolic", 12, None);

                let id_c = item.id;
                let ctx_c = ctx_t.clone();
                let reb_c = self_t.clone();
                chk.connect_toggled(move |c| {
                    {
                        let mut st = ctx_c.borrow_mut();
                        if let Some(t) = st.app_state.todos.iter_mut().find(|x| x.id == id_c) {
                            t.completed = c.is_active();
                        }
                        st.app_state.save();
                    }
                    invoke_slot(&reb_c);
                });

                let id_d = item.id;
                let ctx_d = ctx_t.clone();
                let reb_d = self_t.clone();
                del.connect_clicked(move |_| {
                    {
                        let mut st = ctx_d.borrow_mut();
                        st.app_state.todos.retain(|x| x.id != id_d);
                        st.app_state.save();
                    }
                    invoke_slot(&reb_d);
                });

                row.append(&chk);
                row.append(&lbl);
                row.append(&del);
                box_t.append(&row);
            }
        }));
    }
    invoke_slot(&rebuild_todos);

    let rebuild_shelf: CallbackSlot = new_callback_slot();
    {
        let ctx_s = ctx.clone();
        let box_s = shelf_list_box.clone();
        let self_s = rebuild_shelf.clone();

        *rebuild_shelf.borrow_mut() = Some(Box::new(move || {
            while let Some(child) = box_s.first_child() {
                box_s.remove(&child);
            }
            let files = {
                let mut st = ctx_s.borrow_mut();
                let exp_h = st.compute_tab_expanded_h();
                st.expanded_target_h = exp_h;
                if st.expanded {
                    st.target_h = exp_h;
                }
                st.app_state.shelf_files.clone()
            };
            if files.is_empty() {
                let lbl =
                    gtk4::Label::new(Some("Shelf kosong. Tarik file ke sini atau klik 'Pilih'."));
                lbl.add_css_class("sub-label");
                lbl.set_margin_top(6);
                lbl.set_margin_bottom(4);
                box_s.append(&lbl);
                return;
            }
            for f_item in files {
                let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
                row.add_css_class("list-row-card");
                let f_icon = gtk4::Image::from_icon_name("text-x-generic-symbolic");
                f_icon.set_pixel_size(14);
                f_icon.add_css_class("pill-icon");

                let name_l = gtk4::Label::new(Some(&f_item.name));
                name_l.add_css_class("pill-primary-text");
                name_l.set_hexpand(true);
                name_l.set_halign(gtk4::Align::Start);
                name_l.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);

                let sz_l = gtk4::Label::new(Some(&f_item.size_label));
                sz_l.add_css_class("muted-label");

                let open_b = make_icon_pill_btn("document-open-symbolic", "Buka", false);
                let copy_b = make_icon_pill_btn("edit-copy-symbolic", "Salin", false);
                let rm_b = make_circle_icon_btn("user-trash-symbolic", 12, None);

                let p1 = f_item.path.clone();
                open_b.connect_clicked(move |_| {
                    #[cfg(target_os = "linux")]
                    let _ = Command::new("xdg-open").arg(&p1).spawn();
                    #[cfg(target_os = "macos")]
                    let _ = Command::new("open").arg(&p1).spawn();
                    #[cfg(target_os = "windows")]
                    let _ = Command::new("explorer").arg(&p1).spawn();
                });

                let p2 = f_item.path.clone();
                copy_b.connect_clicked(move |_| {
                    if let Some(disp) = gdk::Display::default() {
                        disp.clipboard().set_text(&p2);
                    }
                });

                let p3 = f_item.path.clone();
                let ctx_r = ctx_s.clone();
                let reb_r = self_s.clone();
                rm_b.connect_clicked(move |_| {
                    {
                        let mut st = ctx_r.borrow_mut();
                        st.app_state.shelf_files.retain(|x| x.path != p3);
                        st.app_state.save();
                    }
                    invoke_slot(&reb_r);
                });

                row.append(&f_icon);
                row.append(&name_l);
                row.append(&sz_l);
                row.append(&open_b);
                row.append(&copy_b);
                row.append(&rm_b);
                box_s.append(&row);
            }
        }));
    }
    invoke_slot(&rebuild_shelf);

    let add_file_to_shelf: Rc<dyn Fn(PathBuf)> = {
        let ctx_f = ctx.clone();
        let reb_f = rebuild_shelf.clone();
        let hub_f = notif_hub.clone();
        Rc::new(move |path: PathBuf| {
            let p_str = path.to_string_lossy().to_string();
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| p_str.clone());
            let sz = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            let size_label = format_file_size(sz);
            let mut added = false;
            {
                let mut st = ctx_f.borrow_mut();
                if !st.app_state.shelf_files.iter().any(|x| x.path == p_str) {
                    st.app_state.shelf_files.insert(
                        0,
                        ShelfFile {
                            path: p_str,
                            name: name.clone(),
                            size_label: size_label.clone(),
                        },
                    );
                    st.app_state.save();
                    added = true;
                }
            }
            if added {
                hub_f.push_notification(
                    "File Shelf",
                    "folder-download-symbolic",
                    "File Disimpan ke Shelf",
                    &format!("{} ({})", name, size_label),
                    0,
                    "shelf",
                );
            }
            invoke_slot(&reb_f);
        })
    };

    // =========================================================================
    // HOVER & INTERACTION CONTROLLERS (Including Interactive Volume HUD Slider!)
    // =========================================================================
    {
        let motion = gtk4::EventControllerMotion::new();

        let ctx_enter = ctx.clone();
        let set_exp_in = set_expanded_state.clone();
        motion.connect_enter(move |_, _, _| {
            let should_expand = {
                let mut st = ctx_enter.borrow_mut();
                st.is_hovered = true;
                // Keep Notch in compact Volume HUD mode while volume bar is active so user can drag it!
                st.app_state.hover_expand && st.vol_osd_remaining <= 0.0
            };
            if should_expand {
                set_exp_in(true);
            }
        });

        let ctx_move = ctx.clone();
        motion.connect_motion(move |_, _, _| {
            ctx_move.borrow_mut().is_hovered = true;
        });

        let ctx_leave = ctx.clone();
        let set_exp_out = set_expanded_state.clone();
        motion.connect_leave(move |_| {
            ctx_leave.borrow_mut().is_hovered = false;
            let ctx_l = ctx_leave.clone();
            let set_l = set_exp_out.clone();
            glib::timeout_add_local_once(Duration::from_millis(360), move || {
                let (hovered, pinned, seeking) = {
                    let st = ctx_l.borrow();
                    (st.is_hovered, st.app_state.pinned, st.media_seek_dragging)
                };
                if !hovered && !pinned && !seeking {
                    set_l(false);
                }
            });
        });
        island_overlay.add_controller(motion);
    }

    {
        let drag = gtk4::GestureDrag::new();
        let drag_start_x = Rc::new(RefCell::new(0.0f64));
        let drag_mode = Rc::new(RefCell::new(0u8)); // 0 = normal click, 1 = slider drag, 2 = mute icon click

        let apply_slider_at_x = {
            let ctx_sl = ctx.clone();
            let bg_sl = bg_canvas.clone();

            Rc::new(move |x_in_hit: f64, force_set: bool| {
                let (pct, changed) = {
                    let mut st = ctx_sl.borrow_mut();
                    let island_x0 = (EXPANDED_W - st.cur_w) * 0.5;
                    let pad_x = if st.app_state.island_style == "notch" {
                        22.0
                    } else {
                        16.0
                    };
                    let inner_left = island_x0 + pad_x;
                    let inner_right = island_x0 + st.cur_w - pad_x;
                    let slider_left = inner_left + 42.0;
                    let slider_right = (inner_right - 48.0).max(slider_left + 60.0);
                    let slider_w = (slider_right - slider_left).max(1.0);

                    let frac = ((x_in_hit - slider_left) / slider_w).clamp(0.0, 1.0);
                    let pct = (frac * 100.0).round() as u32;
                    let changed = st.cur_volume_pct != pct || st.cur_volume_muted;
                    st.cur_volume_pct = pct;
                    st.disp_volume_pct = frac * 100.0;
                    st.cur_volume_muted = false;
                    st.prev_volume_pct = Some(pct);
                    st.prev_volume_muted = Some(false);
                    st.vol_osd_remaining = 2.2;
                    st.last_vol_set_at = Some(Instant::now());
                    (pct, changed)
                };

                if changed || force_set {
                    set_system_volume(pct);
                }

                bg_sl.queue_draw();
            })
        };

        {
            let ctx_b = ctx.clone();
            let sx = drag_start_x.clone();
            let dm = drag_mode.clone();
            let apply_sl = apply_slider_at_x.clone();
            drag.connect_drag_begin(move |_, x, _y| {
                *sx.borrow_mut() = x;
                let mode = {
                    let mut st = ctx_b.borrow_mut();
                    if !st.expanded && st.vol_osd_remaining > 0.0 {
                        let island_x0 = (EXPANDED_W - st.cur_w) * 0.5;
                        let pad_x = if st.app_state.island_style == "notch" {
                            22.0
                        } else {
                            16.0
                        };
                        let inner_left = island_x0 + pad_x;
                        let inner_right = island_x0 + st.cur_w - pad_x;
                        let slider_left = inner_left + 42.0;
                        let slider_right = (inner_right - 48.0).max(slider_left + 60.0);

                        if x >= slider_left - 10.0 && x <= slider_right + 12.0 {
                            st.vol_osd_dragging = true;
                            st.vol_osd_remaining = 2.2;
                            1u8
                        } else if x >= inner_left - 6.0 && x <= inner_left + 34.0 {
                            st.vol_osd_remaining = 2.2;
                            2u8
                        } else {
                            0u8
                        }
                    } else {
                        0u8
                    }
                };
                *dm.borrow_mut() = mode;
                if mode == 1 {
                    apply_sl(x, false);
                }
            });
        }

        {
            let sx = drag_start_x.clone();
            let dm = drag_mode.clone();
            let apply_sl = apply_slider_at_x.clone();
            drag.connect_drag_update(move |_, off_x, _off_y| {
                if *dm.borrow() == 1 {
                    let cur_x = *sx.borrow() + off_x;
                    apply_sl(cur_x, false);
                }
            });
        }

        {
            let ctx_e = ctx.clone();
            let sx = drag_start_x;
            let dm = drag_mode;
            let apply_sl = apply_slider_at_x;
            let set_c = set_expanded_state.clone();
            let win_f = window.clone();
            let bg_e = bg_canvas.clone();
            drag.connect_drag_end(move |_, off_x, off_y| {
                let mode = *dm.borrow();
                if mode == 1 {
                    let final_x = *sx.borrow() + off_x;
                    apply_sl(final_x, true);
                    ctx_e.borrow_mut().vol_osd_dragging = false;
                } else if mode == 2 {
                    {
                        let mut st = ctx_e.borrow_mut();
                        st.cur_volume_muted = !st.cur_volume_muted;
                        st.prev_volume_muted = Some(st.cur_volume_muted);
                        st.vol_osd_remaining = 2.2;
                        st.last_vol_set_at = Some(Instant::now());
                    }
                    toggle_system_mute();
                    bg_e.queue_draw();
                } else if off_x.hypot(off_y) < 10.0 {
                    grab_keyboard_focus(&win_f);
                    let exp = ctx_e.borrow().expanded;
                    set_c(!exp);
                }
            });
        }

        compact_hit_box.add_controller(drag);

        // Mouse Scroll Wheel on Compact Notch adjusts Volume & opens Volume HUD
        let scroll_ctrl =
            gtk4::EventControllerScroll::new(gtk4::EventControllerScrollFlags::VERTICAL);
        let ctx_sc = ctx.clone();
        let bg_sc = bg_canvas.clone();
        let hit_sc = compact_hit_box.clone();
        scroll_ctrl.connect_scroll(move |_, _dx, dy| {
            let new_pct = {
                let mut st = ctx_sc.borrow_mut();
                if st.expanded {
                    return glib::Propagation::Proceed;
                }
                let step: i32 = if dy < 0.0 { 4 } else { -4 };
                let pct = (st.cur_volume_pct as i32 + step).clamp(0, 100) as u32;
                st.cur_volume_pct = pct;
                st.cur_volume_muted = false;
                st.prev_volume_pct = Some(pct);
                st.prev_volume_muted = Some(false);
                if st.vol_osd_remaining <= 0.0 {
                    st.vol_osd_anim_prog = 0.0;
                    st.vel_w += 90.0;
                    st.vel_h += 35.0;
                }
                st.vol_osd_remaining = 2.2;
                st.target_w = VOL_OSD_W;
                st.target_h = VOL_OSD_H;
                st.last_vol_set_at = Some(Instant::now());
                pct
            };
            hit_sc.set_size_request(EXPANDED_W as i32, VOL_OSD_H as i32);
            set_system_volume(new_pct);
            bg_sc.queue_draw();
            glib::Propagation::Stop
        });
        compact_hit_box.add_controller(scroll_ctrl);
    }

    {
        let focus_ctrl = gtk4::EventControllerFocus::new();
        let win_kb = window.clone();
        focus_ctrl.connect_enter(move |_| {
            grab_keyboard_focus(&win_kb);
        });
        todo_entry.add_controller(focus_ctrl);

        let prayer_focus_ctrl = gtk4::EventControllerFocus::new();
        let win_kb2 = window.clone();
        prayer_focus_ctrl.connect_enter(move |_| {
            grab_keyboard_focus(&win_kb2);
        });
        prayer_search_entry.add_controller(prayer_focus_ctrl);
    }

    {
        let all_tabs = all_seg_tabs.clone();
        for (tab_idx, (id, btn, _lbl, _canvas)) in seg_buttons.into_iter().enumerate() {
            let st_c = stack.clone();
            let all_c = all_tabs.clone();
            let id_s = id.clone();
            let win_kb = window.clone();
            let ctx_pulse = ctx.clone();
            let hub_tab = notif_hub.clone();
            let reb_tab = rebuild_notifs.clone();
            btn.connect_clicked(move |b| {
                grab_keyboard_focus(&win_kb);
                st_c.set_visible_child_name(&id_s);
                for (_, other_btn, _, _) in all_c.iter() {
                    other_btn.remove_css_class("active");
                }
                b.add_css_class("active");
                {
                    let mut st = ctx_pulse.borrow_mut();
                    st.active_tab_id = id_s.clone();
                    let exp_h = st.compute_tab_expanded_h();
                    st.expanded_target_h = exp_h;
                    if st.expanded {
                        st.target_h = exp_h;
                    }
                    st.vel_w += 95.0;
                    st.vel_h += 40.0;
                    for i in 0..6 {
                        if i == tab_idx {
                            st.tab_spring_target[i] = 1.0;
                            st.tab_spring_vel[i] += 2.2;
                        } else {
                            st.tab_spring_target[i] = 0.0;
                        }
                    }
                }
                if id_s == "notif" {
                    hub_tab.mark_all_read();
                    invoke_slot(&reb_tab);
                }
            });
        }
    }

    {
        let ctx_p = ctx.clone();
        pin_btn.connect_clicked(move |b| {
            let mut st = ctx_p.borrow_mut();
            st.app_state.pinned = !st.app_state.pinned;
            if st.app_state.pinned {
                b.add_css_class("pinned");
            } else {
                b.remove_css_class("pinned");
            }
            st.vel_w += 75.0;
            st.app_state.save();
        });

        let app_q = app.clone();
        quit_btn.connect_clicked(move |_| {
            set_system_notification_banners_hidden(false);
            app_q.quit();
        });
    }

    {
        let ctx_pp = ctx.clone();
        btn_play.connect_clicked(move |_| {
            ctx_pp.borrow_mut().vel_w += 80.0;
            thread::spawn(|| {
                let _ = send_mpris_command("PlayPause");
            });
        });

        let ctx_pv = ctx.clone();
        btn_prev.connect_clicked(move |_| {
            ctx_pv.borrow_mut().vel_w -= 65.0;
            thread::spawn(|| {
                let _ = send_mpris_command("Previous");
            });
        });

        let ctx_nx = ctx.clone();
        btn_next.connect_clicked(move |_| {
            ctx_nx.borrow_mut().vel_w += 65.0;
            thread::spawn(|| {
                let _ = send_mpris_command("Next");
            });
        });

        // Interactive Media Duration Progress Bar (Click & Drag to Seek)
        let pending_seek: Arc<Mutex<Option<(Option<String>, Option<String>, f64)>>> =
            Arc::new(Mutex::new(None));
        let seek_worker_busy = Arc::new(AtomicBool::new(false));
        let last_live_seek_at = Rc::new(RefCell::new(Instant::now() - Duration::from_secs(5)));

        let dispatch_seek = {
            let pending = pending_seek.clone();
            let busy = seek_worker_busy.clone();
            Rc::new(
                move |bus: Option<String>, track_id: Option<String>, target_sec: f64| {
                    if let Ok(mut guard) = pending.lock() {
                        *guard = Some((bus, track_id, target_sec));
                    }
                    if busy
                        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                        .is_ok()
                    {
                        let p_bg = pending.clone();
                        let b_bg = busy.clone();
                        thread::spawn(move || loop {
                            let next_job = p_bg.lock().ok().and_then(|mut g| g.take());
                            if let Some((bus_opt, tid_opt, sec)) = next_job {
                                let _ = seek_media_to(
                                    bus_opt.as_deref(),
                                    tid_opt.as_deref(),
                                    sec,
                                );
                            } else {
                                b_bg.store(false, Ordering::SeqCst);
                                if p_bg.lock().map(|g| g.is_some()).unwrap_or(false)
                                    && b_bg
                                        .compare_exchange(
                                            false,
                                            true,
                                            Ordering::SeqCst,
                                            Ordering::SeqCst,
                                        )
                                        .is_ok()
                                {
                                    continue;
                                }
                                break;
                            }
                        });
                    }
                },
            )
        };

        let apply_media_seek_at_x = {
            let ctx_ms = ctx.clone();
            let canvas_ms = media_lyric_canvas.clone();
            let bg_ms = bg_canvas.clone();
            let l_mgr_ms = lyrics_mgr.clone();
            let dispatch_ms = dispatch_seek.clone();
            let last_live = last_live_seek_at.clone();

            Rc::new(move |x_pos: f64, is_final: bool| {
                let wf = (canvas_ms.width() as f64).max(80.0);
                let track_x = 2.0;
                let track_w = (wf - 4.0).max(40.0);
                let frac = ((x_pos - track_x) / track_w).clamp(0.0, 1.0);

                let maybe_seek = {
                    let mut st = ctx_ms.borrow_mut();
                    let len_sec = st.cached_media.length_secs as f64;
                    if !st.cached_media.has_player || len_sec <= 0.5 {
                        None
                    } else {
                        let target_sec = (frac * len_sec).clamp(0.0, len_sec);
                        st.interp_pos_sec = target_sec;
                        st.last_media_seek_at = Some(Instant::now());

                        let prov_pref = st.app_state.lyric_provider.clone();
                        let offset_ms = st.app_state.lyric_offset_ms;
                        let snap = l_mgr_ms.get_lyric_snapshot(
                            &st.cached_media.title,
                            &st.cached_media.artist,
                            st.interp_pos_sec,
                            len_sec,
                            &prov_pref,
                            offset_ms,
                        );
                        st.lyric_line_prog = snap.line_progress;
                        st.lyric_is_word_synced = snap.is_word_synced;
                        st.active_word_idx = snap.active_word_idx;
                        st.word_prog = snap.word_progress;
                        st.lyric_accent_rgb = snap.accent_rgb;
                        st.next_lyric = snap.next_line_text;
                        if snap.current_text != st.cur_lyric {
                            st.prev_prev_lyric = st.prev_lyric.clone();
                            st.prev_lyric = snap.prev_line_text;
                            st.cur_lyric = snap.current_text;
                            st.lyric_words = snap.words;
                            st.last_word_idx = 0;
                            st.lyric_anim_prog = 1.0;
                        }

                        Some((
                            st.cached_media.active_bus.clone(),
                            st.cached_media.track_id.clone(),
                            target_sec,
                        ))
                    }
                };

                if let Some((bus, tid, target_sec)) = maybe_seek {
                    let should_send = if is_final {
                        true
                    } else {
                        let mut last = last_live.borrow_mut();
                        if last.elapsed() >= Duration::from_millis(140) {
                            *last = Instant::now();
                            true
                        } else {
                            false
                        }
                    };
                    if should_send {
                        dispatch_ms(bus, tid, target_sec);
                    }
                    canvas_ms.queue_draw();
                    bg_ms.queue_draw();
                }
            })
        };

        let media_motion = gtk4::EventControllerMotion::new();
        {
            let ctx_mm = ctx.clone();
            let canvas_mm = media_lyric_canvas.clone();
            media_motion.connect_motion(move |_, _x, y| {
                let can_seek = {
                    let st = ctx_mm.borrow();
                    st.cached_media.has_player && st.cached_media.length_secs > 0
                };
                if can_seek && (y <= 28.0 || ctx_mm.borrow().media_seek_dragging) {
                    canvas_mm.set_cursor_from_name(Some("pointer"));
                } else {
                    canvas_mm.set_cursor_from_name(None);
                }
            });
            let canvas_ml = media_lyric_canvas.clone();
            media_motion.connect_leave(move |_| {
                canvas_ml.set_cursor_from_name(None);
            });
        }
        media_lyric_canvas.add_controller(media_motion);

        let media_drag = gtk4::GestureDrag::new();
        media_drag.set_propagation_phase(gtk4::PropagationPhase::Capture);
        let m_drag_start_x = Rc::new(RefCell::new(0.0f64));
        let m_drag_active = Rc::new(RefCell::new(false));

        {
            let ctx_db = ctx.clone();
            let sx = m_drag_start_x.clone();
            let act = m_drag_active.clone();
            let apply_s = apply_media_seek_at_x.clone();
            let last_live = last_live_seek_at.clone();
            media_drag.connect_drag_begin(move |gesture, x, y| {
                let can_start = {
                    let mut st = ctx_db.borrow_mut();
                    if y >= -4.0
                        && y <= 30.0
                        && st.cached_media.has_player
                        && st.cached_media.length_secs > 0
                    {
                        st.media_seek_dragging = true;
                        st.last_media_seek_at = Some(Instant::now());
                        true
                    } else {
                        false
                    }
                };
                *act.borrow_mut() = can_start;
                if can_start {
                    *sx.borrow_mut() = x;
                    *last_live.borrow_mut() = Instant::now();
                    gesture.set_state(gtk4::EventSequenceState::Claimed);
                    apply_s(x, false);
                }
            });
        }

        {
            let sx = m_drag_start_x.clone();
            let act = m_drag_active.clone();
            let apply_s = apply_media_seek_at_x.clone();
            media_drag.connect_drag_update(move |_, off_x, _off_y| {
                if *act.borrow() {
                    let cur_x = *sx.borrow() + off_x;
                    apply_s(cur_x, false);
                }
            });
        }

        {
            let ctx_de = ctx.clone();
            let sx = m_drag_start_x;
            let act = m_drag_active;
            let apply_s = apply_media_seek_at_x;
            let canvas_de = media_lyric_canvas.clone();
            media_drag.connect_drag_end(move |_, off_x, _off_y| {
                if *act.borrow() {
                    *act.borrow_mut() = false;
                    {
                        let mut st = ctx_de.borrow_mut();
                        st.media_seek_dragging = false;
                        st.last_media_seek_at = Some(Instant::now());
                    }
                    let final_x = *sx.borrow() + off_x;
                    apply_s(final_x, true);
                    canvas_de.queue_draw();
                }
            });
        }

        media_lyric_canvas.add_controller(media_drag);
    }

    // Pomodoro & Water Controls
    {
        let ctx_ps = ctx.clone();
        btn_pomo_start.connect_clicked(move |b| {
            let mut st = ctx_ps.borrow_mut();
            st.pomo_running = !st.pomo_running;
            st.vel_w += 75.0;
            let icon = if st.pomo_running {
                "media-playback-pause-symbolic"
            } else {
                "media-playback-start-symbolic"
            };
            let txt = if st.pomo_running { "Jeda" } else { "Lanjut" };
            set_icon_pill_content(b, icon, txt);
        });

        let ctx_pr = ctx.clone();
        let dig_c = pomo_digits.clone();
        let bar_c = pomo_bar.clone();
        let start_c = btn_pomo_start.clone();
        btn_pomo_reset.connect_clicked(move |_| {
            let mut st = ctx_pr.borrow_mut();
            st.pomo_running = false;
            st.pomo_remaining_secs = st.pomo_total_secs;
            dig_c.set_label(&format!(
                "{:02}:{:02}",
                st.pomo_remaining_secs / 60,
                st.pomo_remaining_secs % 60
            ));
            bar_c.set_fraction(1.0);
            set_icon_pill_content(&start_c, "media-playback-start-symbolic", "Mulai");
        });

        let ctx_pm = ctx.clone();
        let tag_c = pomo_tag.clone();
        let dig_m = pomo_digits.clone();
        let bar_m = pomo_bar.clone();
        let start_m = btn_pomo_start.clone();
        btn_pomo_mode.connect_clicked(move |b| {
            let mut st = ctx_pm.borrow_mut();
            st.pomo_running = false;
            st.pomo_break_mode = !st.pomo_break_mode;
            if st.pomo_break_mode {
                let b_mins = st.app_state.pomo_break_mins.clamp(1, 60);
                let f_mins = st.app_state.pomo_focus_mins.clamp(1, 180);
                st.pomo_total_secs = b_mins * 60;
                st.pomo_remaining_secs = b_mins * 60;
                tag_c.set_label("ISTIRAHAT PENDEK");
                b.set_label(&format!("Fokus {}m", f_mins));
                dig_m.set_label(&format!("{:02}:00", b_mins));
            } else {
                let f_mins = st.app_state.pomo_focus_mins.clamp(1, 180);
                let b_mins = st.app_state.pomo_break_mins.clamp(1, 60);
                st.pomo_total_secs = f_mins * 60;
                st.pomo_remaining_secs = f_mins * 60;
                tag_c.set_label("POMODORO FOKUS");
                b.set_label(&format!("Rehat {}m", b_mins));
                dig_m.set_label(&format!("{:02}:00", f_mins));
            }
            bar_m.set_fraction(1.0);
            set_icon_pill_content(&start_m, "media-playback-start-symbolic", "Mulai");
        });

        let adjust_pomo_mins: Rc<dyn Fn(i32)> = {
            let ctx_adj = ctx.clone();
            let dig_adj = pomo_digits.clone();
            let bar_adj = pomo_bar.clone();
            let mode_adj = btn_pomo_mode.clone();
            let start_adj = btn_pomo_start.clone();
            let pf_sc_adj = ext_pomo_focus_scale.clone();
            let pb_sc_adj = ext_pomo_break_scale.clone();
            Rc::new(move |delta: i32| {
                let (new_mins, is_break, other_mins) = {
                    let mut st = ctx_adj.borrow_mut();
                    st.pomo_running = false;
                    if st.pomo_break_mode {
                        let cur = st.app_state.pomo_break_mins as i32;
                        let next = if cur == 1 && delta == 5 {
                            5
                        } else {
                            (cur + delta).clamp(1, 60)
                        } as u32;
                        st.app_state.pomo_break_mins = next;
                        st.pomo_total_secs = next * 60;
                        st.pomo_remaining_secs = next * 60;
                        st.vel_w += 55.0;
                        st.app_state.save();
                        (next, true, st.app_state.pomo_focus_mins)
                    } else {
                        let cur = st.app_state.pomo_focus_mins as i32;
                        let next = if cur == 1 && delta == 5 {
                            5
                        } else {
                            (cur + delta).clamp(1, 180)
                        } as u32;
                        st.app_state.pomo_focus_mins = next;
                        st.pomo_total_secs = next * 60;
                        st.pomo_remaining_secs = next * 60;
                        st.vel_w += 55.0;
                        st.app_state.save();
                        (next, false, st.app_state.pomo_break_mins)
                    }
                };
                dig_adj.set_label(&format!("{:02}:00", new_mins));
                bar_adj.set_fraction(1.0);
                set_icon_pill_content(&start_adj, "media-playback-start-symbolic", "Mulai");
                if is_break {
                    mode_adj.set_label(&format!("Fokus {}m", other_mins));
                    pb_sc_adj.set_value(new_mins.clamp(1, 60) as f64);
                } else {
                    mode_adj.set_label(&format!("Rehat {}m", other_mins));
                    pf_sc_adj.set_value(new_mins.clamp(1, 120) as f64);
                }
            })
        };

        let adj_m5 = adjust_pomo_mins.clone();
        btn_pomo_sub5.connect_clicked(move |_| adj_m5(-5));
        let adj_m1 = adjust_pomo_mins.clone();
        btn_pomo_sub1.connect_clicked(move |_| adj_m1(-1));
        let adj_p1 = adjust_pomo_mins.clone();
        btn_pomo_add1.connect_clicked(move |_| adj_p1(1));
        let adj_p5 = adjust_pomo_mins;
        btn_pomo_add5.connect_clicked(move |_| adj_p5(5));

        let sync_alarm_ui: Rc<dyn Fn()> = {
            let ctx_al = ctx.clone();
            let dig_al = alarm_digits.clone();
            let stat_al = alarm_status_lbl.clone();
            let bar_al = alarm_bar.clone();
            let btn_al = btn_alarm_toggle.clone();
            Rc::new(move || {
                let st = ctx_al.borrow();
                let ah = st.app_state.alarm_hour.min(23);
                let am = st.app_state.alarm_min.min(59);
                dig_al.set_label(&format!("{:02}:{:02}", ah, am));

                if st.alarm_ringing {
                    stat_al.set_label("BERBUNYI! ⏰");
                    bar_al.set_fraction(1.0);
                    set_icon_pill_content(&btn_al, "media-playback-stop-symbolic", "Hentikan");
                } else if st.app_state.alarm_enabled {
                    let now = Local::now();
                    let now_secs =
                        (now.hour() * 3600 + now.minute() * 60 + now.second()) as i64;
                    let target_secs = (ah * 3600 + am * 60) as i64;
                    let mut diff = target_secs - now_secs;
                    if diff <= 0 {
                        diff += 86_400;
                    }
                    let rem_txt = if diff >= 3600 {
                        format!("-{}j {:02}m", diff / 3600, (diff % 3600) / 60)
                    } else {
                        format!("-{:02}m {:02}d", diff / 60, diff % 60)
                    };
                    stat_al.set_label(&rem_txt);
                    let frac = (1.0 - (diff as f64 / 3600.0).clamp(0.0, 1.0)).max(0.12);
                    bar_al.set_fraction(frac);
                    set_icon_pill_content(&btn_al, "media-playback-stop-symbolic", "Matikan");
                } else {
                    stat_al.set_label("NONAKTIF");
                    bar_al.set_fraction(0.0);
                    set_icon_pill_content(&btn_al, "alarm-symbolic", "Aktifkan");
                }
            })
        };
        sync_alarm_ui();

        let adjust_alarm_time: Rc<dyn Fn(i32, i32)> = {
            let ctx_at = ctx.clone();
            let sync_at = sync_alarm_ui.clone();
            Rc::new(move |delta_h: i32, delta_m: i32| {
                {
                    let mut st = ctx_at.borrow_mut();
                    let cur_h = st.app_state.alarm_hour.min(23) as i32;
                    let cur_m = st.app_state.alarm_min.min(59) as i32;
                    let total_mins = (cur_h * 60 + cur_m + delta_h * 60 + delta_m).rem_euclid(1440);
                    st.app_state.alarm_hour = (total_mins / 60) as u32;
                    st.app_state.alarm_min = (total_mins % 60) as u32;
                    st.alarm_ringing = false;
                    st.alarm_ring_remaining_secs = 0;
                    st.last_triggered_alarm_key.clear();
                    st.vel_w += 55.0;
                    st.app_state.save();
                }
                stop_alarm_chime();
                sync_at();
            })
        };

        let al_add_h = adjust_alarm_time.clone();
        btn_alarm_add_h.connect_clicked(move |_| al_add_h(1, 0));
        let al_sub_h = adjust_alarm_time.clone();
        btn_alarm_sub_h.connect_clicked(move |_| al_sub_h(-1, 0));
        let al_sub_m5 = adjust_alarm_time.clone();
        btn_alarm_sub_m5.connect_clicked(move |_| al_sub_m5(0, -5));
        let al_add_m1 = adjust_alarm_time.clone();
        btn_alarm_add_m1.connect_clicked(move |_| al_add_m1(0, 1));
        let al_add_m5 = adjust_alarm_time;
        btn_alarm_add_m5.connect_clicked(move |_| al_add_m5(0, 5));

        let ctx_atog = ctx.clone();
        let sync_atog = sync_alarm_ui.clone();
        btn_alarm_toggle.connect_clicked(move |_| {
            let was_ringing = {
                let mut st = ctx_atog.borrow_mut();
                st.vel_w += 75.0;
                if st.alarm_ringing {
                    st.alarm_ringing = false;
                    st.alarm_ring_remaining_secs = 0;
                    st.app_state.alarm_enabled = false;
                    st.app_state.save();
                    true
                } else {
                    st.app_state.alarm_enabled = !st.app_state.alarm_enabled;
                    st.last_triggered_alarm_key.clear();
                    st.app_state.save();
                    false
                }
            };
            if was_ringing {
                stop_alarm_chime();
            }
            sync_atog();
        });

        let ctx_asnz = ctx.clone();
        let sync_asnz = sync_alarm_ui;
        btn_alarm_snooze.connect_clicked(move |_| {
            {
                let mut st = ctx_asnz.borrow_mut();
                let now = Local::now();
                let total_mins = (now.hour() as i32 * 60 + now.minute() as i32 + 5).rem_euclid(1440);
                st.app_state.alarm_hour = (total_mins / 60) as u32;
                st.app_state.alarm_min = (total_mins % 60) as u32;
                st.app_state.alarm_enabled = true;
                st.alarm_ringing = false;
                st.alarm_ring_remaining_secs = 0;
                st.last_triggered_alarm_key.clear();
                st.vel_w += 70.0;
                st.app_state.save();
            }
            stop_alarm_chime();
            sync_asnz();
        });
    }

    // To-Do Add Handler
    {
        let add_todo: Rc<dyn Fn()> = {
            let ctx_ta = ctx.clone();
            let ent_c = todo_entry.clone();
            let reb_t = rebuild_todos.clone();
            Rc::new(move || {
                let text = ent_c.text().trim().to_string();
                if text.is_empty() {
                    return;
                }
                {
                    let mut st = ctx_ta.borrow_mut();
                    let next_id = st.app_state.todos.iter().map(|t| t.id).max().unwrap_or(0) + 1;
                    st.app_state.todos.push(TodoItem {
                        id: next_id,
                        text,
                        completed: false,
                    });
                    st.app_state.save();
                }
                ent_c.set_text("");
                invoke_slot(&reb_t);
            })
        };

        let a1 = add_todo.clone();
        btn_todo_add.connect_clicked(move |_| a1());
        let a2 = add_todo;
        todo_entry.connect_activate(move |_| a2());
    }

    // Shelf Drag & Drop + File Picker
    {
        let drop_target =
            gtk4::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
        let add_d = add_file_to_shelf.clone();
        drop_target.connect_drop(move |_, value, _, _| {
            if let Ok(flist) = value.get::<gdk::FileList>() {
                for gf in flist.files() {
                    if let Some(p) = gf.path() {
                        add_d(p);
                    }
                }
                return true;
            }
            false
        });
        shelf_page.add_controller(drop_target);

        let win_p = window.clone();
        let add_p = add_file_to_shelf;
        btn_pick_file.connect_clicked(move |_| {
            let dialog = gtk4::FileDialog::builder()
                .title("Pilih File ke Shelf")
                .modal(true)
                .build();
            let cb = add_p.clone();
            dialog.open_multiple(Some(&win_p), None::<&gtk4::gio::Cancellable>, move |res| {
                if let Ok(list) = res {
                    for i in 0..list.n_items() {
                        if let Some(obj) = list.item(i) {
                            if let Ok(gf) = obj.downcast::<gtk4::gio::File>() {
                                if let Some(p) = gf.path() {
                                    cb(p);
                                }
                            }
                        }
                    }
                }
            });
        });

        let ctx_cs = ctx.clone();
        let reb_cs = rebuild_shelf.clone();
        btn_clear_shelf.connect_clicked(move |_| {
            {
                let mut st = ctx_cs.borrow_mut();
                st.app_state.shelf_files.clear();
                st.app_state.save();
            }
            invoke_slot(&reb_cs);
        });
    }

    let refresh_1s: CallbackSlot = new_callback_slot();
    let dd_updating = Rc::new(RefCell::new(false));
    {
        let ctx_city = ctx.clone();
        let ref_c = refresh_1s.clone();
        let opts_c = active_region_options.clone();
        let upd_c = dd_updating.clone();
        let ext_badge_c = ext_prayer_badge_lbl.clone();
        city_dropdown.connect_selected_notify(move |dd| {
            if *upd_c.borrow() {
                return;
            }
            let idx = dd.selected() as usize;
            let maybe_reg = opts_c.borrow().get(idx).cloned();
            if let Some(reg) = maybe_reg {
                ext_badge_c.set_label(&format!("{} ({})", reg.name, reg.tz_label));
                let mut st = ctx_city.borrow_mut();
                st.app_state.selected_city = reg.name;
                st.app_state.prayer_lat = Some(reg.lat);
                st.app_state.prayer_lon = Some(reg.lon);
                st.app_state.prayer_tz_offset = Some(reg.tz_offset);
                st.app_state.prayer_tz_label = Some(reg.tz_label);
                st.vel_w += 65.0;
                st.app_state.save();
            }
            invoke_slot(&ref_c);
        });

        let update_region_dropdown: Rc<dyn Fn(Vec<PrayerRegion>, bool)> = {
            let opts_u = active_region_options.clone();
            let sl_u = region_str_list.clone();
            let dd_u = city_dropdown.clone();
            let upd_u = dd_updating.clone();
            let ctx_u = ctx.clone();
            let ref_u = refresh_1s.clone();
            let ext_badge_u = ext_prayer_badge_lbl.clone();
            Rc::new(move |new_list: Vec<PrayerRegion>, auto_apply_first: bool| {
                if new_list.is_empty() {
                    return;
                }
                *upd_u.borrow_mut() = true;
                let labels: Vec<String> = new_list
                    .iter()
                    .map(|r| format!("{} ({})", r.name, r.tz_label))
                    .collect();
                let refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
                let old_len = sl_u.n_items();
                sl_u.splice(0, old_len, &refs);
                *opts_u.borrow_mut() = new_list.clone();
                dd_u.set_selected(0);
                *upd_u.borrow_mut() = false;

                if auto_apply_first {
                    if let Some(first) = new_list.into_iter().next() {
                        ext_badge_u.set_label(&format!("{} ({})", first.name, first.tz_label));
                        {
                            let mut st = ctx_u.borrow_mut();
                            st.app_state.selected_city = first.name;
                            st.app_state.prayer_lat = Some(first.lat);
                            st.app_state.prayer_lon = Some(first.lon);
                            st.app_state.prayer_tz_offset = Some(first.tz_offset);
                            st.app_state.prayer_tz_label = Some(first.tz_label);
                            st.vel_w += 75.0;
                            st.app_state.save();
                        }
                        invoke_slot(&ref_u);
                    }
                }
            })
        };

        let upd_live = update_region_dropdown.clone();
        prayer_search_entry.connect_changed(move |ent| {
            let q = ent.text().trim().to_string();
            let local = search_regions_local(&q);
            if !local.is_empty() {
                upd_live(local, false);
            }
        });

        let trigger_region_search: Rc<dyn Fn()> = {
            let ent_s = prayer_search_entry.clone();
            let upd_s = update_region_dropdown.clone();
            let title_s = prayer_title_lbl.clone();
            Rc::new(move || {
                let q = ent_s.text().trim().to_string();
                if q.is_empty() {
                    let all = search_regions_local("");
                    upd_s(all, false);
                    return;
                }
                let local = search_regions_local(&q);
                if !local.is_empty() {
                    upd_s(local.clone(), true);
                } else {
                    title_s.set_label("JADWAL SHOLAT • MENCARI KOTA/KECAMATAN...");
                }

                let slot: Arc<Mutex<Option<Vec<PrayerRegion>>>> = Arc::new(Mutex::new(None));
                let slot_bg = slot.clone();
                let q_bg = q.clone();
                thread::spawn(move || {
                    let online = search_regions_online(&q_bg);
                    if let Ok(mut g) = slot_bg.lock() {
                        *g = Some(online);
                    }
                });

                let upd_cb = upd_s.clone();
                let local_fb = local;
                glib::timeout_add_local(Duration::from_millis(80), move || {
                    let maybe_done = slot.lock().ok().and_then(|mut g| g.take());
                    if let Some(online_res) = maybe_done {
                        let mut merged = online_res;
                        for loc in &local_fb {
                            if !merged
                                .iter()
                                .any(|r| r.name.eq_ignore_ascii_case(&loc.name))
                            {
                                merged.push(loc.clone());
                            }
                        }
                        if !merged.is_empty() {
                            upd_cb(merged, true);
                        }
                        glib::ControlFlow::Break
                    } else {
                        glib::ControlFlow::Continue
                    }
                });
            })
        };

        let tr1 = trigger_region_search.clone();
        btn_prayer_search.connect_clicked(move |_| tr1());
        let tr2 = trigger_region_search;
        prayer_search_entry.connect_activate(move |_| tr2());
    }

    // =========================================================================
    // 1-SECOND PERIODIC UI REFRESH (Zero Subprocesses on UI Thread!)
    // =========================================================================
    {
        let ctx_u = ctx.clone();
        let bg_mon = bg_monitor.clone();
        let hub_1s = notif_hub.clone();
        let bg_c = bg_canvas.clone();
        let m_vis_1s = media_vis_canvas.clone();
        let m_lyr_1s = media_lyric_canvas.clone();
        let m_src = media_source_lbl.clone();
        let m_title = media_title_lbl.clone();
        let m_artist = media_artist_lbl.clone();
        let m_play_img = play_img.clone();
        let a_stack = album_stack.clone();
        let a_pic = album_picture.clone();
        let pm_dig = pomo_digits.clone();
        let pm_bar = pomo_bar.clone();
        let pm_sess = pomo_sess_lbl.clone();
        let pm_mode_btn = btn_pomo_mode.clone();
        let al_dig = alarm_digits.clone();
        let al_stat = alarm_status_lbl.clone();
        let al_bar = alarm_bar.clone();
        let al_btn = btn_alarm_toggle.clone();
        let sh_title = prayer_title_lbl.clone();
        let sh_next = prayer_next_lbl.clone();
        let sh_pills = sholat_pills.clone();

        *refresh_1s.borrow_mut() = Some(Box::new(move || {
            let now = Local::now();
            let now_hm = now.format("%H:%M").to_string();
            let today_ymd = now.format("%Y-%m-%d").to_string();

            let (media, metrics) = {
                let guard = bg_mon.lock().unwrap();
                (guard.media.clone(), guard.metrics.clone())
            };

            let mut pomo_finished: Option<(bool, u32, u32)> = None;
            let mut alarm_just_triggered: Option<(u32, u32)> = None;
            let mut play_alarm_tick = false;
            let active_region = {
                let mut st = ctx_u.borrow_mut();
                if st.pomo_running {
                    if st.pomo_remaining_secs > 0 {
                        st.pomo_remaining_secs -= 1;
                    } else {
                        st.pomo_running = false;
                        if !st.pomo_break_mode {
                            st.app_state.pomo_completed_sessions += 1;
                            st.app_state.save();
                        }
                        st.pomo_remaining_secs = st.pomo_total_secs;
                        pomo_finished = Some((
                            st.pomo_break_mode,
                            st.app_state.pomo_completed_sessions,
                            st.app_state.pomo_focus_mins,
                        ));
                    }
                    let mins = st.pomo_remaining_secs / 60;
                    let secs = st.pomo_remaining_secs % 60;
                    pm_dig.set_label(&format!("{:02}:{:02}", mins, secs));
                    pm_bar.set_fraction(
                        (st.pomo_remaining_secs as f64 / st.pomo_total_secs.max(1) as f64)
                            .clamp(0.0, 1.0),
                    );
                    pm_sess.set_label(&format!("{} Sesi", st.app_state.pomo_completed_sessions));
                    st.clock_str = format!("{:02}:{:02}", mins, secs);
                } else {
                    st.clock_str = now_hm.clone();
                    let mins = st.pomo_remaining_secs / 60;
                    let secs = st.pomo_remaining_secs % 60;
                    pm_dig.set_label(&format!("{:02}:{:02}", mins, secs));
                    if st.pomo_break_mode {
                        pm_mode_btn.set_label(&format!(
                            "Fokus {}m",
                            st.app_state.pomo_focus_mins
                        ));
                    } else {
                        pm_mode_btn.set_label(&format!(
                            "Rehat {}m",
                            st.app_state.pomo_break_mins
                        ));
                    }
                }

                // Check & Update Alarm state
                let ah = st.app_state.alarm_hour.min(23);
                let am = st.app_state.alarm_min.min(59);
                al_dig.set_label(&format!("{:02}:{:02}", ah, am));

                if st.app_state.alarm_enabled
                    && !st.alarm_ringing
                    && now.hour() == ah
                    && now.minute() == am
                    && now.second() < 10
                {
                    let a_key = format!("{}-{:02}:{:02}", today_ymd, ah, am);
                    if st.last_triggered_alarm_key != a_key {
                        st.last_triggered_alarm_key = a_key;
                        st.alarm_ringing = true;
                        st.alarm_ring_remaining_secs = 24;
                        alarm_just_triggered = Some((ah, am));
                    }
                }

                if st.alarm_ringing {
                    if st.alarm_ring_remaining_secs % 3 == 0 {
                        play_alarm_tick = true;
                    }
                    if st.alarm_ring_remaining_secs > 1 {
                        st.alarm_ring_remaining_secs -= 1;
                    } else {
                        st.alarm_ringing = false;
                        st.alarm_ring_remaining_secs = 0;
                        st.app_state.alarm_enabled = false;
                        st.app_state.save();
                    }
                    al_stat.set_label("BERBUNYI! ⏰");
                    al_bar.set_fraction(1.0);
                    set_icon_pill_content(&al_btn, "media-playback-stop-symbolic", "Hentikan");
                } else if st.app_state.alarm_enabled {
                    let now_secs =
                        (now.hour() * 3600 + now.minute() * 60 + now.second()) as i64;
                    let target_secs = (ah * 3600 + am * 60) as i64;
                    let mut diff = target_secs - now_secs;
                    if diff <= 0 {
                        diff += 86_400;
                    }
                    let rem_txt = if diff >= 3600 {
                        format!("-{}j {:02}m", diff / 3600, (diff % 3600) / 60)
                    } else {
                        format!("-{:02}m {:02}d", diff / 60, diff % 60)
                    };
                    al_stat.set_label(&rem_txt);
                    let frac = (1.0 - (diff as f64 / 3600.0).clamp(0.0, 1.0)).max(0.12);
                    al_bar.set_fraction(frac);
                    set_icon_pill_content(&al_btn, "media-playback-stop-symbolic", "Matikan");
                } else {
                    al_stat.set_label("NONAKTIF");
                    al_bar.set_fraction(0.0);
                    set_icon_pill_content(&al_btn, "alarm-symbolic", "Aktifkan");
                }

                resolve_region_from_state(
                    &st.app_state.selected_city,
                    st.app_state.prayer_lat,
                    st.app_state.prayer_lon,
                    st.app_state.prayer_tz_offset,
                    st.app_state.prayer_tz_label.as_deref(),
                )
            };

            if let Some((ah, am)) = alarm_just_triggered {
                hub_1s.push_notification(
                    "Alarm Pengingat",
                    "alarm-symbolic",
                    &format!("Alarm Pukul {:02}:{:02} Berbunyi! ⏰", ah, am),
                    "Waktu alarm telah tiba. Buka tab Fokus untuk menghentikan atau menunda 5 menit.",
                    2,
                    "alarm",
                );
            }
            if play_alarm_tick {
                play_alarm_chime();
            }

            if let Some((was_break, total_sessions, focus_mins)) = pomo_finished {
                play_alarm_chime();
                if was_break {
                    hub_1s.push_notification(
                        "Pomodoro Fokus",
                        "alarm-symbolic",
                        "Waktu Istirahat Selesai!",
                        "Saatnya kembali fokus untuk sesi berikutnya.",
                        2,
                        "pomodoro",
                    );
                } else {
                    hub_1s.push_notification(
                        "Pomodoro Fokus",
                        "alarm-symbolic",
                        &format!("Sesi Fokus {} Menit Selesai! 🎉", focus_mins),
                        &format!(
                            "Total {} sesi fokus selesai hari ini. Ambil istirahat sejenak!",
                            total_sessions
                        ),
                        2,
                        "pomodoro",
                    );
                }
            }

            m_src.set_label(&if media.has_player {
                format!(
                    "{} • {}",
                    media.player_name.to_uppercase(),
                    if media.playing { "DIPUTAR" } else { "DIJEDA" }
                )
            } else {
                "TIDAK ADA MEDIA AKTIF".to_string()
            });
            m_title.set_label(&media.title);
            m_artist.set_label(&media.artist);
            m_play_img.set_icon_name(Some(if media.playing {
                "media-playback-pause-symbolic"
            } else {
                "media-playback-start-symbolic"
            }));

            {
                let mut st = ctx_u.borrow_mut();
                if st.last_art_path != media.art_path {
                    st.last_art_path = media.art_path.clone();
                    if let Some(ref path_str) = media.art_path {
                        a_pic.set_filename(Some(std::path::Path::new(path_str)));
                        a_stack.set_visible_child_name("art");
                    } else {
                        a_stack.set_visible_child_name("fallback");
                    }
                }
            }

            let sched = calculate_prayer_schedule_for_region(&active_region);
            sh_title.set_label(&format!(
                "JADWAL SHOLAT • {} ({})",
                sched.city.to_uppercase(),
                sched.tz_label
            ));
            sh_next.set_label(&format!(
                "{} pukul {} ({})",
                sched.next_prayer_name, sched.next_prayer_time, sched.countdown_str
            ));

            for (i, (box_w, name_l, time_l)) in sh_pills.iter().enumerate() {
                if let Some((n, t, _)) = sched.prayers.get(i) {
                    name_l.set_label(n);
                    time_l.set_label(t);
                    if n == &sched.next_prayer_name {
                        box_w.add_css_class("active");
                    } else {
                        box_w.remove_css_class("active");
                    }
                }
            }

            // System Event Notifications (Battery, Charger, Volume, Prayer Time)
            let mut sys_notifs_to_push: Vec<(String, String, String, String, u8, String)> =
                Vec::new();
            {
                let mut st = ctx_u.borrow_mut();
                st.next_prayer_summary =
                    format!("{} {}", sched.next_prayer_name, sched.next_prayer_time);

                let sys_ev_on = st.app_state.notif_enabled && st.app_state.system_event_notifs;

                // 1. Check Prayer Time Match
                for (p_name, p_time, _) in &sched.prayers {
                    let key = format!("{}-{}", p_name, p_time);
                    if *p_time == now_hm && st.last_notified_prayer != key {
                        st.last_notified_prayer = key;
                        if sys_ev_on {
                            sys_notifs_to_push.push((
                                "Jadwal Sholat".to_string(),
                                "weather-clear-night-symbolic".to_string(),
                                format!("Waktu {} Telah Tiba", p_name),
                                format!(
                                    "Pukul {} {} untuk wilayah {}",
                                    p_time, sched.tz_label, sched.city
                                ),
                                2,
                                "prayer".to_string(),
                            ));
                        }
                    }
                }

                // 2. Check Battery Charging & Low Battery Events
                if let Some(bat_pct) = metrics.battery_percent {
                    if let Some(prev_chg) = st.prev_battery_charging {
                        if prev_chg != metrics.battery_charging && sys_ev_on {
                            if metrics.battery_charging {
                                sys_notifs_to_push.push((
                                    "Daya & Baterai".to_string(),
                                    "battery-good-charging-symbolic".to_string(),
                                    "Pengisi Daya Terhubung".to_string(),
                                    format!("Mengisi daya baterai ({}%)", bat_pct),
                                    1,
                                    "device".to_string(),
                                ));
                            } else {
                                sys_notifs_to_push.push((
                                    "Daya & Baterai".to_string(),
                                    "battery-good-symbolic".to_string(),
                                    "Menggunakan Daya Baterai".to_string(),
                                    format!("Kapasitas baterai saat ini {}%", bat_pct),
                                    1,
                                    "device".to_string(),
                                ));
                            }
                        }
                    }
                    st.prev_battery_charging = Some(metrics.battery_charging);

                    if !metrics.battery_charging && bat_pct <= 20 {
                        if !st.prev_battery_low_warned && sys_ev_on {
                            st.prev_battery_low_warned = true;
                            sys_notifs_to_push.push((
                                "Baterai Lemah".to_string(),
                                "dialog-warning-symbolic".to_string(),
                                format!("Baterai Tersisa {}%", bat_pct),
                                "Segera hubungkan pengisi daya.".to_string(),
                                2,
                                "device".to_string(),
                            ));
                        }
                    } else if metrics.battery_charging || bat_pct >= 25 {
                        st.prev_battery_low_warned = false;
                    }
                }
            }

            for (app_n, icon_n, sum_n, body_n, urg_n, cat_n) in sys_notifs_to_push {
                hub_1s.push_notification(&app_n, &icon_n, &sum_n, &body_n, urg_n, &cat_n);
            }

            bg_c.queue_draw();
            m_vis_1s.queue_draw();
            m_lyr_1s.queue_draw();
        }));
    }

    invoke_slot(&refresh_1s);
    if std::env::var("ARWILAND_EXPAND").is_ok() {
        set_expanded_state(true);
    }
    if std::env::var("ARWILAND_SETTINGS").is_ok() {
        settings_win.present();
    }

    {
        let r_tick = refresh_1s.clone();
        glib::timeout_add_local(Duration::from_secs(1), move || {
            invoke_slot(&r_tick);
            glib::ControlFlow::Continue
        });
    }

    app.connect_shutdown(|_| {
        set_system_notification_banners_hidden(false);
    });

    gtk4::prelude::WidgetExt::realize(&window);
    {
        let y_off = ctx.borrow().app_state.y_offset;
        configure_pre_map_overlay(&window, CANVAS_WIDTH, CANVAS_HEIGHT, y_off);
        update_pill_input_region(&window, CANVAS_WIDTH, init_cw + 8, COMPACT_H as i32 + 6);
    }

    window.present();

    {
        let win_init = window.clone();
        let ctx_init = ctx;
        glib::timeout_add_local_once(Duration::from_millis(40), move || {
            let (y_off, cw) = {
                let st = ctx_init.borrow();
                (st.app_state.y_offset, st.app_state.compact_width)
            };
            setup_top_center_overlay(&win_init, CANVAS_WIDTH, CANVAS_HEIGHT, y_off);
            update_pill_input_region(&win_init, CANVAS_WIDTH, cw + 8, COMPACT_H as i32 + 6);
        });
    }
}
