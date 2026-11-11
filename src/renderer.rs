use gtk4::cairo;
use gtk4::glib::translate::ToGlibPtr;
use gtk4::prelude::*;
use std::f64::consts::{FRAC_PI_2, PI};
use std::ffi::c_void;

use crate::lyrics::LyricWord;
use crate::notifications::IslandNotification;
use crate::runtime::{RuntimeContext, CANVAS_HEIGHT, COMPACT_H};

extern "C" {
    fn pango_cairo_show_layout(cr: *mut c_void, layout: *mut c_void);
}

pub fn cairo_rounded_rect(cr: &cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let r = r.min(w * 0.5).min(h * 0.5).max(1.0);
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -FRAC_PI_2, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, FRAC_PI_2);
    cr.arc(x + r, y + h - r, r, FRAC_PI_2, PI);
    cr.arc(x + r, y + r, r, PI, 3.0 * FRAC_PI_2);
    cr.close_path();
}

pub fn cairo_notch_path_at(cr: &cairo::Context, x0: f64, w: f64, h: f64, ear_r: f64, bot_r: f64) {
    let er = ear_r.min(w * 0.16).min(h * 0.45).max(2.0);
    let br = bot_r.min((w - 2.0 * er) * 0.45).min(h * 0.5).max(4.0);

    cr.new_sub_path();
    cr.move_to(x0, 0.0);
    cr.arc(x0, er, er, -FRAC_PI_2, 0.0);
    cr.arc_negative(x0 + er + br, h - br, br, PI, FRAC_PI_2);
    cr.arc_negative(x0 + w - er - br, h - br, br, FRAC_PI_2, 0.0);
    cr.arc(x0 + w, er, er, PI, 3.0 * FRAC_PI_2);
    cr.close_path();
}

pub fn draw_pango_layout_at(cr: &cairo::Context, layout: &gtk4::pango::Layout, x: f64, y: f64) {
    cr.move_to(x, y);
    let ptr: *mut gtk4::pango::ffi::PangoLayout = layout.to_glib_none().0;
    unsafe {
        pango_cairo_show_layout(cr.to_raw_none() as *mut c_void, ptr as *mut c_void);
    }
}

pub fn lyric_font_for_text(text: &str) -> gtk4::pango::FontDescription {
    let char_len = text.chars().count();
    let size_str = if char_len > 58 {
        "Inter, Cantarell, Sans 8.4"
    } else if char_len > 44 {
        "Inter, Cantarell, Sans 8.9"
    } else {
        "Inter, Cantarell, Sans 9.5"
    };
    let mut fd = gtk4::pango::FontDescription::from_string(size_str);
    fd.set_weight(gtk4::pango::Weight::Semibold);
    fd
}

pub fn measure_unconstrained_text<W: IsA<gtk4::Widget>>(widget: &W, text: &str, bold: bool) -> f64 {
    let layout = widget.create_pango_layout(None);
    let mut font_desc = gtk4::pango::FontDescription::from_string("Inter, Cantarell, Sans 9.5");
    font_desc.set_weight(if bold {
        gtk4::pango::Weight::Bold
    } else {
        gtk4::pango::Weight::Semibold
    });
    layout.set_font_description(Some(&font_desc));
    layout.set_text(text);
    let (w, _h) = layout.pixel_size();
    w as f64
}

pub fn measure_words_exact<W: IsA<gtk4::Widget>>(
    widget: &W,
    full_text: &str,
    words: &[LyricWord],
) -> f64 {
    if full_text == "●  ●  ●" {
        return 44.0;
    }
    let font_lyric = lyric_font_for_text(full_text);
    let full_layout = widget.create_pango_layout(None);
    full_layout.set_font_description(Some(&font_lyric));
    full_layout.set_text(full_text);
    let (full_w, _) = full_layout.pixel_size();

    if words.is_empty() {
        return full_w as f64;
    }

    let space_layout = widget.create_pango_layout(None);
    space_layout.set_font_description(Some(&font_lyric));
    space_layout.set_text(" ");
    let (sp_w, _) = space_layout.pixel_size();
    let sp_f = (sp_w as f64).max(3.5);

    let mut sum_w = 0.0;
    for w_item in words {
        let wl = widget.create_pango_layout(None);
        wl.set_font_description(Some(&font_lyric));
        wl.set_text(&w_item.text);
        let (ww, _) = wl.pixel_size();
        sum_w += ww as f64;
        if w_item.space_after {
            sum_w += sp_f;
        }
    }
    sum_w.max(full_w as f64)
}

fn draw_notif_category_icon(
    area: &gtk4::DrawingArea,
    cr: &cairo::Context,
    notif: &IslandNotification,
    cx: f64,
    cy: f64,
    ease: f64,
    (nr, ng, nb): (f64, f64, f64),
) {
    let app_l = notif.app_name.to_lowercase();
    let sum_l = notif.summary.to_lowercase();
    cr.set_source_rgba(
        (nr * 0.30 + 0.70).min(1.0),
        (ng * 0.30 + 0.70).min(1.0),
        (nb * 0.30 + 0.70).min(1.0),
        0.97 * ease,
    );

    if notif.urgency >= 2 {
        let mut f_excl = gtk4::pango::FontDescription::from_string("Inter, Cantarell, Sans 14");
        f_excl.set_weight(gtk4::pango::Weight::Bold);
        let l_ex = area.create_pango_layout(None);
        l_ex.set_font_description(Some(&f_excl));
        l_ex.set_text("!");
        let (ew, eh) = l_ex.pixel_size();
        cr.set_source_rgba(1.0, 0.88, 0.90, 0.98 * ease);
        draw_pango_layout_at(cr, &l_ex, cx - ew as f64 * 0.5, cy - eh as f64 * 0.5);
    } else if app_l.contains("whatsapp")
        || app_l.contains("zapfast")
        || app_l.contains("telegram")
        || app_l.contains("discord")
    {
        cairo_rounded_rect(cr, cx - 7.5, cy - 6.5, 15.0, 11.5, 4.0);
        let _ = cr.fill();
        cr.move_to(cx - 3.5, cy + 4.5);
        cr.line_to(cx - 6.5, cy + 8.0);
        cr.line_to(cx + 0.5, cy + 4.5);
        cr.close_path();
        let _ = cr.fill();
    } else if app_l.contains("baterai")
        || app_l.contains("daya")
        || app_l.contains("power")
        || sum_l.contains("baterai")
    {
        cr.move_to(cx + 1.2, cy - 7.5);
        cr.line_to(cx - 5.2, cy + 0.8);
        cr.line_to(cx - 0.5, cy + 0.8);
        cr.line_to(cx - 1.8, cy + 7.5);
        cr.line_to(cx + 5.2, cy - 1.0);
        cr.line_to(cx + 0.5, cy - 1.0);
        cr.close_path();
        let _ = cr.fill();
    } else if app_l.contains("volume") || sum_l.contains("volume") {
        cr.move_to(cx - 6.5, cy - 2.5);
        cr.line_to(cx - 3.0, cy - 2.5);
        cr.line_to(cx + 1.5, cy - 6.5);
        cr.line_to(cx + 1.5, cy + 6.5);
        cr.line_to(cx - 3.0, cy + 2.5);
        cr.line_to(cx - 6.5, cy + 2.5);
        cr.close_path();
        let _ = cr.fill();
        cr.set_line_width(1.6);
        cr.arc(cx + 1.5, cy, 4.5, -0.75, 0.75);
        let _ = cr.stroke();
    } else {
        cr.arc(cx, cy - 2.2, 5.4, PI, 0.0);
        cr.line_to(cx + 6.8, cy + 4.2);
        cr.line_to(cx - 6.8, cy + 4.2);
        cr.close_path();
        let _ = cr.fill();

        cr.arc(cx, cy + 6.2, 2.1, 0.0, PI);
        let _ = cr.fill();
    }
}

