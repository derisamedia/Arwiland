use gtk4::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::lyrics::{provider_accent, LyricsManager};
use crate::notifications::{set_system_notification_banners_hidden, NotificationHub};
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
    let ext_title = gtk4::Label::new(Some("Pengaturan Arwiland Dynamic Island"));
    ext_title.add_css_class("heading-label");
    ext_title.set_halign(gtk4::Align::Start);
    let ext_sub = gtk4::Label::new(Some(
        "Atur notifikasi Live Notch, bentuk Dynamic Island, fisika animasi, dan provider lirik.",
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
    let st_sec_lbl = gtk4::Label::new(Some("STARTUP OTOMATIS (RUN ON STARTUP)"));
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
    let nsec_lbl = gtk4::Label::new(Some("NOTIFIKASI DYNAMIC ISLAND (LIVE NOTCH)"));
    nsec_lbl.add_css_class("settings-section-title");
    nsec_lbl.set_hexpand(true);
    nsec_lbl.set_halign(gtk4::Align::Start);

    let btn_ext_test_notif = make_icon_pill_btn(
        "preferences-system-notifications-symbolic",
        "Kirim Tes Notif ke Notch",
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
        "Pilih Provider Lirik (Mendukung Sinkronisasi Vokal Kata-per-Kata / Word-Sync):",
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

    let lstyle_desc_lbl = gtk4::Label::new(Some("Gaya Animasi Lirik di Notch:"));
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

    let sh_sec_lbl = gtk4::Label::new(Some("BENTUK ISLAND & ANIMASI FISIKA"));
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
    }
}
