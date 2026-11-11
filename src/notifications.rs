use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct IslandNotification {
    pub id: u64,
    pub app_name: String,
    pub app_icon: String,
    pub summary: String,
    pub body: String,
    pub urgency: u8, // 0 = Low, 1 = Normal, 2 = Critical
    pub timestamp_str: String,
    pub category: String,
    pub read: bool,
}

impl IslandNotification {
    pub fn accent_rgb(&self) -> (f64, f64, f64) {
        if self.urgency >= 2 {
            return (0.96, 0.25, 0.37); // Rose / Critical Red
        }
        let app_lower = self.app_name.to_lowercase();
        let cat_lower = self.category.to_lowercase();
        if app_lower.contains("whatsapp")
            || app_lower.contains("zapfast")
            || app_lower.contains("spotify")
        {
            (0.13, 0.77, 0.37) // Vibrant Green
        } else if app_lower.contains("telegram")
            || app_lower.contains("chrome")
            || app_lower.contains("chromium")
            || app_lower.contains("network")
        {
            (0.22, 0.68, 0.98) // Sky Blue
        } else if app_lower.contains("discord") || app_lower.contains("code") {
            (0.51, 0.55, 0.97) // Indigo
        } else if app_lower.contains("firefox")
            || app_lower.contains("baterai")
            || app_lower.contains("power")
            || cat_lower.contains("device")
        {
            (0.96, 0.62, 0.04) // Amber Orange
        } else if app_lower.contains("sholat")
            || app_lower.contains("pomodoro")
            || app_lower.contains("arwiland")
        {
            (0.06, 0.73, 0.51) // Emerald
        } else {
            (0.20, 0.83, 0.60) // Default Emerald-Mint
        }
    }

    pub fn resolved_icon_name(&self) -> &'static str {
        let icon_l = self.app_icon.to_lowercase();
        let app_l = self.app_name.to_lowercase();
        let sum_l = self.summary.to_lowercase();

        if self.urgency >= 2 || icon_l.contains("error") || icon_l.contains("critical") {
            "dialog-warning-symbolic"
        } else if app_l.contains("whatsapp")
            || app_l.contains("zapfast")
            || app_l.contains("telegram")
            || app_l.contains("discord")
            || icon_l.contains("chat")
            || icon_l.contains("message")
        {
            "user-available-symbolic"
        } else if app_l.contains("spotify")
            || app_l.contains("vlc")
            || app_l.contains("media")
            || icon_l.contains("audio")
            || icon_l.contains("music")
        {
            "audio-x-generic-symbolic"
        } else if app_l.contains("baterai")
            || app_l.contains("power")
            || app_l.contains("daya")
            || sum_l.contains("baterai")
            || sum_l.contains("pengisi daya")
            || icon_l.contains("battery")
        {
            "battery-good-charging-symbolic"
        } else if app_l.contains("volume") || sum_l.contains("volume") || icon_l.contains("volume")
        {
            if sum_l.contains("mute") || sum_l.contains("bisu") || icon_l.contains("muted") {
                "audio-volume-muted-symbolic"
            } else {
                "audio-volume-high-symbolic"
            }
        } else if app_l.contains("sholat") || sum_l.contains("sholat") {
            "weather-clear-night-symbolic"
        } else if app_l.contains("pomodoro")
            || sum_l.contains("pomodoro")
            || icon_l.contains("alarm")
        {
            "alarm-symbolic"
        } else if app_l.contains("hidrasi") || sum_l.contains("air") {
            "weather-showers-symbolic"
        } else if app_l.contains("shelf") || app_l.contains("nautilus") || icon_l.contains("folder")
        {
            "folder-download-symbolic"
        } else if app_l.contains("chrome") || app_l.contains("firefox") || app_l.contains("web") {
            "network-workgroup-symbolic"
        } else {
            "preferences-system-notifications-symbolic"
        }
    }

    pub fn badge_color_class(&self) -> &'static str {
        if self.urgency >= 2 {
            return "critical";
        }
        let app_lower = self.app_name.to_lowercase();
        let cat_lower = self.category.to_lowercase();
        if app_lower.contains("telegram")
            || app_lower.contains("chrome")
            || app_lower.contains("chromium")
            || app_lower.contains("network")
            || cat_lower.contains("audio")
        {
            "sky"
        } else if app_lower.contains("discord") || app_lower.contains("code") {
            "indigo"
        } else if app_lower.contains("firefox")
            || app_lower.contains("baterai")
            || app_lower.contains("daya")
            || app_lower.contains("power")
            || cat_lower.contains("device")
        {
            "amber"
        } else {
            "emerald"
        }
    }
}