fn draw_live_notification_toast(
    area: &gtk4::DrawingArea,
    cr: &cairo::Context,
    st: &RuntimeContext,
    notif: &IslandNotification,
    inner_left: f64,
    inner_right: f64,
    h: f64,
) {
    let inner_w = (inner_right - inner_left).max(40.0);
    let (nr, ng, nb) = notif.accent_rgb();
    let t_in = st.notif_anim_prog.clamp(0.0, 1.0);
    let ease = 1.0 - (1.0 - t_in).powi(3);
    let slide_y = (1.0 - ease) * 10.0;

    let _ = cr.save();
    cr.rectangle(inner_left - 4.0, 1.0, inner_w + 8.0, (h - 2.0).max(20.0));
    cr.clip();

    // Left Icon Badge Squircle (flat, no gradient/glow)
    let badge_sz = 38.0;
    let badge_x = inner_left;
    let badge_y = (h - badge_sz) * 0.5 + slide_y;

    cairo_rounded_rect(cr, badge_x, badge_y, badge_sz, badge_sz, 11.0);
    cr.set_source_rgba(nr, ng, nb, 0.24 * ease);
    let _ = cr.fill();

    draw_notif_category_icon(
        area,
        cr,
        notif,
        badge_x + badge_sz * 0.5,
        badge_y + badge_sz * 0.5,
        ease,
        (nr, ng, nb),
    );

    let text_left = badge_x + badge_sz + 12.0;
    let text_avail_w = (inner_right - text_left).max(40.0);

    // Top Row: [APP NAME] • Time
    let mut font_tag =
        gtk4::pango::FontDescription::from_string("JetBrains Mono, Cantarell, Sans 8.0");
    font_tag.set_weight(gtk4::pango::Weight::Bold);

    let urgency_prefix = if notif.urgency >= 2 {
        "PENTING • "
    } else {
        ""
    };
    let top_tag_str = format!(
        "{}{} • {}",
        urgency_prefix,
        notif.app_name.to_uppercase(),
        notif.timestamp_str
    );
    let tag_layout = area.create_pango_layout(None);
    tag_layout.set_font_description(Some(&font_tag));
    tag_layout.set_text(&top_tag_str);
    let (_tag_w, tag_h) = tag_layout.pixel_size();

    let top_y = (badge_y - 1.0).max(7.0);
    cr.set_source_rgba(
        (nr * 0.28 + 0.72).min(1.0),
        (ng * 0.28 + 0.72).min(1.0),
        (nb * 0.28 + 0.72).min(1.0),
        0.96 * ease,
    );
    draw_pango_layout_at(cr, &tag_layout, text_left, top_y);

    let has_distinct_body = !notif.body.is_empty() && notif.body != notif.summary;

    if has_distinct_body {
        let sum_y = top_y + (tag_h as f64) + 1.5;
        let mut font_sum = gtk4::pango::FontDescription::from_string("Inter, Cantarell, Sans 9.6");
        font_sum.set_weight(gtk4::pango::Weight::Bold);
        let sum_layout = area.create_pango_layout(None);
        sum_layout.set_font_description(Some(&font_sum));
        sum_layout.set_width((text_avail_w as i32) * gtk4::pango::SCALE);
        sum_layout.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        sum_layout.set_single_paragraph_mode(true);
        sum_layout.set_text(&notif.summary);
        let (_sw, sh) = sum_layout.pixel_size();

        cr.set_source_rgba(0.98, 0.99, 1.0, 0.98 * ease);
        draw_pango_layout_at(cr, &sum_layout, text_left, sum_y);

        let body_y = sum_y + (sh as f64) + 1.0;
        let _ = cr.save();
        cr.rectangle(text_left, body_y - 1.0, text_avail_w, 19.0);
        cr.clip();

        let mut font_body = gtk4::pango::FontDescription::from_string("Inter, Cantarell, Sans 8.8");
        font_body.set_weight(gtk4::pango::Weight::Medium);
        let body_layout = area.create_pango_layout(None);
        body_layout.set_font_description(Some(&font_body));
        body_layout.set_single_paragraph_mode(true);
        body_layout.set_text(&notif.body);
        let (bw_px, _bh_px) = body_layout.pixel_size();
        let bw_f = bw_px as f64;

        let scroll_x = if bw_f > text_avail_w {
            let overflow = bw_f - text_avail_w + 14.0;
            let wait_sec = 0.85;
            if st.notif_scroll_phase > wait_sec {
                let prog_s = ((st.notif_scroll_phase - wait_sec) * 42.0).min(overflow);
                text_left - prog_s
            } else {
                text_left
            }
        } else {
            text_left
        };

        cr.set_source_rgba(0.84, 0.86, 0.92, 0.92 * ease);
        draw_pango_layout_at(cr, &body_layout, scroll_x, body_y);
        let _ = cr.restore();
    } else {
        let sum_y = top_y + (tag_h as f64) + 6.5;
        let _ = cr.save();
        cr.rectangle(text_left, sum_y - 2.0, text_avail_w, 24.0);
        cr.clip();

        let mut font_msg = gtk4::pango::FontDescription::from_string("Inter, Cantarell, Sans 10.2");
        font_msg.set_weight(gtk4::pango::Weight::Bold);
        let msg_layout = area.create_pango_layout(None);
        msg_layout.set_font_description(Some(&font_msg));
        msg_layout.set_single_paragraph_mode(true);
        msg_layout.set_text(&notif.summary);
        let (mw_px, _mh_px) = msg_layout.pixel_size();
        let mw_f = mw_px as f64;

        let scroll_x = if mw_f > text_avail_w {
            let overflow = mw_f - text_avail_w + 12.0;
            let wait_sec = 0.85;
            if st.notif_scroll_phase > wait_sec {
                let prog_s = ((st.notif_scroll_phase - wait_sec) * 42.0).min(overflow);
                text_left - prog_s
            } else {
                text_left
            }
        } else {
            text_left
        };

        cr.set_source_rgba(0.98, 0.99, 1.0, 0.98 * ease);
        draw_pango_layout_at(cr, &msg_layout, scroll_x, sum_y);
        let _ = cr.restore();
    }

    let _ = cr.restore();
}

