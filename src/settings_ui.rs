use gtk4::glib;
use gtk4::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::lyrics::{provider_accent, LyricsManager};
use crate::notifications::{set_system_notification_banners_hidden, NotificationHub};
use crate::prayer::{
    resolve_region_from_state, search_regions_local, search_regions_online, PrayerRegion,
    INDONESIAN_CITIES,
};
use crate::runtime::{
    make_icon_pill_btn, RuntimeContext, CANVAS_HEIGHT, CANVAS_WIDTH,
};
use crate::state::set_autostart_enabled;
use crate::x11_helper::setup_top_center_overlay;

pub struct SettingsHandles {
    pub window: gtk4::Window,
    pub ext_provider_status_lbl: gtk4::Label,
    pub btn_ext_test_notif: gtk4::Button,
    pub startup_chk: gtk4::CheckButton,
    pub pomo_focus_scale: gtk4::Scale,
    pub pomo_break_scale: gtk4::Scale,
    pub ext_prayer_badge_lbl: gtk4::Label,
}

#[allow(clippy::too_many_arguments)]
pub fn build_settings_window(
    app: &gtk4::Application,
    ctx: &Rc<RefCell<RuntimeContext>>,
    lyrics_mgr: &Rc<LyricsManager>,
    notif_hub: &NotificationHub,
    main_window: &gtk4::ApplicationWindow,
    bg_canvas: &gtk4::DrawingArea,
    island_startup_btn: &gtk4::Button,
) -> SettingsHandles {
    let settings_win = gtk4::Window::builder()
        .application(app)
        .title("Pengaturan Arwiland")
        .decorated(true)
        .resizable(true)
        .default_width(540)
        .default_height(610)
        .hide_on_close(true)
        .build();
    settings_win.add_css_class("settings-window");

    let settings_scroll = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .hexpand(true)
        .vexpand(true)
        .build();

    let settings_root = gtk4::Box::new(gtk4::Orientation::Vertical, 14);
    settings_root.add_css_class("settings-root");
    settings_root.set_margin_top(16);
    settings_root.set_margin_bottom(18);
    settings_root.set_margin_start(18);
    settings_root.set_margin_end(18);

    // Header Title + Reset to Default Button
    let ext_head_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    ext_head_row.set_valign(gtk4::Align::Center);

    let ext_head_box = gtk4::Box::new(gtk4::Orientation::Vertical, 3);
    ext_head_box.set_hexpand(true);
    let ext_title = gtk4::Label::new(Some("Pengaturan Arwiland"));
    ext_title.add_css_class("heading-label");
    ext_title.set_halign(gtk4::Align::Start);
    let ext_sub = gtk4::Label::new(Some(
        "Atur pengaturanmu disini",
    ));
    ext_sub.add_css_class("sub-label");
    ext_sub.set_halign(gtk4::Align::Start);
    ext_head_box.append(&ext_title);
    ext_head_box.append(&ext_sub);

    let btn_reset_defaults = make_icon_pill_btn("view-refresh-symbolic", "Reset ke Default", false);
    btn_reset_defaults.set_valign(gtk4::Align::Center);
    btn_reset_defaults.set_tooltip_text(Some("Kembalikan semua pengaturan ke bawaan pabrik"));

    ext_head_row.append(&ext_head_box);
    ext_head_row.append(&btn_reset_defaults);
    settings_root.append(&ext_head_row);

    // --- SECTION 0A: SISTEM & STARTUP OTOMATIS (RUN ON STARTUP) ---
    let startup_card = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    startup_card.add_css_class("settings-card");

    let st_top = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    st_top.set_valign(gtk4::Align::Center);
    let st_sec_lbl = gtk4::Label::new(Some("RUN ON STARTUP"));
    st_sec_lbl.add_css_class("settings-section-title");
    st_sec_lbl.set_hexpand(true);
    st_sec_lbl.set_halign(gtk4::Align::Start);

    let init_startup = ctx.borrow().app_state.run_on_startup;
    let startup_badge_lbl = gtk4::Label::new(Some(if init_startup {
        "AKTIF SAAT BOOT"
    } else {
        "NONAKTIF"
    }));
    startup_badge_lbl.add_css_class("provider-badge");

    st_top.append(&st_sec_lbl);
    st_top.append(&startup_badge_lbl);
    startup_card.append(&st_top);

    let startup_chk = gtk4::CheckButton::with_label(
        "Jalankan Arwiland secara otomatis saat komputer dinyalakan (Run on Startup)",
    );
    startup_chk.set_active(init_startup);
    startup_card.append(&startup_chk);

    let startup_info_lbl = gtk4::Label::new(Some(
        "Mengelola file XDG Autostart (~/.config/autostart/arwiland.desktop) secara otomatis.",
    ));
    startup_info_lbl.add_css_class("muted-label");
    startup_info_lbl.set_halign(gtk4::Align::Start);
    startup_card.append(&startup_info_lbl);

    settings_root.append(&startup_card);

    // --- SECTION 0B: NOTIFIKASI DYNAMIC ISLAND (LIVE NOTCH) ---
    let notif_set_card = gtk4::Box::new(gtk4::Orientation::Vertical, 9);
    notif_set_card.add_css_class("settings-card");

    let nsec_top = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    nsec_top.set_valign(gtk4::Align::Center);
    let nsec_lbl = gtk4::Label::new(Some("NOTIFIKASI"));
    nsec_lbl.add_css_class("settings-section-title");
    nsec_lbl.set_hexpand(true);
    nsec_lbl.set_halign(gtk4::Align::Start);

    let btn_ext_test_notif = make_icon_pill_btn(
        "preferences-system-notifications-symbolic",
        "Uji Notifikasi",
        true,
    );
    nsec_top.append(&nsec_lbl);
    nsec_top.append(&btn_ext_test_notif);
    notif_set_card.append(&nsec_top);

    let notif_enabled_chk = gtk4::CheckButton::with_label(
        "Tampilkan semua notifikasi aplikasi & pesan masuk di dalam Notch / Island",
    );
    notif_enabled_chk.set_active(ctx.borrow().app_state.notif_enabled);
    notif_set_card.append(&notif_enabled_chk);

    let suppress_banners_chk = gtk4::CheckButton::with_label(
        "Sembunyikan popup banner notifikasi bawaan sistem (GNOME) agar tidak tumpang tindih",
    );
    suppress_banners_chk.set_active(ctx.borrow().app_state.suppress_system_banners);
    notif_set_card.append(&suppress_banners_chk);

    let sys_events_chk = gtk4::CheckButton::with_label(
        "Tampilkan notifikasi kejadian sistem otomatis (Baterai, Charger, Volume, Waktu Sholat)",
    );
    sys_events_chk.set_active(ctx.borrow().app_state.system_event_notifs);
    notif_set_card.append(&sys_events_chk);

    let ndur_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    ndur_row.set_valign(gtk4::Align::Center);
    let ndur_lbl = gtk4::Label::new(Some("Durasi Tampil Notif:"));
    ndur_lbl.add_css_class("sub-label");
    ndur_lbl.set_width_chars(15);
    ndur_lbl.set_halign(gtk4::Align::Start);
    let notif_dur_scale = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, 2.0, 12.0, 1.0);
    notif_dur_scale.set_value(ctx.borrow().app_state.notif_duration_secs.clamp(2, 12) as f64);
    notif_dur_scale.set_hexpand(true);
    notif_dur_scale.set_draw_value(false);
    let notif_dur_val_lbl = gtk4::Label::new(Some(&format!(
        "{} dtk",
        ctx.borrow().app_state.notif_duration_secs.clamp(2, 12)
    )));
    notif_dur_val_lbl.add_css_class("muted-label");
    ndur_row.append(&ndur_lbl);
    ndur_row.append(&notif_dur_scale);
    ndur_row.append(&notif_dur_val_lbl);
    notif_set_card.append(&ndur_row);

    settings_root.append(&notif_set_card);

    // --- SECTION 0C: POMODORO FOKUS & JADWAL SHOLAT (KOTA / KECAMATAN) ---
    let pomo_sholat_card = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    pomo_sholat_card.add_css_class("settings-card");

    let ps_top = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    ps_top.set_valign(gtk4::Align::Center);
    let ps_sec_lbl = gtk4::Label::new(Some("POMODORO FOKUS & WILAYAH JADWAL SHOLAT"));
    ps_sec_lbl.add_css_class("settings-section-title");
    ps_sec_lbl.set_hexpand(true);
    ps_sec_lbl.set_halign(gtk4::Align::Start);

    let init_reg = {
        let st = ctx.borrow();
        resolve_region_from_state(
            &st.app_state.selected_city,
            st.app_state.prayer_lat,
            st.app_state.prayer_lon,
            st.app_state.prayer_tz_offset,
            st.app_state.prayer_tz_label.as_deref(),
        )
    };
    let ext_prayer_badge_lbl = gtk4::Label::new(Some(&format!(
        "{} ({})",
        init_reg.name, init_reg.tz_label
    )));
    ext_prayer_badge_lbl.add_css_class("provider-badge");
    ps_top.append(&ps_sec_lbl);
    ps_top.append(&ext_prayer_badge_lbl);
    pomo_sholat_card.append(&ps_top);

    // Pomodoro Focus Duration Slider + Presets
    let pf_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    pf_row.set_valign(gtk4::Align::Center);
    let pf_lbl = gtk4::Label::new(Some("Durasi Fokus:"));
    pf_lbl.add_css_class("sub-label");
    pf_lbl.set_width_chars(15);
    pf_lbl.set_halign(gtk4::Align::Start);
    let pomo_focus_scale = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, 1.0, 120.0, 1.0);
    pomo_focus_scale.set_value(ctx.borrow().app_state.pomo_focus_mins.clamp(1, 120) as f64);
    pomo_focus_scale.set_hexpand(true);
    pomo_focus_scale.set_draw_value(false);
    let pomo_focus_val_lbl = gtk4::Label::new(Some(&format!(
        "{} mnt",
        ctx.borrow().app_state.pomo_focus_mins
    )));
    pomo_focus_val_lbl.add_css_class("muted-label");
    pf_row.append(&pf_lbl);
    pf_row.append(&pomo_focus_scale);
    pf_row.append(&pomo_focus_val_lbl);
    pomo_sholat_card.append(&pf_row);

    let pf_presets_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    let pf_preset_hint = gtk4::Label::new(Some("Preset Fokus:"));
    pf_preset_hint.add_css_class("sub-label");
    pf_preset_hint.set_width_chars(15);
    pf_preset_hint.set_halign(gtk4::Align::Start);
    pf_presets_row.append(&pf_preset_hint);
    let mut focus_preset_btns: Vec<(u32, gtk4::Button)> = Vec::new();
    for mins in [15u32, 25, 30, 45, 60, 90] {
        let b = gtk4::Button::with_label(&format!("{}m", mins));
        b.add_css_class("pill-btn");
        if ctx.borrow().app_state.pomo_focus_mins == mins {
            b.add_css_class("primary");
        }
        pf_presets_row.append(&b);
        focus_preset_btns.push((mins, b));
    }
    pomo_sholat_card.append(&pf_presets_row);

    // Pomodoro Break Duration Slider
    let pb_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    pb_row.set_valign(gtk4::Align::Center);
    let pb_lbl = gtk4::Label::new(Some("Durasi Istirahat:"));
    pb_lbl.add_css_class("sub-label");
    pb_lbl.set_width_chars(15);
    pb_lbl.set_halign(gtk4::Align::Start);
    let pomo_break_scale = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, 1.0, 60.0, 1.0);
    pomo_break_scale.set_value(ctx.borrow().app_state.pomo_break_mins.clamp(1, 60) as f64);
    pomo_break_scale.set_hexpand(true);
    pomo_break_scale.set_draw_value(false);
    let pomo_break_val_lbl = gtk4::Label::new(Some(&format!(
        "{} mnt",
        ctx.borrow().app_state.pomo_break_mins
    )));
    pomo_break_val_lbl.add_css_class("muted-label");
    pb_row.append(&pb_lbl);
    pb_row.append(&pomo_break_scale);
    pb_row.append(&pomo_break_val_lbl);
    pomo_sholat_card.append(&pb_row);

    // Prayer Region Search (Kota / Kecamatan)
    let sh_search_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    sh_search_row.set_valign(gtk4::Align::Center);
    let sh_search_lbl = gtk4::Label::new(Some("Kota/Kecamatan:"));
    sh_search_lbl.add_css_class("sub-label");
    sh_search_lbl.set_width_chars(15);
    sh_search_lbl.set_halign(gtk4::Align::Start);

    let ext_region_entry = gtk4::Entry::new();
    ext_region_entry.set_placeholder_text(Some(
        "Ketik kota atau kecamatan (mis: Kebayoran Baru, Coblong, Gubeng)...",
    ));
    ext_region_entry.add_css_class("island-entry");
    ext_region_entry.set_hexpand(true);

    let btn_ext_search_region =
        make_icon_pill_btn("system-search-symbolic", "Cari Wilayah", true);

    sh_search_row.append(&sh_search_lbl);
    sh_search_row.append(&ext_region_entry);
    sh_search_row.append(&btn_ext_search_region);
    pomo_sholat_card.append(&sh_search_row);

    let sh_dd_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    sh_dd_row.set_valign(gtk4::Align::Center);
    let sh_dd_lbl = gtk4::Label::new(Some("Pilih Wilayah:"));
    sh_dd_lbl.add_css_class("sub-label");
    sh_dd_lbl.set_width_chars(15);
    sh_dd_lbl.set_halign(gtk4::Align::Start);

    let ext_regions_vec: Rc<RefCell<Vec<PrayerRegion>>> = Rc::new(RefCell::new({
        let mut v = vec![init_reg.clone()];
        for c in INDONESIAN_CITIES {
            if !v.iter().any(|r| r.name.eq_ignore_ascii_case(c.name)) {
                v.push(c.to_region());
            }
        }
        v
    }));
    let ext_region_labels: Vec<String> = ext_regions_vec
        .borrow()
        .iter()
        .map(|r| format!("{} ({})", r.name, r.tz_label))
        .collect();
    let ext_region_refs: Vec<&str> = ext_region_labels.iter().map(|s| s.as_str()).collect();
    let ext_region_strlist = gtk4::StringList::new(&ext_region_refs);
    let ext_region_dd = gtk4::DropDown::new(Some(ext_region_strlist.clone()), gtk4::Expression::NONE);
    ext_region_dd.set_selected(0);
    ext_region_dd.set_hexpand(true);

    sh_dd_row.append(&sh_dd_lbl);
    sh_dd_row.append(&ext_region_dd);
    pomo_sholat_card.append(&sh_dd_row);

    settings_root.append(&pomo_sholat_card);

    // --- SECTION 1: PROVIDER & GAYA LIRIK DINAMIS ---
    let lyr_card = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    lyr_card.add_css_class("settings-card");

    let lyr_sec_top = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    let lyr_sec_lbl = gtk4::Label::new(Some("PROVIDER & GAYA LIRIK DINAMIS"));
    lyr_sec_lbl.add_css_class("settings-section-title");
    lyr_sec_lbl.set_hexpand(true);
    lyr_sec_lbl.set_halign(gtk4::Align::Start);

    let ext_provider_status_lbl = gtk4::Label::new(Some("AUTO • Siap"));
    ext_provider_status_lbl.add_css_class("provider-badge");

    let btn_reload_lyrics = make_icon_pill_btn("view-refresh-symbolic", "Muat Ulang Lirik", false);
    lyr_sec_top.append(&lyr_sec_lbl);
    lyr_sec_top.append(&ext_provider_status_lbl);
    lyr_sec_top.append(&btn_reload_lyrics);
    lyr_card.append(&lyr_sec_top);

    let prov_desc_lbl = gtk4::Label::new(Some(
        "Pilih Provider Lirik:",
    ));
    prov_desc_lbl.add_css_class("sub-label");
    prov_desc_lbl.set_halign(gtk4::Align::Start);
    lyr_card.append(&prov_desc_lbl);

    let prov_grid = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    let prov_rows = [
        gtk4::Box::new(gtk4::Orientation::Horizontal, 6),
        gtk4::Box::new(gtk4::Orientation::Horizontal, 6),
        gtk4::Box::new(gtk4::Orientation::Horizontal, 6),
        gtk4::Box::new(gtk4::Orientation::Horizontal, 6),
    ];
    let cur_prov = ctx.borrow().app_state.lyric_provider.clone();
    let prov_defs = [
        ("auto", "Otomatis (Multi-Provider)"),
        ("yt_captions", "Youtube Auto Captions"),
        ("youlyplus", "YouLy Plus"),
        ("apple", "Apple Music"),
        ("simpmusic", "SimpMusic"),
        ("lrclib", "LrcLib"),
        ("kugou", "KuGou"),
        ("netease", "NetEase"),
        ("qqmusic", "QQMusic"),
        ("deezer", "Deezer"),
        ("yt_subtitle", "Youtube Subtitle"),
        ("yt_music", "Youtube Music"),
    ];
    let mut prov_buttons: Vec<(String, gtk4::Button)> = Vec::new();
    for (idx, (id, label)) in prov_defs.iter().enumerate() {
        let b = gtk4::Button::with_label(label);
        b.add_css_class("pill-btn");
        b.set_hexpand(true);
        if cur_prov.eq_ignore_ascii_case(id) {
            b.add_css_class("primary");
        }
        prov_rows[(idx / 3).min(3)].append(&b);
        prov_buttons.push((id.to_string(), b));
    }
    for row in &prov_rows {
        prov_grid.append(row);
    }
    lyr_card.append(&prov_grid);

    let lstyle_desc_lbl = gtk4::Label::new(Some("Gaya Animasi Lirik:"));
    lstyle_desc_lbl.add_css_class("sub-label");
    lstyle_desc_lbl.set_halign(gtk4::Align::Start);
    lyr_card.append(&lstyle_desc_lbl);

    let lstyle_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    let cur_lstyle = ctx.borrow().app_state.lyric_style.clone();
    let lstyle_defs = [
        ("word_bounce", "Per-Kata Elastis (Word Bounce)"),
        ("karaoke_sweep", "Karaoke Sweep Halus"),
        ("cinema_glow", "Sinematik Fokus Kata"),
    ];
    let mut lstyle_buttons: Vec<(String, gtk4::Button)> = Vec::new();
    for (id, label) in lstyle_defs {
        let b = gtk4::Button::with_label(label);
        b.add_css_class("pill-btn");
        if cur_lstyle.eq_ignore_ascii_case(id) {
            b.add_css_class("primary");
        }
        lstyle_row.append(&b);
        lstyle_buttons.push((id.to_string(), b));
    }
    lyr_card.append(&lstyle_row);

    // Lyric Offset Row (-2500ms .. +2500ms)
    let offset_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    offset_row.set_valign(gtk4::Align::Center);
    let offset_lbl = gtk4::Label::new(Some("Offset Waktu Lirik:"));
    offset_lbl.add_css_class("sub-label");
    let offset_scale =
        gtk4::Scale::with_range(gtk4::Orientation::Horizontal, -2500.0, 2500.0, 50.0);
    offset_scale.set_value(ctx.borrow().app_state.lyric_offset_ms as f64);
    offset_scale.set_hexpand(true);
    offset_scale.set_draw_value(false);

    let offset_val_lbl = gtk4::Label::new(Some(&format!(
        "{:+} ms",
        ctx.borrow().app_state.lyric_offset_ms
    )));
    offset_val_lbl.add_css_class("muted-label");
    let btn_offset_zero = gtk4::Button::with_label("0 ms");
    btn_offset_zero.add_css_class("pill-btn");

    offset_row.append(&offset_lbl);
    offset_row.append(&offset_scale);
    offset_row.append(&offset_val_lbl);
    offset_row.append(&btn_offset_zero);
    lyr_card.append(&offset_row);

    settings_root.append(&lyr_card);

    // --- SECTION 2: BENTUK & FISIKA ISLAND ---
    let shape_card = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    shape_card.add_css_class("settings-card");

    let sh_sec_lbl = gtk4::Label::new(Some("BENTUK & ANIMASI"));
    sh_sec_lbl.add_css_class("settings-section-title");
    sh_sec_lbl.set_halign(gtk4::Align::Start);
    shape_card.append(&sh_sec_lbl);

    let shape_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    shape_row.set_valign(gtk4::Align::Center);
    let shape_lbl = gtk4::Label::new(Some("Bentuk Island:"));
    shape_lbl.add_css_class("sub-label");
    shape_lbl.set_width_chars(15);
    shape_lbl.set_halign(gtk4::Align::Start);

    let cur_style = ctx.borrow().app_state.island_style.clone();
    let btn_style_notch = gtk4::Button::with_label("Notch Atas");
    btn_style_notch.add_css_class("pill-btn");
    if cur_style == "notch" {
        btn_style_notch.add_css_class("primary");
    }

    let btn_style_pill = gtk4::Button::with_label("Kapsul Pill");
    btn_style_pill.add_css_class("pill-btn");
    if cur_style == "pill" {
        btn_style_pill.add_css_class("primary");
    }

    let btn_style_dock = gtk4::Button::with_label("Bar Modern");
    btn_style_dock.add_css_class("pill-btn");
    if cur_style == "dock" {
        btn_style_dock.add_css_class("primary");
    }

    shape_row.append(&shape_lbl);
    shape_row.append(&btn_style_notch);
    shape_row.append(&btn_style_pill);
    shape_row.append(&btn_style_dock);
    shape_card.append(&shape_row);

    let anim_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    anim_row.set_valign(gtk4::Align::Center);
    let anim_lbl = gtk4::Label::new(Some("Gaya Animasi:"));
    anim_lbl.add_css_class("sub-label");
    anim_lbl.set_width_chars(15);
    anim_lbl.set_halign(gtk4::Align::Start);

    let cur_anim = ctx.borrow().app_state.anim_preset.clone();
    let btn_anim_smooth = gtk4::Button::with_label("Ultra Smooth");
    btn_anim_smooth.add_css_class("pill-btn");
    if cur_anim == "smooth" {
        btn_anim_smooth.add_css_class("primary");
    }

    let btn_anim_bounce = gtk4::Button::with_label("Spring Elastis");
    btn_anim_bounce.add_css_class("pill-btn");
    if cur_anim == "bounce" {
        btn_anim_bounce.add_css_class("primary");
    }

    let btn_anim_snappy = gtk4::Button::with_label("Cepat / Snappy");
    btn_anim_snappy.add_css_class("pill-btn");
    if cur_anim == "snappy" {
        btn_anim_snappy.add_css_class("primary");
    }

    anim_row.append(&anim_lbl);
    anim_row.append(&btn_anim_smooth);
    anim_row.append(&btn_anim_bounce);
    anim_row.append(&btn_anim_snappy);
    shape_card.append(&anim_row);

    // Width & Max Lyric Width Sliders
    let w_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    w_row.set_valign(gtk4::Align::Center);
    let width_lbl = gtk4::Label::new(Some("Lebar Dasar:"));
    width_lbl.add_css_class("sub-label");
    width_lbl.set_width_chars(15);
    width_lbl.set_halign(gtk4::Align::Start);
    let width_scale = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, 250.0, 420.0, 2.0);
    width_scale.set_value(ctx.borrow().app_state.compact_width as f64);
    width_scale.set_hexpand(true);
    width_scale.set_draw_value(false);
    let width_val_lbl = gtk4::Label::new(Some(&format!(
        "{} px",
        ctx.borrow().app_state.compact_width
    )));
    width_val_lbl.add_css_class("muted-label");
    w_row.append(&width_lbl);
    w_row.append(&width_scale);
    w_row.append(&width_val_lbl);
    shape_card.append(&w_row);

    let mw_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    mw_row.set_valign(gtk4::Align::Center);
    let maxw_lbl = gtk4::Label::new(Some("Lebar Maks Lirik:"));
    maxw_lbl.add_css_class("sub-label");
    maxw_lbl.set_width_chars(15);
    maxw_lbl.set_halign(gtk4::Align::Start);
    let maxw_scale = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, 480.0, 900.0, 4.0);
    maxw_scale.set_value(ctx.borrow().app_state.max_lyric_width.clamp(480, 900) as f64);
    maxw_scale.set_hexpand(true);
    maxw_scale.set_draw_value(false);
    let maxw_val_lbl = gtk4::Label::new(Some(&format!(
        "{} px",
        ctx.borrow().app_state.max_lyric_width.clamp(480, 900)
    )));
    maxw_val_lbl.add_css_class("muted-label");
    mw_row.append(&maxw_lbl);
    mw_row.append(&maxw_scale);
    mw_row.append(&maxw_val_lbl);
    shape_card.append(&mw_row);

    let y_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    y_row.set_valign(gtk4::Align::Center);
    let y_lbl = gtk4::Label::new(Some("Posisi Vertikal Y:"));
    y_lbl.add_css_class("sub-label");
    y_lbl.set_width_chars(15);
    y_lbl.set_halign(gtk4::Align::Start);
    let y_scale = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, 0.0, 48.0, 1.0);
    y_scale.set_value(ctx.borrow().app_state.y_offset as f64);
    y_scale.set_hexpand(true);
    y_scale.set_draw_value(false);
    let y_val_lbl = gtk4::Label::new(Some(&format!("{} px", ctx.borrow().app_state.y_offset)));
    y_val_lbl.add_css_class("muted-label");
    y_row.append(&y_lbl);
    y_row.append(&y_scale);
    y_row.append(&y_val_lbl);
    shape_card.append(&y_row);

    let behavior_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    behavior_row.set_valign(gtk4::Align::Center);
    let hover_chk =
        gtk4::CheckButton::with_label("Buka otomatis saat kursor diarahkan ke Notch (Hover)");
    hover_chk.set_active(ctx.borrow().app_state.hover_expand);
    hover_chk.set_hexpand(true);

    let btn_reset_bottom =
        make_icon_pill_btn("view-refresh-symbolic", "Reset Pengaturan Default", false);
    behavior_row.append(&hover_chk);
    behavior_row.append(&btn_reset_bottom);
    shape_card.append(&behavior_row);

    settings_root.append(&shape_card);
    settings_scroll.set_child(Some(&settings_root));
    settings_win.set_child(Some(&settings_scroll));

    // --- WIRE UP SETTINGS HANDLERS ---
    {
        let ctx_su = ctx.clone();
        let badge_su = startup_badge_lbl;
        let hub_su = notif_hub.clone();
        let top_btn_su = island_startup_btn.clone();

        startup_chk.connect_toggled(move |c| {
            let active = c.is_active();
            let _ = set_autostart_enabled(active);
            {
                let mut st = ctx_su.borrow_mut();
                st.app_state.run_on_startup = active;
                st.vel_w += 75.0;
                st.app_state.save();
            }

            if active {
                badge_su.set_label("AKTIF SAAT BOOT");
                top_btn_su.add_css_class("pinned");
                hub_su.push_notification(
                    "Sistem Arwiland",
                    "system-run-symbolic",
                    "Run on Startup Diaktifkan",
                    "Arwiland akan otomatis berjalan saat komputer dinyalakan.",
                    1,
                    "system",
                );
            } else {
                badge_su.set_label("NONAKTIF");
                top_btn_su.remove_css_class("pinned");
                hub_su.push_notification(
                    "Sistem Arwiland",
                    "system-run-symbolic",
                    "Run on Startup Dinonaktifkan",
                    "Autostart saat komputer menyala telah dimatikan.",
                    0,
                    "system",
                );
            }
        });

        let ctx_ne = ctx.clone();
        notif_enabled_chk.connect_toggled(move |c| {
            let (enabled, sup) = {
                let mut st = ctx_ne.borrow_mut();
                st.app_state.notif_enabled = c.is_active();
                st.app_state.save();
                (
                    st.app_state.notif_enabled,
                    st.app_state.suppress_system_banners,
                )
            };
            set_system_notification_banners_hidden(enabled && sup);
        });

        let ctx_sb = ctx.clone();
        suppress_banners_chk.connect_toggled(move |c| {
            let (enabled, sup) = {
                let mut st = ctx_sb.borrow_mut();
                st.app_state.suppress_system_banners = c.is_active();
                st.app_state.save();
                (
                    st.app_state.notif_enabled,
                    st.app_state.suppress_system_banners,
                )
            };
            set_system_notification_banners_hidden(enabled && sup);
        });

        let ctx_se = ctx.clone();
        sys_events_chk.connect_toggled(move |c| {
            let mut st = ctx_se.borrow_mut();
            st.app_state.system_event_notifs = c.is_active();
            st.app_state.save();
        });

        let ctx_nd = ctx.clone();
        let nd_lbl = notif_dur_val_lbl.clone();
        notif_dur_scale.connect_value_changed(move |s| {
            let val = s.value().round() as i32;
            nd_lbl.set_label(&format!("{} dtk", val));
            let mut st = ctx_nd.borrow_mut();
            st.app_state.notif_duration_secs = val;
            st.app_state.save();
        });

        let all_f_presets = Rc::new(focus_preset_btns);
        {
            let ctx_pf = ctx.clone();
            let pf_val_l = pomo_focus_val_lbl.clone();
            let presets_pf = all_f_presets.clone();
            pomo_focus_scale.connect_value_changed(move |s| {
                let mins = (s.value().round() as u32).clamp(1, 180);
                pf_val_l.set_label(&format!("{} mnt", mins));
                for (m, btn) in presets_pf.iter() {
                    if *m == mins {
                        btn.add_css_class("primary");
                    } else {
                        btn.remove_css_class("primary");
                    }
                }
                let mut st = ctx_pf.borrow_mut();
                if st.app_state.pomo_focus_mins != mins {
                    st.app_state.pomo_focus_mins = mins;
                    if !st.pomo_break_mode && !st.pomo_running {
                        st.pomo_total_secs = mins * 60;
                        st.pomo_remaining_secs = mins * 60;
                    }
                    st.app_state.save();
                }
            });

            for (mins, btn) in all_f_presets.iter() {
                let m = *mins;
                let pf_sc = pomo_focus_scale.clone();
                btn.connect_clicked(move |_| {
                    pf_sc.set_value(m as f64);
                });
            }

            let ctx_pb = ctx.clone();
            let pb_val_l = pomo_break_val_lbl.clone();
            pomo_break_scale.connect_value_changed(move |s| {
                let mins = (s.value().round() as u32).clamp(1, 60);
                pb_val_l.set_label(&format!("{} mnt", mins));
                let mut st = ctx_pb.borrow_mut();
                if st.app_state.pomo_break_mins != mins {
                    st.app_state.pomo_break_mins = mins;
                    if st.pomo_break_mode && !st.pomo_running {
                        st.pomo_total_secs = mins * 60;
                        st.pomo_remaining_secs = mins * 60;
                    }
                    st.app_state.save();
                }
            });
        }

        // Settings Window: Prayer Region (Kota / Kecamatan) Search & Dropdown
        let ext_dd_updating = Rc::new(RefCell::new(false));
        {
            let ctx_dd = ctx.clone();
            let regs_dd = ext_regions_vec.clone();
            let badge_dd = ext_prayer_badge_lbl.clone();
            let upd_flag = ext_dd_updating.clone();
            ext_region_dd.connect_selected_notify(move |dd| {
                if *upd_flag.borrow() {
                    return;
                }
                let idx = dd.selected() as usize;
                let maybe_reg = regs_dd.borrow().get(idx).cloned();
                if let Some(reg) = maybe_reg {
                    badge_dd.set_label(&format!("{} ({})", reg.name, reg.tz_label));
                    let mut st = ctx_dd.borrow_mut();
                    st.app_state.selected_city = reg.name;
                    st.app_state.prayer_lat = Some(reg.lat);
                    st.app_state.prayer_lon = Some(reg.lon);
                    st.app_state.prayer_tz_offset = Some(reg.tz_offset);
                    st.app_state.prayer_tz_label = Some(reg.tz_label);
                    st.vel_w += 65.0;
                    st.app_state.save();
                }
            });

            let repopulate_ext_dd: Rc<dyn Fn(Vec<PrayerRegion>, bool)> = {
                let regs_r = ext_regions_vec.clone();
                let sl_r = ext_region_strlist.clone();
                let dd_r = ext_region_dd.clone();
                let upd_r = ext_dd_updating.clone();
                let ctx_r = ctx.clone();
                let badge_r = ext_prayer_badge_lbl.clone();
                Rc::new(move |new_list: Vec<PrayerRegion>, auto_apply_first: bool| {
                    if new_list.is_empty() {
                        return;
                    }
                    *upd_r.borrow_mut() = true;
                    let labels: Vec<String> = new_list
                        .iter()
                        .map(|r| format!("{} ({})", r.name, r.tz_label))
                        .collect();
                    let refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
                    let old_len = sl_r.n_items();
                    sl_r.splice(0, old_len, &refs);
                    *regs_r.borrow_mut() = new_list.clone();
                    dd_r.set_selected(0);
                    *upd_r.borrow_mut() = false;

                    if auto_apply_first {
                        if let Some(first) = new_list.into_iter().next() {
                            badge_r.set_label(&format!("{} ({})", first.name, first.tz_label));
                            let mut st = ctx_r.borrow_mut();
                            st.app_state.selected_city = first.name;
                            st.app_state.prayer_lat = Some(first.lat);
                            st.app_state.prayer_lon = Some(first.lon);
                            st.app_state.prayer_tz_offset = Some(first.tz_offset);
                            st.app_state.prayer_tz_label = Some(first.tz_label);
                            st.vel_w += 75.0;
                            st.app_state.save();
                        }
                    }
                })
            };

            let repop_live = repopulate_ext_dd.clone();
            ext_region_entry.connect_changed(move |ent| {
                let q = ent.text().trim().to_string();
                let local = search_regions_local(&q);
                if !local.is_empty() {
                    repop_live(local, false);
                }
            });

            let do_ext_search: Rc<dyn Fn()> = {
                let ent_s = ext_region_entry.clone();
                let repop_s = repopulate_ext_dd.clone();
                let badge_s = ext_prayer_badge_lbl.clone();
                Rc::new(move || {
                    let q = ent_s.text().trim().to_string();
                    if q.is_empty() {
                        let all = search_regions_local("");
                        repop_s(all, false);
                        return;
                    }
                    let local = search_regions_local(&q);
                    if !local.is_empty() {
                        repop_s(local.clone(), true);
                    }
                    badge_s.set_label("Mencari Kecamatan/Kota...");
                    let slot: Arc<Mutex<Option<Vec<PrayerRegion>>>> = Arc::new(Mutex::new(None));
                    let slot_bg = slot.clone();
                    let q_bg = q.clone();
                    thread::spawn(move || {
                        let online = search_regions_online(&q_bg);
                        if let Ok(mut g) = slot_bg.lock() {
                            *g = Some(online);
                        }
                    });
                    let repop_cb = repop_s.clone();
                    let local_fallback = local;
                    glib::timeout_add_local(Duration::from_millis(80), move || {
                        let maybe_done = slot.lock().ok().and_then(|mut g| g.take());
                        if let Some(online_res) = maybe_done {
                            let mut merged = online_res;
                            for loc in &local_fallback {
                                if !merged
                                    .iter()
                                    .any(|r| r.name.eq_ignore_ascii_case(&loc.name))
                                {
                                    merged.push(loc.clone());
                                }
                            }
                            if !merged.is_empty() {
                                repop_cb(merged, true);
                            }
                            glib::ControlFlow::Break
                        } else {
                            glib::ControlFlow::Continue
                        }
                    });
                })
            };

            let ds1 = do_ext_search.clone();
            btn_ext_search_region.connect_clicked(move |_| ds1());
            let ds2 = do_ext_search;
            ext_region_entry.connect_activate(move |_| ds2());
        }
    }

    {
        let all_prov_btns = Rc::new(prov_buttons.clone());
        for (id, btn) in &prov_buttons {
            let all_p = all_prov_btns.clone();
            let id_s = id.clone();
            let ctx_p = ctx.clone();
            let l_mgr = lyrics_mgr.clone();
            btn.connect_clicked(move |b| {
                for (_, other) in all_p.iter() {
                    other.remove_css_class("primary");
                }
                b.add_css_class("primary");
                l_mgr.clear_cache();
                let mut st = ctx_p.borrow_mut();
                st.app_state.lyric_provider = id_s.clone();
                st.cur_lyric.clear();
                st.lyric_accent_rgb = provider_accent(&id_s);
                st.vel_w += 85.0;
                st.app_state.save();
            });
        }

        let all_lstyle_btns = Rc::new(lstyle_buttons.clone());
        for (id, btn) in &lstyle_buttons {
            let all_ls = all_lstyle_btns.clone();
            let id_s = id.clone();
            let ctx_ls = ctx.clone();
            btn.connect_clicked(move |b| {
                for (_, other) in all_ls.iter() {
                    other.remove_css_class("primary");
                }
                b.add_css_class("primary");
                let mut st = ctx_ls.borrow_mut();
                st.app_state.lyric_style = id_s.clone();
                st.lyric_anim_prog = 0.0;
                st.vel_w += 75.0;
                st.app_state.save();
            });
        }

        let l_mgr_r = lyrics_mgr.clone();
        let ctx_r = ctx.clone();
        btn_reload_lyrics.connect_clicked(move |_| {
            l_mgr_r.clear_cache();
            let mut st = ctx_r.borrow_mut();
            st.cur_lyric.clear();
            st.vel_w += 90.0;
        });

        let ctx_off = ctx.clone();
        let off_lbl = offset_val_lbl.clone();
        offset_scale.connect_value_changed(move |s| {
            let val = (s.value() / 50.0).round() as i32 * 50;
            off_lbl.set_label(&format!("{:+} ms", val));
            let mut st = ctx_off.borrow_mut();
            st.app_state.lyric_offset_ms = val;
            st.app_state.save();
        });

        let off_sc = offset_scale.clone();
        btn_offset_zero.connect_clicked(move |_| {
            off_sc.set_value(0.0);
        });
    }

    {
        let b_notch = btn_style_notch.clone();
        let b_pill = btn_style_pill.clone();
        let b_dock = btn_style_dock.clone();
        let y_sc = y_scale.clone();

        let apply_style: Rc<dyn Fn(&str)> = {
            let ctx_st = ctx.clone();
            let bg_c = bg_canvas.clone();
            let win_c = main_window.clone();
            let b_n = b_notch.clone();
            let b_p = b_pill.clone();
            let b_d = b_dock.clone();
            let y_s = y_sc.clone();

            Rc::new(move |new_style: &str| {
                b_n.remove_css_class("primary");
                b_p.remove_css_class("primary");
                b_d.remove_css_class("primary");

                let default_y = match new_style {
                    "notch" => {
                        b_n.add_css_class("primary");
                        0
                    }
                    "dock" => {
                        b_d.add_css_class("primary");
                        4
                    }
                    _ => {
                        b_p.add_css_class("primary");
                        6
                    }
                };

                {
                    let mut st = ctx_st.borrow_mut();
                    st.app_state.island_style = new_style.to_string();
                    st.app_state.y_offset = default_y;
                    st.vel_w += 90.0;
                    st.app_state.save();
                }
                y_s.set_value(default_y as f64);
                setup_top_center_overlay(&win_c, CANVAS_WIDTH, CANVAS_HEIGHT, default_y);
                bg_c.queue_draw();
            })
        };

        let ap1 = apply_style.clone();
        btn_style_notch.connect_clicked(move |_| ap1("notch"));
        let ap2 = apply_style.clone();
        btn_style_pill.connect_clicked(move |_| ap2("pill"));
        let ap3 = apply_style.clone();
        btn_style_dock.connect_clicked(move |_| ap3("dock"));
    }

    {
        let b_sm = btn_anim_smooth.clone();
        let b_bn = btn_anim_bounce.clone();
        let b_sn = btn_anim_snappy.clone();

        let apply_anim: Rc<dyn Fn(&str)> = {
            let ctx_an = ctx.clone();
            let bs = b_sm.clone();
            let bb = b_bn.clone();
            let bsn = b_sn.clone();
            Rc::new(move |preset: &str| {
                bs.remove_css_class("primary");
                bb.remove_css_class("primary");
                bsn.remove_css_class("primary");
                match preset {
                    "bounce" => bb.add_css_class("primary"),
                    "snappy" => bsn.add_css_class("primary"),
                    _ => bs.add_css_class("primary"),
                }
                let mut st = ctx_an.borrow_mut();
                st.app_state.anim_preset = preset.to_string();
                st.vel_w += 110.0;
                st.vel_h += 45.0;
                st.app_state.save();
            })
        };

        let an1 = apply_anim.clone();
        btn_anim_smooth.connect_clicked(move |_| an1("smooth"));
        let an2 = apply_anim.clone();
        btn_anim_bounce.connect_clicked(move |_| an2("bounce"));
        let an3 = apply_anim.clone();
        btn_anim_snappy.connect_clicked(move |_| an3("snappy"));
    }

    {
        let ctx_w = ctx.clone();
        let w_val_l = width_val_lbl.clone();
        width_scale.connect_value_changed(move |s| {
            let val = s.value().round() as i32;
            w_val_l.set_label(&format!("{} px", val));
            let mut st = ctx_w.borrow_mut();
            st.app_state.compact_width = val;
            if !st.cached_media.playing {
                st.dynamic_compact_w = val as f64;
                if !st.expanded && st.active_notif_toast.is_none() {
                    st.target_w = val as f64;
                }
            }
            st.app_state.save();
        });

        let ctx_mw = ctx.clone();
        let mw_val_l = maxw_val_lbl.clone();
        maxw_scale.connect_value_changed(move |s| {
            let val = s.value().round() as i32;
            mw_val_l.set_label(&format!("{} px", val));
            let mut st = ctx_mw.borrow_mut();
            st.app_state.max_lyric_width = val;
            st.cur_lyric.clear();
            st.app_state.save();
        });

        let ctx_y = ctx.clone();
        let win_y = main_window.clone();
        let y_val_l = y_val_lbl.clone();
        y_scale.connect_value_changed(move |s| {
            let val = s.value().round() as i32;
            y_val_l.set_label(&format!("{} px", val));
            {
                let mut st = ctx_y.borrow_mut();
                st.app_state.y_offset = val;
                st.app_state.save();
            }
            setup_top_center_overlay(&win_y, CANVAS_WIDTH, CANVAS_HEIGHT, val);
        });

        let ctx_hv = ctx.clone();
        hover_chk.connect_toggled(move |c| {
            let mut st = ctx_hv.borrow_mut();
            st.app_state.hover_expand = c.is_active();
            st.app_state.save();
        });

        // Reset All Settings to Factory Defaults Handler
        let all_prov_r = Rc::new(prov_buttons);
        let all_lstyle_r = Rc::new(lstyle_buttons);
        let reset_fn: Rc<dyn Fn()> = {
            let ctx_res = ctx.clone();
            let l_mgr_res = lyrics_mgr.clone();
            let win_res = main_window.clone();
            let bg_res = bg_canvas.clone();
            let b_n = btn_style_notch;
            let b_p = btn_style_pill;
            let b_d = btn_style_dock;
            let b_sm = btn_anim_smooth;
            let b_bn = btn_anim_bounce;
            let b_sn = btn_anim_snappy;
            let w_sc = width_scale;
            let mw_sc = maxw_scale;
            let y_sc = y_scale;
            let off_sc = offset_scale;
            let hv_ck = hover_chk;
            let ne_ck = notif_enabled_chk;
            let sb_ck = suppress_banners_chk;
            let se_ck = sys_events_chk;
            let nd_sc = notif_dur_scale;
            let pf_sc = pomo_focus_scale.clone();
            let pb_sc = pomo_break_scale.clone();
            let provs = all_prov_r;
            let lstyles = all_lstyle_r;

            Rc::new(move || {
                b_n.add_css_class("primary");
                b_p.remove_css_class("primary");
                b_d.remove_css_class("primary");

                b_sm.add_css_class("primary");
                b_bn.remove_css_class("primary");
                b_sn.remove_css_class("primary");

                for (id, btn) in provs.iter() {
                    if id == "auto" {
                        btn.add_css_class("primary");
                    } else {
                        btn.remove_css_class("primary");
                    }
                }
                for (id, btn) in lstyles.iter() {
                    if id == "word_bounce" {
                        btn.add_css_class("primary");
                    } else {
                        btn.remove_css_class("primary");
                    }
                }

                w_sc.set_value(300.0);
                mw_sc.set_value(780.0);
                y_sc.set_value(0.0);
                off_sc.set_value(0.0);
                hv_ck.set_active(true);
                ne_ck.set_active(true);
                sb_ck.set_active(true);
                se_ck.set_active(true);
                nd_sc.set_value(5.0);
                pf_sc.set_value(25.0);
                pb_sc.set_value(5.0);

                l_mgr_res.clear_cache();
                {
                    let mut st = ctx_res.borrow_mut();
                    st.app_state.island_style = "notch".to_string();
                    st.app_state.anim_preset = "smooth".to_string();
                    st.app_state.compact_width = 300;
                    st.app_state.max_lyric_width = 780;
                    st.app_state.y_offset = 0;
                    st.app_state.hover_expand = true;
                    st.app_state.notif_enabled = true;
                    st.app_state.suppress_system_banners = true;
                    st.app_state.system_event_notifs = true;
                    st.app_state.notif_duration_secs = 5;
                    st.app_state.pomo_focus_mins = 25;
                    st.app_state.pomo_break_mins = 5;
                    if !st.pomo_running {
                        st.pomo_total_secs = if st.pomo_break_mode { 5 * 60 } else { 25 * 60 };
                        st.pomo_remaining_secs = st.pomo_total_secs;
                    }
                    st.app_state.lyric_provider = "auto".to_string();
                    st.app_state.lyric_style = "word_bounce".to_string();
                    st.app_state.lyric_offset_ms = 0;
                    st.lyric_accent_rgb = provider_accent("auto");
                    st.cur_lyric.clear();
                    if !st.cached_media.playing {
                        st.dynamic_compact_w = 300.0;
                        if !st.expanded {
                            st.target_w = 300.0;
                        }
                    }
                    st.vel_w += 110.0;
                    st.app_state.save();
                }

                set_system_notification_banners_hidden(true);
                setup_top_center_overlay(&win_res, CANVAS_WIDTH, CANVAS_HEIGHT, 0);
                bg_res.queue_draw();
            })
        };

        let rf1 = reset_fn.clone();
        btn_reset_defaults.connect_clicked(move |_| rf1());
        let rf2 = reset_fn;
        btn_reset_bottom.connect_clicked(move |_| rf2());
    }

    SettingsHandles {
        window: settings_win,
        ext_provider_status_lbl,
        btn_ext_test_notif,
        startup_chk,
        pomo_focus_scale,
        pomo_break_scale,
        ext_prayer_badge_lbl,
    }
}