struct DedupRecord {
    app_name: String,
    summary: String,
    body: String,
    at: Instant,
}

struct HubInner {
    next_id: u64,
    history: VecDeque<IslandNotification>,
    pending_toasts: VecDeque<IslandNotification>,
    recent_dedup: VecDeque<DedupRecord>,
    seq: u64,
}

#[derive(Clone)]
pub struct NotificationHub {
    inner: Arc<Mutex<HubInner>>,
}

impl NotificationHub {
    pub fn new(initial_history: Vec<IslandNotification>) -> Self {
        let filtered: Vec<IslandNotification> = initial_history
            .into_iter()
            .filter(|n| n.category != "audio-volume" && n.app_name != "Volume Sistem")
            .collect();
        let max_id = filtered.iter().map(|n| n.id).max().unwrap_or(0);
        let mut hist = VecDeque::from(filtered);
        while hist.len() > 35 {
            hist.pop_back();
        }
        let hub = Self {
            inner: Arc::new(Mutex::new(HubInner {
                next_id: max_id + 1,
                history: hist,
                pending_toasts: VecDeque::new(),
                recent_dedup: VecDeque::new(),
                seq: 1,
            })),
        };
        hub.spawn_dbus_listener();
        hub
    }

    pub fn push_notification(
        &self,
        app_name: &str,
        app_icon: &str,
        summary: &str,
        body: &str,
        urgency: u8,
        category: &str,
    ) {
        let clean_app = clean_app_title(app_name);
        let clean_sum = strip_html_and_trim(summary);
        let clean_body = strip_html_and_trim(body);

        if clean_sum.is_empty() && clean_body.is_empty() {
            return;
        }

        let final_summary = if clean_sum.is_empty() {
            clean_app.clone()
        } else {
            clean_sum
        };

        let mut guard = self.inner.lock().unwrap();
        let now_inst = Instant::now();

        // Remove expired deduplication entries (> 900ms)
        while let Some(front) = guard.recent_dedup.front() {
            if now_inst.duration_since(front.at) > Duration::from_millis(900) {
                guard.recent_dedup.pop_front();
            } else {
                break;
            }
        }

        // Check duplicate within 900ms (e.g., gnome-shell forwarding or rapid duplicate signals)
        if guard.recent_dedup.iter().any(|r| {
            r.app_name.eq_ignore_ascii_case(&clean_app)
                && r.summary == final_summary
                && r.body == clean_body
        }) {
            return;
        }

        guard.recent_dedup.push_back(DedupRecord {
            app_name: clean_app.clone(),
            summary: final_summary.clone(),
            body: clean_body.clone(),
            at: now_inst,
        });

        // Volume changes are rendered by the dedicated Interactive Volume HUD, not as text notifications
        if category == "audio-volume" || app_icon.contains("audio-volume") {
            return;
        }

        let id = guard.next_id;
        guard.next_id += 1;

        let notif = IslandNotification {
            id,
            app_name: clean_app,
            app_icon: app_icon.trim().to_string(),
            summary: final_summary,
            body: clean_body,
            urgency,
            timestamp_str: Local::now().format("%H:%M").to_string(),
            category: category.to_string(),
            read: false,
        };

        guard.history.push_front(notif.clone());
        while guard.history.len() > 35 {
            guard.history.pop_back();
        }

        guard.pending_toasts.push_back(notif);
        guard.seq = guard.seq.wrapping_add(1);
    }

    pub fn pop_next_toast(&self) -> Option<IslandNotification> {
        let mut guard = self.inner.lock().unwrap();
        guard.pending_toasts.pop_front()
    }