fn draw_volume_osd(
    area: &gtk4::DrawingArea,
    cr: &cairo::Context,
    st: &RuntimeContext,
    inner_left: f64,
    inner_right: f64,
    h: f64,
) {
    let inner_w = (inner_right - inner_left).max(40.0);
    let muted = st.cur_volume_muted || st.cur_volume_pct == 0;
    let (vr, vg, vb) = if muted {
        (0.96, 0.25, 0.37)
    } else {
        (0.22, 0.74, 0.97)
    };

    let t_in = st.vol_osd_anim_prog.clamp(0.0, 1.0);
    let ease = 1.0 - (1.0 - t_in).powi(3);
    let cy = h * 0.5;

    let _ = cr.save();
    cr.rectangle(inner_left - 4.0, 1.0, inner_w + 8.0, (h - 2.0).max(20.0));
    cr.clip();

    // 1. Left Speaker / Mute Button Squircle (flat, no gradient/glow)
    let badge_sz = 30.0;
    let badge_x = inner_left;
    let badge_y = cy - badge_sz * 0.5;

    cairo_rounded_rect(cr, badge_x, badge_y, badge_sz, badge_sz, 9.5);
    cr.set_source_rgba(vr, vg, vb, 0.22 * ease);
    let _ = cr.fill();

    let scx = badge_x + badge_sz * 0.5;
    let scy = cy;
    cr.set_source_rgba(
        (vr * 0.25 + 0.75).min(1.0),
        (vg * 0.25 + 0.75).min(1.0),
        (vb * 0.25 + 0.75).min(1.0),
        0.98 * ease,
    );

    // Speaker Cone
    cr.move_to(scx - 6.5, scy - 2.4);
    cr.line_to(scx - 3.2, scy - 2.4);
    cr.line_to(scx + 0.8, scy - 6.0);
    cr.line_to(scx + 0.8, scy + 6.0);
    cr.line_to(scx - 3.2, scy + 2.4);
    cr.line_to(scx - 6.5, scy + 2.4);
    cr.close_path();
    let _ = cr.fill();

    if muted {
        cr.set_line_width(1.7);
        cr.move_to(scx + 3.0, scy - 3.0);
        cr.line_to(scx + 7.4, scy + 3.0);
        let _ = cr.stroke();
        cr.move_to(scx + 7.4, scy - 3.0);
        cr.line_to(scx + 3.0, scy + 3.0);
        let _ = cr.stroke();
    } else {
        cr.set_line_width(1.5);
        cr.arc(scx + 1.0, scy, 3.6, -0.70, 0.70);
        let _ = cr.stroke();
        if st.cur_volume_pct >= 45 {
            cr.arc(scx + 1.0, scy, 6.2, -0.75, 0.75);
            let _ = cr.stroke();
        }
    }

    // 2. Right Volume Percentage Label
    let mut font_pct =
        gtk4::pango::FontDescription::from_string("JetBrains Mono, Cantarell, Sans 9.2");
    font_pct.set_weight(gtk4::pango::Weight::Bold);
    let pct_str = if muted {
        "MUTE".to_string()
    } else {
        format!("{}%", (st.disp_volume_pct.round() as u32).min(100))
    };
    let pct_layout = area.create_pango_layout(None);
    pct_layout.set_font_description(Some(&font_pct));
    pct_layout.set_text(&pct_str);
    let (pw, ph) = pct_layout.pixel_size();
    let pct_x = inner_right - pw as f64 - 2.0;
    let pct_y = cy - (ph as f64) * 0.5;
    if muted {
        cr.set_source_rgba(1.0, 0.45, 0.55, 0.96 * ease);
    } else {
        cr.set_source_rgba(0.96, 0.98, 1.0, 0.96 * ease);
    }
    draw_pango_layout_at(cr, &pct_layout, pct_x, pct_y);

    // 3. Middle Interactive Slider Bar + Draggable Knob
    let slider_left = badge_x + badge_sz + 12.0;
    let slider_right = (inner_right - 48.0).max(slider_left + 60.0);
    let slider_w = slider_right - slider_left;
    let track_h = if st.vol_osd_dragging { 7.8 } else { 6.2 };
    let track_y = cy - track_h * 0.5;

    // Track Background
    cairo_rounded_rect(cr, slider_left, track_y, slider_w, track_h, track_h * 0.5);
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.14 * ease);
    let _ = cr.fill();

    // Active Track Fill
    let frac = if muted {
        0.0
    } else {
        (st.disp_volume_pct / 100.0).clamp(0.0, 1.0)
    };
    if frac > 0.005 {
        let fill_w = (slider_w * frac).max(track_h);
        cairo_rounded_rect(cr, slider_left, track_y, fill_w, track_h, track_h * 0.5);
        cr.set_source_rgba(vr, vg, vb, 0.96 * ease);
        let _ = cr.fill();
    }

    // Slider Thumb Handle (Knob)
    let thumb_x = slider_left + slider_w * frac;
    let knob_r = if st.vol_osd_dragging { 7.2 } else { 5.8 };

    cr.arc(thumb_x, cy, knob_r, 0.0, 2.0 * PI);
    cr.set_source_rgba(0.99, 0.99, 1.0, 0.98 * ease);
    let _ = cr.fill();

    cr.arc(thumb_x, cy, 2.3, 0.0, 2.0 * PI);
    cr.set_source_rgba(vr, vg, vb, 0.95 * ease);
    let _ = cr.fill();

    let _ = cr.restore();
}

pub fn draw_island_canvas(
    area: &gtk4::DrawingArea,
    cr: &cairo::Context,
    width: i32,
    st: &RuntimeContext,
) {
    let canvas_w = width as f64;
    let w = st.cur_w.clamp(200.0, canvas_w - 4.0);
    let h = st.cur_h.clamp(COMPACT_H, CANVAS_HEIGHT as f64 - 2.0);
    let x0 = (canvas_w - w) * 0.5;

    let style = st.app_state.island_style.as_str();
    let lyric_style = st.app_state.lyric_style.as_str();
    let exp_h = st.expanded_target_h.max(160.0);
    let prog = ((h - COMPACT_H) / (exp_h - COMPACT_H)).clamp(0.0, 1.0);
    let is_playing = st.cached_media.has_player && st.cached_media.playing;
    let has_vol_osd = !st.expanded && st.vol_osd_remaining > 0.0;
    let has_toast = !st.expanded
        && !has_vol_osd
        && st.active_notif_toast.is_some()
        && st.notif_toast_remaining > 0.0;
    let has_expanded_toast =
        st.expanded && st.active_notif_toast.is_some() && st.notif_toast_remaining > 0.0;

    let (ar, ag, ab) = if has_vol_osd {
        if st.cur_volume_muted || st.cur_volume_pct == 0 {
            (0.96, 0.25, 0.37)
        } else {
            (0.22, 0.74, 0.97)
        }
    } else if let Some(ref notif) = st.active_notif_toast {
        if st.notif_toast_remaining > 0.0 {
            notif.accent_rgb()
        } else {
            st.lyric_accent_rgb
        }
    } else {
        st.lyric_accent_rgb
    };

    let mut font_bold = gtk4::pango::FontDescription::from_string("Inter, Cantarell, Sans 9.5");
    font_bold.set_weight(gtk4::pango::Weight::Bold);

    let font_lyric = lyric_font_for_text(&st.cur_lyric);

    // 1. Draw borderless Notch / Pill / Bar silhouette
    if style == "notch" {
        let ear_r = 14.0;
        let bot_r = 17.5 + prog * 6.5;
        cairo_notch_path_at(cr, x0, w, h - 1.0, ear_r, bot_r);
        cr.set_source_rgb(0.025, 0.025, 0.038);
        let _ = cr.fill();

        cr.set_source_rgb(0.025, 0.025, 0.038);
        cr.rectangle(x0 + ear_r, 0.0, (w - 2.0 * ear_r).max(1.0), 1.6);
        let _ = cr.fill();
    } else if style == "dock" {
        let r = 10.0 + prog * 4.0;
        cairo_rounded_rect(cr, x0, 1.0, w, h - 2.0, r);
        cr.set_source_rgb(0.03, 0.03, 0.045);
        let _ = cr.fill();
    } else {
        let r = 19.0 + prog * 7.0;
        cairo_rounded_rect(cr, x0, 1.0, w, h - 2.0, r);
        cr.set_source_rgb(0.028, 0.028, 0.042);
        let _ = cr.fill();
    }

    let pad_x = if style == "notch" { 22.0 } else { 16.0 };
    let inner_left = x0 + pad_x;
    let inner_right = x0 + w - pad_x;
    let inner_w = (inner_right - inner_left).max(40.0);

    // 2A-0. Interactive Volume HUD Mode inside Collapsed Notch
    if has_vol_osd {
        draw_volume_osd(area, cr, st, inner_left, inner_right, h);
        return;
    }

    // 2A. Live Notification Toast Mode inside Collapsed Notch
    if has_toast {
        if let Some(ref notif) = st.active_notif_toast {
            draw_live_notification_toast(area, cr, st, notif, inner_left, inner_right, h);
            return;
        }
    }

    // 2B. Standard Compact Top Bar Content (Clock + Unread Badge + Live Expanded Banner / Lyrics / Sholat)
    let center_y = COMPACT_H * 0.5;

    let _ = cr.save();
    cr.rectangle(inner_left, 1.0, inner_w, COMPACT_H - 2.0);
    cr.clip();

    let mut left_cursor = inner_left;
    if is_playing {
        let bar_w = 2.4;
        let gap = 2.1;
        for i in 0..3 {
            let phase_off = i as f64 * 1.35;
            let amp = ((st.wave_phase + phase_off).sin() * 0.5 + 0.5).clamp(0.18, 1.0);
            let bh = 4.2 + amp * 10.0;
            let bx = left_cursor + (i as f64) * (bar_w + gap);
            let by = center_y - bh * 0.5;
            cairo_rounded_rect(cr, bx, by, bar_w, bh, 1.2);
            if i == 1 {
                cr.set_source_rgba(ar, ag, ab, 0.98);
            } else {
                cr.set_source_rgba(ar * 0.85 + 0.15, ag * 0.85 + 0.15, ab * 0.85 + 0.15, 0.86);
            }
            let _ = cr.fill();
        }
        left_cursor += 3.0 * (bar_w + gap) + 5.5;
    } else {
        cr.arc(left_cursor + 4.0, center_y, 3.2, 0.0, 2.0 * PI);
        if st.unread_notif_count > 0 {
            cr.set_source_rgba(0.20, 0.83, 0.60, 0.95);
        } else {
            cr.set_source_rgba(0.51, 0.55, 0.97, 0.85);
        }
        let _ = cr.fill();
        left_cursor += 12.5;
    }

    let time_layout = area.create_pango_layout(None);
    time_layout.set_font_description(Some(&font_bold));
    time_layout.set_text(&st.clock_str);
    let (tw, th) = time_layout.pixel_size();
    cr.set_source_rgba(0.97, 0.98, 1.0, 0.96);
    draw_pango_layout_at(cr, &time_layout, left_cursor, center_y - (th as f64) * 0.5);
    left_cursor += (tw as f64) + 8.0;

    // Unread Notification Pill Badge in Compact Notch when unread_notif_count > 0
    if st.unread_notif_count > 0 {
        let mut f_badge =
            gtk4::pango::FontDescription::from_string("JetBrains Mono, Cantarell, Sans 7.8");
        f_badge.set_weight(gtk4::pango::Weight::Bold);
        let b_txt = format!("{}", st.unread_notif_count.min(99));
        let b_lay = area.create_pango_layout(None);
        b_lay.set_font_description(Some(&f_badge));
        b_lay.set_text(&b_txt);
        let (bw, bh) = b_lay.pixel_size();
        let pill_w = (bw as f64 + 10.0).max(16.0);
        let pill_h = 15.0;
        let py = center_y - pill_h * 0.5;

        cairo_rounded_rect(cr, left_cursor, py, pill_w, pill_h, 7.5);
        cr.set_source_rgba(0.06, 0.73, 0.51, 0.26);
        let _ = cr.fill();

        cr.set_source_rgba(0.25, 0.92, 0.68, 0.98);
        draw_pango_layout_at(
            cr,
            &b_lay,
            left_cursor + (pill_w - bw as f64) * 0.5,
            center_y - (bh as f64) * 0.5,
        );
        left_cursor += pill_w + 8.0;
    } else {
        left_cursor += 6.0;
    }

    let lyric_avail_w = (inner_right - left_cursor).max(10.0);
    let _ = cr.save();
    cr.rectangle(left_cursor, 0.5, lyric_avail_w, COMPACT_H - 1.0);
    cr.clip();

    if has_expanded_toast {
        if let Some(ref notif) = st.active_notif_toast {
            let (nr, ng, nb) = notif.accent_rgb();
            let pill_txt = if notif.body.is_empty() {
                format!("{} • {}", notif.app_name.to_uppercase(), notif.summary)
            } else {
                format!(
                    "{} • {} — {}",
                    notif.app_name.to_uppercase(),
                    notif.summary,
                    notif.body
                )
            };
            let mut f_strip =
                gtk4::pango::FontDescription::from_string("Inter, Cantarell, Sans 8.8");
            f_strip.set_weight(gtk4::pango::Weight::Semibold);
            let strip_lay = area.create_pango_layout(None);
            strip_lay.set_font_description(Some(&f_strip));
            strip_lay.set_text(&pill_txt);
            let (sw, sh) = strip_lay.pixel_size();
            let sw_f = sw as f64;
            let safe_w = (lyric_avail_w - 6.0).max(20.0);
            let sx = if sw_f > safe_w {
                let overflow = sw_f - safe_w;
                let scroll_t = ((st.notif_scroll_phase - 0.5).max(0.0) * 0.32).sin() * 0.5 + 0.5;
                left_cursor - overflow * scroll_t
            } else {
                (inner_right - 4.0 - sw_f).max(left_cursor)
            };
            let sy = center_y - (sh as f64) * 0.5;
            cr.set_source_rgba(
                (nr * 0.35 + 0.65).min(1.0),
                (ng * 0.35 + 0.65).min(1.0),
                (nb * 0.35 + 0.65).min(1.0),
                0.96,
            );
            draw_pango_layout_at(cr, &strip_lay, sx, sy);
        }
    } else if is_playing && !st.cur_lyric.is_empty() {
        let t = st.lyric_anim_prog.clamp(0.0, 1.0);
        let ease = 1.0 - (1.0 - t).powi(3);

        // Outgoing lyric line (slides upward & fades out)
        if t < 0.995 && !st.prev_lyric.is_empty() {
            let prev_font = lyric_font_for_text(&st.prev_lyric);
            let prev_layout = area.create_pango_layout(None);
            prev_layout.set_font_description(Some(&prev_font));
            prev_layout.set_text(&st.prev_lyric);
            let (pw, ph) = prev_layout.pixel_size();
            let px = (inner_right - 3.0 - pw as f64).max(left_cursor);
            let py = center_y - (ph as f64) * 0.5 - 14.0 * ease;
            let alpha_out = ((1.0 - ease) * (1.0 - ease)).clamp(0.0, 1.0);
            cr.set_source_rgba(0.78, 0.82, 0.96, 0.72 * alpha_out);
            draw_pango_layout_at(cr, &prev_layout, px, py);
        }

        // Incoming / active lyric line
        let full_layout = area.create_pango_layout(None);
        full_layout.set_font_description(Some(&font_lyric));
        full_layout.set_text(&st.cur_lyric);
        let (cw_px, ch_px) = full_layout.pixel_size();
        let cw_f = cw_px as f64;
        let ch_f = ch_px as f64;
        let base_y = center_y - ch_f * 0.5 + (1.0 - ease) * 14.0;

        if st.cur_lyric == "●  ●  ●" {
            let dots_right_x = (inner_right - 8.0).max(left_cursor + 34.0);
            let start_dot_x = dots_right_x - 27.0;
            for i in 0..3 {
                let dx = start_dot_x + (i as f64) * 13.5;
                let base_dot_y = center_y + (1.0 - ease) * 10.0;
                if i < st.active_word_idx {
                    cr.arc(dx, base_dot_y, 3.8, 0.0, 2.0 * PI);
                    cr.set_source_rgba(
                        (ar * 0.35 + 0.65).min(1.0),
                        (ag * 0.35 + 0.65).min(1.0),
                        (ab * 0.35 + 0.65).min(1.0),
                        0.96 * ease,
                    );
                    let _ = cr.fill();
                } else if i == st.active_word_idx {
                    let pulse = (st.word_prog * PI).sin();
                    let radius = 3.1 + 1.55 * pulse;
                    let dy = base_dot_y - 2.2 * pulse;

                    let fill_a = (0.42 + 0.58 * st.word_prog) * ease;
                    cr.arc(dx, dy, radius, 0.0, 2.0 * PI);
                    cr.set_source_rgba(0.99, 0.99, 1.0, fill_a);
                    let _ = cr.fill();
                } else {
                    cr.arc(dx, base_dot_y, 3.0, 0.0, 2.0 * PI);
                    cr.set_source_rgba(0.74, 0.79, 0.92, 0.35 * ease);
                    let _ = cr.fill();
                }
            }
        } else if !st.lyric_is_word_synced {
            let safe_avail = (lyric_avail_w - 4.0).max(10.0);
            let base_x = if cw_f > safe_avail {
                let overflow = cw_f - safe_avail;
                left_cursor - overflow * st.lyric_line_prog.clamp(0.0, 1.0)
            } else {
                (inner_right - 3.0 - cw_f).max(left_cursor)
            };
            cr.set_source_rgba(0.99, 0.99, 1.0, 0.98 * ease);
            draw_pango_layout_at(cr, &full_layout, base_x, base_y);
        } else if (lyric_style == "word_bounce" || lyric_style == "cinema_glow")
            && !st.lyric_words.is_empty()
        {
            let space_layout = area.create_pango_layout(None);
            space_layout.set_font_description(Some(&font_lyric));
            space_layout.set_text(" ");
            let (sp_w, _) = space_layout.pixel_size();
            let sp_f = (sp_w as f64).max(3.5);

            let mut word_layouts: Vec<(gtk4::pango::Layout, f64, f64)> =
                Vec::with_capacity(st.lyric_words.len());
            let mut exact_total_w = 0.0;
            let mut active_center_rel = 0.0;

            for (idx, w_item) in st.lyric_words.iter().enumerate() {
                let w_layout = area.create_pango_layout(None);
                w_layout.set_font_description(Some(&font_lyric));
                w_layout.set_text(&w_item.text);
                let (ww_px, _) = w_layout.pixel_size();
                let ww = ww_px as f64;
                let rel_x = exact_total_w;
                if idx == st.active_word_idx {
                    active_center_rel = rel_x + ww * 0.5;
                }
                word_layouts.push((w_layout, ww, rel_x));
                exact_total_w += ww;
                if w_item.space_after {
                    exact_total_w += sp_f;
                }
            }

            let safe_avail = (lyric_avail_w - 4.0).max(10.0);
            let base_x = if exact_total_w > safe_avail {
                let max_scroll = exact_total_w - safe_avail;
                let desired_scroll = (active_center_rel - safe_avail * 0.45).clamp(0.0, max_scroll);
                left_cursor - desired_scroll
            } else {
                (inner_right - 3.0 - exact_total_w).max(left_cursor)
            };

            for (idx, (w_layout, ww, rel_x)) in word_layouts.into_iter().enumerate() {
                let wx = base_x + rel_x;

                if idx < st.active_word_idx {
                    if lyric_style == "cinema_glow" {
                        cr.set_source_rgba(0.85, 0.88, 0.96, 0.65 * ease);
                    } else {
                        cr.set_source_rgba(0.96, 0.98, 1.0, 0.94 * ease);
                    }
                    draw_pango_layout_at(cr, &w_layout, wx, base_y);
                } else if idx == st.active_word_idx {
                    let bounce_y = if lyric_style == "word_bounce" {
                        -2.1 * (st.word_prog * PI).sin()
                    } else {
                        -1.1 * (st.word_prog * PI).sin()
                    };
                    let wy = base_y + bounce_y;

                    cr.set_source_rgba(0.75, 0.80, 0.94, 0.55 * ease);
                    draw_pango_layout_at(cr, &w_layout, wx, wy);

                    let fill_w = (ww * (0.18 + 0.82 * st.word_prog)).clamp(0.0, ww) + 1.5;
                    let _ = cr.save();
                    cr.rectangle(wx - 0.5, 0.0, fill_w, COMPACT_H);
                    cr.clip();
                    cr.set_source_rgba(
                        (ar * 0.35 + 0.65).min(1.0),
                        (ag * 0.35 + 0.65).min(1.0),
                        (ab * 0.35 + 0.65).min(1.0),
                        1.0 * ease,
                    );
                    draw_pango_layout_at(cr, &w_layout, wx, wy);
                    let _ = cr.restore();
                } else {
                    let dim_alpha = if lyric_style == "cinema_glow" {
                        0.34
                    } else {
                        0.46
                    };
                    cr.set_source_rgba(0.74, 0.79, 0.92, dim_alpha * ease);
                    draw_pango_layout_at(cr, &w_layout, wx, base_y);
                }
            }
        } else {
            let safe_avail = (lyric_avail_w - 4.0).max(10.0);
            let base_x = if cw_f > safe_avail {
                let overflow = cw_f - safe_avail;
                left_cursor - overflow * st.lyric_line_prog.clamp(0.0, 1.0)
            } else {
                (inner_right - 3.0 - cw_f).max(left_cursor)
            };
            cr.set_source_rgba(0.76, 0.81, 0.94, 0.48 * ease);
            draw_pango_layout_at(cr, &full_layout, base_x, base_y);

            let sweep_w = (cw_f * (0.12 + 0.88 * st.lyric_line_prog)).clamp(0.0, cw_f) + 1.5;
            if sweep_w > 0.5 {
                let _ = cr.save();
                cr.rectangle(base_x - 0.5, 0.0, sweep_w, COMPACT_H);
                cr.clip();
                cr.set_source_rgba(0.98, 0.99, 1.0, 0.98 * ease);
                draw_pango_layout_at(cr, &full_layout, base_x, base_y);
                let _ = cr.restore();
            }
        }
    } else {
        let info_layout = area.create_pango_layout(None);
        info_layout.set_font_description(Some(&font_lyric));
        info_layout.set_text(&st.next_prayer_summary);
        let (iw, ih) = info_layout.pixel_size();
        let ix = (inner_right - iw as f64).max(left_cursor);
        let iy = center_y - (ih as f64) * 0.5;
        cr.set_source_rgba(0.84, 0.87, 0.95, 0.88);
        draw_pango_layout_at(cr, &info_layout, ix, iy);
    }

    let _ = cr.restore();
    let _ = cr.restore();
}