    pub fn seq(&self) -> u64 {
        self.inner.lock().unwrap().seq
    }

    pub fn get_history(&self) -> Vec<IslandNotification> {
        self.inner.lock().unwrap().history.iter().cloned().collect()
    }

    pub fn unread_count(&self) -> usize {
        self.inner
            .lock()
            .unwrap()
            .history
            .iter()
            .filter(|n| !n.read)
            .count()
    }

    pub fn mark_all_read(&self) {
        let mut guard = self.inner.lock().unwrap();
        let mut changed = false;
        for n in guard.history.iter_mut() {
            if !n.read {
                n.read = true;
                changed = true;
            }
        }
        if changed {
            guard.seq = guard.seq.wrapping_add(1);
        }
    }

    pub fn remove_by_id(&self, id: u64) {
        let mut guard = self.inner.lock().unwrap();
        let prev_len = guard.history.len();
        guard.history.retain(|n| n.id != id);
        if guard.history.len() != prev_len {
            guard.seq = guard.seq.wrapping_add(1);
        }
    }

    pub fn clear_all(&self) {
        let mut guard = self.inner.lock().unwrap();
        guard.history.clear();
        guard.pending_toasts.clear();
        guard.seq = guard.seq.wrapping_add(1);
    }

    fn spawn_dbus_listener(&self) {
        let hub = self.clone();
        thread::spawn(move || loop {
            let child_res = Command::new("dbus-monitor")
                .args([
                    "--session",
                    "type='method_call',interface='org.freedesktop.Notifications',member='Notify'",
                    "type='method_call',interface='org.gtk.Notifications',member='AddNotification'",
                    "type='method_call',interface='org.freedesktop.portal.Notification',member='AddNotification'",
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn();

            let Ok(mut child) = child_res else {
                thread::sleep(Duration::from_secs(2));
                continue;
            };

            if let Some(stdout) = child.stdout.take() {
                let reader = BufReader::new(stdout);
                parse_dbus_monitor_stream(reader, &hub);
            }

            let _ = child.kill();
            let _ = child.wait();
            thread::sleep(Duration::from_millis(800));
        });
    }
}

#[derive(PartialEq)]
enum CaptureMode {
    Idle,
    FreedesktopNotify,
    GtkAddNotification,
}

#[derive(Default)]
struct RawNotifyBuilder {
    top_strings: Vec<String>,
    reading_multiline_str: bool,
    multiline_buf: String,
    in_arrays: usize,
    last_dict_key: String,
    hint_icon: String,
    hint_desktop_entry: String,
    hint_category: String,
    hint_urgency: u8,
    is_forwarded_shell_copy: bool,
    gtk_title: String,
    gtk_body: String,
    gtk_priority: String,
}

impl RawNotifyBuilder {
    fn reset(&mut self) {
        *self = Self::default();
    }
}

fn parse_dbus_monitor_stream<R: BufRead>(reader: R, hub: &NotificationHub) {
    let mut mode = CaptureMode::Idle;
    let mut b = RawNotifyBuilder::default();

    for line_res in reader.lines() {
        let Ok(line) = line_res else {
            break;
        };

        if line.starts_with("method call ") {
            // If a previous message wasn't flushed yet, flush it
            flush_builder(&mut mode, &mut b, hub);

            if line.contains("interface=org.freedesktop.Notifications")
                && line.contains("member=Notify")
            {
                mode = CaptureMode::FreedesktopNotify;
                b.reset();
                b.hint_urgency = 1;
            } else if (line.contains("interface=org.gtk.Notifications")
                || line.contains("interface=org.freedesktop.portal.Notification"))
                && line.contains("member=AddNotification")
            {
                mode = CaptureMode::GtkAddNotification;
                b.reset();
                b.hint_urgency = 1;
            } else {
                mode = CaptureMode::Idle;
            }
            continue;
        }

        if line.starts_with("signal ")
            || line.starts_with("method return ")
            || line.starts_with("error ")
        {
            flush_builder(&mut mode, &mut b, hub);
            mode = CaptureMode::Idle;
            continue;
        }

        match mode {
            CaptureMode::Idle => {}
            CaptureMode::FreedesktopNotify => {
                // Handle multiline string continuation for summary/body
                if b.reading_multiline_str {
                    b.multiline_buf.push('\n');
                    if let Some(stripped) = line.strip_suffix('"') {
                        b.multiline_buf.push_str(stripped);
                        b.reading_multiline_str = false;
                        if b.in_arrays == 0 {
                            b.top_strings.push(std::mem::take(&mut b.multiline_buf));
                        }
                    } else {
                        b.multiline_buf.push_str(&line);
                    }
                    continue;
                }

                if line == "   array [" {
                    b.in_arrays += 1;
                    continue;
                }

                // Top-level arguments before arrays: app_name, app_icon, summary, body
                if b.in_arrays == 0 {
                    if let Some(rest) = line.strip_prefix("   string \"") {
                        if let Some(val) = rest.strip_suffix('"') {
                            b.top_strings.push(val.to_string());
                        } else {
                            b.reading_multiline_str = true;
                            b.multiline_buf = rest.to_string();
                        }
                    }
                    continue;
                }

                // Inside hints array (second array)
                if b.in_arrays >= 2 {
                    let trimmed = line.trim();
                    if let Some(k_rest) = line.strip_prefix("         string \"") {
                        if let Some(k) = k_rest.strip_suffix('"') {
                            b.last_dict_key = k.to_string();
                            if k == "x-shell-sender" || k == "x-shell-sender-pid" {
                                b.is_forwarded_shell_copy = true;
                            }
                        }
                    } else if trimmed.starts_with("variant") {
                        if b.last_dict_key == "urgency" {
                            if let Some(byte_str) = trimmed.split("byte ").nth(1) {
                                if let Ok(u) = byte_str.trim().parse::<u8>() {
                                    b.hint_urgency = u;
                                }
                            }
                        } else if b.last_dict_key == "image-path"
                            || b.last_dict_key == "image_path"
                            || b.last_dict_key == "sound-name"
                        {
                            if let Some(s) = extract_quoted(trimmed) {
                                if b.last_dict_key.starts_with("image") {
                                    b.hint_icon = s;
                                }
                            }
                        } else if b.last_dict_key == "desktop-entry" {
                            if let Some(s) = extract_quoted(trimmed) {
                                b.hint_desktop_entry = s;
                            }
                        } else if b.last_dict_key == "category" {
                            if let Some(s) = extract_quoted(trimmed) {
                                b.hint_category = s;
                            }
                        }
                    }
                }

                // Final top-level argument of Notify is always `   int32 <timeout>`
                if line.starts_with("   int32 ") {
                    flush_builder(&mut mode, &mut b, hub);
                }
            }
            CaptureMode::GtkAddNotification => {
                if b.reading_multiline_str {
                    b.multiline_buf.push('\n');
                    if let Some(stripped) = line.strip_suffix('"') {
                        b.multiline_buf.push_str(stripped);
                        b.reading_multiline_str = false;
                        let val = std::mem::take(&mut b.multiline_buf);
                        if b.last_dict_key == "title" {
                            b.gtk_title = val;
                        } else if b.last_dict_key == "body" {
                            b.gtk_body = val;
                        }
                    } else {
                        b.multiline_buf.push_str(&line);
                    }
                    continue;
                }

                if line == "   array [" {
                    b.in_arrays += 1;
                    continue;
                }

                if b.in_arrays == 0 {
                    if let Some(rest) = line.strip_prefix("   string \"") {
                        if let Some(val) = rest.strip_suffix('"') {
                            b.top_strings.push(val.to_string());
                        }
                    }
                    continue;
                }

                let trimmed = line.trim();
                if let Some(k_rest) = line.strip_prefix("         string \"") {
                    if let Some(k) = k_rest.strip_suffix('"') {
                        b.last_dict_key = k.to_string();
                    }
                } else if trimmed.starts_with("variant") {
                    if let Some(first_q) = trimmed.find('"') {
                        let after_q = &trimmed[first_q + 1..];
                        if let Some(val) = after_q.strip_suffix('"') {
                            match b.last_dict_key.as_str() {
                                "title" => b.gtk_title = val.to_string(),
                                "body" => b.gtk_body = val.to_string(),
                                "priority" => b.gtk_priority = val.to_string(),
                                _ => {}
                            }
                        } else {
                            b.reading_multiline_str = true;
                            b.multiline_buf = after_q.to_string();
                        }
                    }
                }

                if line == "   ]" {
                    flush_builder(&mut mode, &mut b, hub);
                }
            }
        }
    }
}

fn flush_builder(mode: &mut CaptureMode, b: &mut RawNotifyBuilder, hub: &NotificationHub) {
    match *mode {
        CaptureMode::Idle => {}
        CaptureMode::FreedesktopNotify => {
            if !b.is_forwarded_shell_copy && b.top_strings.len() >= 4 {
                let raw_app = &b.top_strings[0];
                let raw_icon = &b.top_strings[1];
                let summary = &b.top_strings[2];
                let body = &b.top_strings[3];

                let app_name = if !raw_app.trim().is_empty() && raw_app != "notify-send" {
                    raw_app.clone()
                } else if !b.hint_desktop_entry.is_empty() {
                    b.hint_desktop_entry.clone()
                } else if !raw_app.trim().is_empty() {
                    "Notifikasi Sistem".to_string()
                } else {
                    "Aplikasi Linux".to_string()
                };

                let icon = if !raw_icon.is_empty() {
                    raw_icon.clone()
                } else {
                    b.hint_icon.clone()
                };

                hub.push_notification(
                    &app_name,
                    &icon,
                    summary,
                    body,
                    b.hint_urgency,
                    &b.hint_category,
                );
            }
        }
        CaptureMode::GtkAddNotification => {
            let app_id = b.top_strings.first().cloned().unwrap_or_default();
            if !b.gtk_title.is_empty() || !b.gtk_body.is_empty() {
                let urgency = match b.gtk_priority.as_str() {
                    "urgent" | "high" => 2,
                    "low" => 0,
                    _ => 1,
                };
                hub.push_notification(&app_id, "", &b.gtk_title, &b.gtk_body, urgency, "gtk");
            }
        }
    }
    *mode = CaptureMode::Idle;
    b.reset();
}

fn extract_quoted(s: &str) -> Option<String> {
    let start = s.find('"')?;
    let rest = &s[start + 1..];
    let end = rest.rfind('"')?;
    Some(rest[..end].to_string())
}

fn clean_app_title(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "Sistem".to_string();
    }
    // Convert Reverse-DNS app IDs like `org.gnome.Nautilus` -> `Nautilus`
    let base = if trimmed.contains('.') && !trimmed.contains(' ') {
        trimmed.rsplit('.').next().unwrap_or(trimmed)
    } else {
        trimmed
    };
    let normalized = base.replace(['-', '_'], " ");
    let mut c = normalized.chars();
    match c.next() {
        None => "Sistem".to_string(),
        Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
    }
}

pub fn strip_html_and_trim(input: &str) -> String {
    let mut replaced = input
        .replace("<br>", " • ")
        .replace("<br/>", " • ")
        .replace("<br />", " • ")
        .replace("\r\n", " ")
        .replace('\n', " ");

    // Strip any `<...>` tags
    let mut out = String::with_capacity(replaced.len());
    let mut in_tag = false;
    for ch in replaced.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }

    replaced = out
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ");

    // Collapse multiple spaces
    let parts: Vec<&str> = replaced.split_whitespace().collect();
    parts.join(" ")
}

/// Configures GNOME Shell's built-in notification banner popup (`org.gnome.desktop.notifications show-banners`)
/// so that when Arwiland handles notifications inside the Dynamic Island Notch, GNOME doesn't draw a duplicate popup on top.
pub fn set_system_notification_banners_hidden(hide_banners: bool) {
    let val = if hide_banners { "false" } else { "true" };
    thread::spawn(move || {
        let _ = Command::new("gsettings")
            .args([
                "set",
                "org.gnome.desktop.notifications",
                "show-banners",
                val,
            ])
            .status();
    });
}