pub fn draw_media_card_visualizer(
    _area: &gtk4::DrawingArea,
    cr: &cairo::Context,
    w: i32,
    h: i32,
    st: &RuntimeContext,
) {
    let wf = w as f64;
    let hf = h as f64;
    let cy = hf * 0.5;
    let is_playing = st.cached_media.has_player && st.cached_media.playing;
    let (ar, ag, ab) = st.lyric_accent_rgb;

    let bar_w = 3.2;
    let gap = 3.2;
    let total_w = 4.0 * bar_w + 3.0 * gap;
    let x0 = (wf - total_w).max(0.0);
    let base_heights = [5.5, 9.5, 13.5, 17.0];

    for (i, &bh_idle) in base_heights.iter().enumerate() {
        let bh = if is_playing {
            let phase_off = i as f64 * 1.15;
            let wave = ((st.wave_phase * 1.15 + phase_off).sin() * 0.5 + 0.5).clamp(0.15, 1.0);
            4.5 + wave * 14.5
        } else {
            bh_idle
        };
        let bx = x0 + (i as f64) * (bar_w + gap);
        let by = cy - bh * 0.5;
        cairo_rounded_rect(cr, bx, by, bar_w, bh, bar_w * 0.5);
        cr.set_source_rgba(ar, ag, ab, 0.96);
        let _ = cr.fill();
    }
}

fn format_sec_mm_ss(sec: f64) -> String {
    let total = sec.max(0.0).round() as u64;
    let m = total / 60;
    let s = total % 60;
    format!("{}:{:02}", m, s)
}

pub fn draw_media_card_lyrics_and_progress(
    area: &gtk4::DrawingArea,
    cr: &cairo::Context,
    w: i32,
    _h: i32,
    st: &RuntimeContext,
) {
    let wf = (w as f64).max(80.0);
    let (ar, ag, ab) = st.lyric_accent_rgb;
    let is_playing = st.cached_media.has_player && st.cached_media.playing;

    // 1. Progress Bar + Knob + Elapsed/Total Time
    let pos_sec = st.interp_pos_sec.max(0.0);
    let len_sec = (st.cached_media.length_secs as f64).max(0.0);
    let frac = if len_sec > 0.5 {
        (pos_sec / len_sec).clamp(0.0, 1.0)
    } else if is_playing {
        st.lyric_line_prog.clamp(0.0, 1.0)
    } else {
        0.0
    };

    let track_x = 2.0;
    let track_w = (wf - 4.0).max(40.0);
    let track_y = 6.0;
    let track_h = 4.2;

    cairo_rounded_rect(cr, track_x, track_y, track_w, track_h, track_h * 0.5);
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.14);
    let _ = cr.fill();

    let fill_w = (track_w * frac).max(if frac > 0.002 { track_h } else { 0.0 });
    if fill_w > 0.0 {
        cairo_rounded_rect(cr, track_x, track_y, fill_w, track_h, track_h * 0.5);
        cr.set_source_rgba(ar, ag, ab, 0.96);
        let _ = cr.fill();
    }

    let knob_x = (track_x + track_w * frac).clamp(track_x + 4.5, track_x + track_w - 4.5);
    let knob_y = track_y + track_h * 0.5;
    cr.arc(knob_x, knob_y, 5.2, 0.0, 2.0 * PI);
    cr.set_source_rgba(0.98, 0.99, 1.0, 0.98);
    let _ = cr.fill();

    let mut font_time = gtk4::pango::FontDescription::from_string("Inter, Cantarell, Sans 8.5");
    font_time.set_weight(gtk4::pango::Weight::Medium);

    let left_time_str = format_sec_mm_ss(pos_sec);
    let right_time_str = if len_sec > 0.5 {
        format_sec_mm_ss(len_sec)
    } else {
        "0:00".to_string()
    };

    let l_time = area.create_pango_layout(Some(&left_time_str));
    l_time.set_font_description(Some(&font_time));
    cr.set_source_rgba(0.56, 0.58, 0.63, 0.92);
    draw_pango_layout_at(cr, &l_time, track_x, track_y + 9.0);

    let r_time = area.create_pango_layout(Some(&right_time_str));
    r_time.set_font_description(Some(&font_time));
    let (rt_w, _) = r_time.pixel_size();
    draw_pango_layout_at(cr, &r_time, track_x + track_w - rt_w as f64, track_y + 9.0);

    // 2. 3-Line Karaoke Lyrics Viewport with Smooth Bottom-to-Top Rolling Animation
    let lyr_top = 32.0;
    let lyr_h = 74.0;
    let _ = cr.save();
    cr.rectangle(0.0, lyr_top, wf, lyr_h);
    cr.clip();

    let t = st.lyric_anim_prog.clamp(0.0, 1.0);
    // Cubic ease-out for smooth upward scrolling
    let ease = 1.0 - (1.0 - t).powi(3);

    let prev_txt = if !st.cur_lyric.is_empty() {
        st.prev_lyric.as_str()
    } else {
        "Lirik tampil di sini"
    };
    let main_txt = if !st.cur_lyric.is_empty() {
        st.cur_lyric.as_str()
    } else {
        "baris demi baris,"
    };
    let next_txt = if !st.cur_lyric.is_empty() {
        st.next_lyric.as_str()
    } else {
        "mengikuti irama lagumu"
    };

    let y_slot_prev = lyr_top - 4.0;
    let y_slot_main = lyr_top + 17.0;
    let y_slot_next = lyr_top + 48.0;
    let y_slot_below = lyr_top + 76.0;
    let y_slot_above = lyr_top - 24.0;

    // A. Outgoing old previous line scrolling up & out of the top edge
    if t < 0.99 && !st.prev_prev_lyric.is_empty() && !st.cur_lyric.is_empty() {
        let mut fd_out = gtk4::pango::FontDescription::from_string("Inter, Cantarell, Sans 10.2");
        fd_out.set_weight(gtk4::pango::Weight::Bold);
        let out_lay = area.create_pango_layout(Some(&st.prev_prev_lyric));
        out_lay.set_font_description(Some(&fd_out));
        out_lay.set_width((wf as i32) * gtk4::pango::SCALE);
        out_lay.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        out_lay.set_single_paragraph_mode(true);
        let y_out = y_slot_prev + (y_slot_above - y_slot_prev) * ease;
        let alpha_out = (0.28 * (1.0 - ease)).clamp(0.0, 1.0);
        cr.set_source_rgba(0.62, 0.64, 0.69, alpha_out);
        draw_pango_layout_at(cr, &out_lay, 2.0, y_out);
    }

    // B. Previous line: rolls up from `y_slot_main` (13.5pt) to `y_slot_prev` (10.2pt)
    if !prev_txt.is_empty() {
        let prev_pt = if !st.cur_lyric.is_empty() {
            13.5 + (10.2 - 13.5) * ease
        } else {
            10.2
        };
        let mut fd_prev =
            gtk4::pango::FontDescription::from_string(&format!("Inter, Cantarell, Sans {:.1}", prev_pt));
        fd_prev.set_weight(gtk4::pango::Weight::Bold);
        let p_lay = area.create_pango_layout(Some(prev_txt));
        p_lay.set_font_description(Some(&fd_prev));
        p_lay.set_width((wf as i32) * gtk4::pango::SCALE);
        p_lay.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        p_lay.set_single_paragraph_mode(true);
        let y_prev = if !st.cur_lyric.is_empty() {
            y_slot_main + (y_slot_prev - y_slot_main) * ease
        } else {
            y_slot_prev
        };
        let alpha_prev = if !st.cur_lyric.is_empty() {
            0.78 + (0.28 - 0.78) * ease
        } else {
            0.28
        };
        cr.set_source_rgba(0.62, 0.64, 0.69, alpha_prev);
        draw_pango_layout_at(cr, &p_lay, 2.0, y_prev);
    }

    // C. Main active line: rolls up from `y_slot_next` (10.2pt) to `y_slot_main` (13.5pt)
    let main_pt = if !st.cur_lyric.is_empty() {
        10.2 + (13.5 - 10.2) * ease
    } else {
        13.5
    };
    let mut font_main_lyr =
        gtk4::pango::FontDescription::from_string(&format!("Inter, Cantarell, Sans {:.1}", main_pt));
    font_main_lyr.set_weight(gtk4::pango::Weight::Bold);

    let main_y = if !st.cur_lyric.is_empty() {
        y_slot_next + (y_slot_main - y_slot_next) * ease
    } else {
        y_slot_main
    };

    if main_txt == "●  ●  ●" {
        for i in 0..3 {
            let dx = 10.0 + (i as f64) * 16.0;
            let dy = main_y + 11.0;
            if i <= st.active_word_idx {
                cr.arc(dx, dy, 4.2, 0.0, 2.0 * PI);
                cr.set_source_rgba(0.98, 0.99, 1.0, 0.96);
                let _ = cr.fill();
            } else {
                cr.arc(dx, dy, 3.4, 0.0, 2.0 * PI);
                cr.set_source_rgba(0.52, 0.55, 0.60, 0.42);
                let _ = cr.fill();
            }
        }
    } else if st.lyric_is_word_synced && !st.lyric_words.is_empty() && !st.cur_lyric.is_empty() {
        let space_lay = area.create_pango_layout(Some(" "));
        space_lay.set_font_description(Some(&font_main_lyr));
        let (sp_w, _) = space_lay.pixel_size();
        let sp_f = (sp_w as f64).max(4.5);

        let mut word_layouts = Vec::with_capacity(st.lyric_words.len());
        let mut total_w = 0.0;
        let mut active_center = 0.0;

        for (idx, w_item) in st.lyric_words.iter().enumerate() {
            let wl = area.create_pango_layout(Some(&w_item.text));
            wl.set_font_description(Some(&font_main_lyr));
            let (ww_i, _) = wl.pixel_size();
            let ww = ww_i as f64;
            let rx = total_w;
            if idx == st.active_word_idx {
                active_center = rx + ww * 0.5;
            }
            word_layouts.push((wl, ww, rx));
            total_w += ww;
            if w_item.space_after {
                total_w += sp_f;
            }
        }

        let avail = (wf - 4.0).max(40.0);
        let base_x = if total_w > avail {
            let max_sc = total_w - avail;
            let sc = (active_center - avail * 0.4).clamp(0.0, max_sc);
            2.0 - sc
        } else {
            2.0
        };

        for (idx, (wl, ww, rx)) in word_layouts.into_iter().enumerate() {
            let wx = base_x + rx;
            if idx < st.active_word_idx {
                cr.set_source_rgba(0.98, 0.99, 1.0, 0.98);
                draw_pango_layout_at(cr, &wl, wx, main_y);
            } else if idx == st.active_word_idx {
                cr.set_source_rgba(0.48, 0.50, 0.55, 0.72);
                draw_pango_layout_at(cr, &wl, wx, main_y);

                let fill_w = (ww * (0.15 + 0.85 * st.word_prog)).clamp(0.0, ww) + 1.0;
                let _ = cr.save();
                cr.rectangle(wx - 0.5, lyr_top, fill_w, lyr_h);
                cr.clip();
                cr.set_source_rgba(0.98, 0.99, 1.0, 0.99);
                draw_pango_layout_at(cr, &wl, wx, main_y);
                let _ = cr.restore();
            } else {
                cr.set_source_rgba(0.48, 0.50, 0.55, 0.72);
                draw_pango_layout_at(cr, &wl, wx, main_y);
            }
        }
    } else {
        let m_lay = area.create_pango_layout(Some(main_txt));
        m_lay.set_font_description(Some(&font_main_lyr));
        let (mw_i, _) = m_lay.pixel_size();
        let mw = mw_i as f64;
        cr.set_source_rgba(0.48, 0.50, 0.55, 0.72);
        draw_pango_layout_at(cr, &m_lay, 2.0, main_y);

        let demo_prog = if !st.cur_lyric.is_empty() {
            st.lyric_line_prog.clamp(0.15, 1.0)
        } else {
            0.46
        };
        let sweep_w = (mw * demo_prog).clamp(0.0, mw);
        let _ = cr.save();
        cr.rectangle(1.5, lyr_top, sweep_w, lyr_h);
        cr.clip();
        cr.set_source_rgba(0.98, 0.99, 1.0, 0.98);
        draw_pango_layout_at(cr, &m_lay, 2.0, main_y);
        let _ = cr.restore();
    }

    // D. Next upcoming line: rolls up from `y_slot_below` into `y_slot_next`
    if !next_txt.is_empty() {
        let mut font_sub_lyr =
            gtk4::pango::FontDescription::from_string("Inter, Cantarell, Sans 10.2");
        font_sub_lyr.set_weight(gtk4::pango::Weight::Bold);
        let n_lay = area.create_pango_layout(Some(next_txt));
        n_lay.set_font_description(Some(&font_sub_lyr));
        n_lay.set_width((wf as i32) * gtk4::pango::SCALE);
        n_lay.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        n_lay.set_single_paragraph_mode(true);
        let y_next = if !st.cur_lyric.is_empty() {
            y_slot_below + (y_slot_next - y_slot_below) * ease
        } else {
            y_slot_next
        };
        let alpha_next = if !st.cur_lyric.is_empty() {
            0.38 * ease
        } else {
            0.36
        };
        cr.set_source_rgba(0.46, 0.48, 0.53, alpha_next);
        draw_pango_layout_at(cr, &n_lay, 2.0, y_next);
    }

    let _ = cr.restore();
}
