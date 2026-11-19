use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
#[cfg(target_os = "linux")]
use std::io::{BufRead, BufReader};
#[cfg(target_os = "linux")]
use std::os::unix::process::CommandExt;
#[cfg(target_os = "linux")]
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
#[cfg(any(target_os = "linux", target_os = "windows"))]
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

    #[allow(dead_code)]
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

    #[allow(dead_code)]
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

    #[cfg(target_os = "linux")]
    fn spawn_dbus_listener(&self) {
        let hub = self.clone();
        thread::spawn(move || loop {
            let mut cmd = Command::new("dbus-monitor");
            cmd.args([
                "--session",
                "type='method_call',interface='org.freedesktop.Notifications',member='Notify'",
                "type='method_call',interface='org.gtk.Notifications',member='AddNotification'",
                "type='method_call',interface='org.freedesktop.portal.Notification',member='AddNotification'",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
            unsafe {
                cmd.pre_exec(|| {
                    libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
                    Ok(())
                });
            }
            let child_res = cmd.spawn();

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

    #[cfg(target_os = "windows")]
    fn spawn_dbus_listener(&self) {
        let hub = self.clone();
        if win_has_package_identity() {
            thread::spawn(move || win_run_uwp_notification_listener(hub));
        } else {
            // Unpackaged Win32 .exe on Windows 10/11: watch WPN SQLite WAL/DB directly
            // (works without MSIX package identity and avoids ~10-15 MB of WinRT notification DLLs!)
            thread::spawn(move || win_run_wpndatabase_listener(hub));
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    fn spawn_dbus_listener(&self) {}
}

#[cfg(target_os = "windows")]
fn win_run_uwp_notification_listener(hub: NotificationHub) {
    use std::collections::HashSet;
    use windows::UI::Notifications::Management::{
        UserNotificationListener, UserNotificationListenerAccessStatus,
    };
    use windows::UI::Notifications::{KnownNotificationBindings, NotificationKinds};
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }

    let Ok(listener) = UserNotificationListener::Current() else {
        win_run_wpndatabase_listener(hub);
        return;
    };
    let Ok(access_op) = listener.RequestAccessAsync() else {
        win_run_wpndatabase_listener(hub);
        return;
    };
    let Ok(status) = access_op.get() else {
        win_run_wpndatabase_listener(hub);
        return;
    };
    if status != UserNotificationListenerAccessStatus::Allowed {
        win_run_wpndatabase_listener(hub);
        return;
    }

    let mut seen: HashSet<u32> = HashSet::new();
    let mut first_pass = true;
    let toast_binding = KnownNotificationBindings::ToastGeneric().ok();

    loop {
        if let Ok(op) = listener.GetNotificationsAsync(NotificationKinds::Toast) {
            if let Ok(list) = op.get() {
                let count = list.Size().unwrap_or(0);
                for i in 0..count {
                    let Ok(item) = list.GetAt(i) else { continue };
                    let Ok(uid) = item.Id() else { continue };
                    if !seen.insert(uid) {
                        continue;
                    }
                    if first_pass {
                        continue;
                    }
                    let app_name = item
                        .AppInfo()
                        .ok()
                        .and_then(|ai| ai.DisplayInfo().ok())
                        .and_then(|di| di.DisplayName().ok())
                        .map(|s| s.to_string_lossy())
                        .unwrap_or_else(|| "Windows".to_string());

                    let mut texts: Vec<String> = Vec::new();
                    if let (Ok(notif), Some(ref tb)) = (item.Notification(), &toast_binding) {
                        if let Ok(visual) = notif.Visual() {
                            if let Ok(binding) = visual.GetBinding(tb) {
                                if let Ok(elems) = binding.GetTextElements() {
                                    let ec = elems.Size().unwrap_or(0);
                                    for j in 0..ec {
                                        if let Ok(el) = elems.GetAt(j) {
                                            if let Ok(t) = el.Text() {
                                                let s = t.to_string_lossy();
                                                if !s.trim().is_empty() {
                                                    texts.push(s);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if !texts.is_empty() {
                        let summary = texts[0].clone();
                        let body = if texts.len() > 1 {
                            texts[1..].join(" ")
                        } else {
                            String::new()
                        };
                        hub.push_notification(&app_name, "", &summary, &body, 1, "windows");
                    }
                }
                first_pass = false;
            }
        }
        thread::sleep(Duration::from_millis(1200));
    }
}

#[cfg(target_os = "windows")]
fn win_now_filetime_ticks() -> i64 {
    const UNIX_EPOCH_FILETIME_TICKS: i64 = 116_444_736_000_000_000;
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    UNIX_EPOCH_FILETIME_TICKS
        + (d.as_secs() as i64) * 10_000_000
        + (d.subsec_nanos() as i64) / 100
}

#[cfg(target_os = "windows")]
fn fnv1a_64(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[cfg(target_os = "windows")]
fn win_read_shared_file(path: &std::path::Path, max_bytes: usize) -> Option<Vec<u8>> {
    use std::io::Read;
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_SHARE_ALL: u32 = 0x1 | 0x2 | 0x4;
    let f = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_ALL)
        .open(path)
        .ok()?;
    let meta_len = f.metadata().ok().map(|m| m.len() as usize).unwrap_or(0);
    if meta_len == 0 || meta_len > max_bytes {
        return None;
    }
    let mut buf = Vec::with_capacity(meta_len);
    f.take(max_bytes as u64).read_to_end(&mut buf).ok()?;
    Some(buf)
}

#[cfg(target_os = "windows")]
fn win_file_sig(path: &std::path::Path) -> Option<(u64, std::time::SystemTime)> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
    Some((meta.len(), mtime))
}

#[cfg(target_os = "windows")]
fn win_find_recent_arrival_time(buf: &[u8], toast_pos: usize, now_ft: i64) -> Option<i64> {
    let win_start = toast_pos.saturating_sub(96);
    if toast_pos < win_start + 8 {
        return None;
    }
    let min_ft = now_ft - 120 * 10_000_000;
    let max_ft = now_ft + 10 * 10_000_000;
    let slice = &buf[win_start..toast_pos];
    // Scan right-to-left (ArrivalTime sits directly before DataVersion + Payload in wpndatabase.db)
    for i in (0..=slice.len() - 8).rev() {
        let val = i64::from_be_bytes([
            slice[i],
            slice[i + 1],
            slice[i + 2],
            slice[i + 3],
            slice[i + 4],
            slice[i + 5],
            slice[i + 6],
            slice[i + 7],
        ]);
        if val >= min_ft && val <= max_ft {
            return Some(val);
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn win_infer_toast_app(xml: &str, attribution: Option<&str>) -> String {
    let lower_all = xml.to_lowercase();
    let check_hint = |s: &str| -> Option<&'static str> {
        if s.contains("whatsapp") {
            Some("WhatsApp")
        } else if s.contains("telegram") {
            Some("Telegram")
        } else if s.contains("discord") {
            Some("Discord")
        } else if s.contains("chrome") {
            Some("Google Chrome")
        } else if s.contains("msedge") || s.contains("microsoft.edge") {
            Some("Microsoft Edge")
        } else if s.contains("firefox") {
            Some("Firefox")
        } else if s.contains("spotify") {
            Some("Spotify")
        } else if s.contains("teams") {
            Some("Microsoft Teams")
        } else if s.contains("outlook") || s.contains("hxmail") {
            Some("Outlook")
        } else if s.contains("slack") {
            Some("Slack")
        } else if s.contains("sechealth") || s.contains("defender") {
            Some("Keamanan Windows")
        } else if s.contains("windowsupdate") {
            Some("Windows Update")
        } else if s.contains("steam") {
            Some("Steam")
        } else if s.contains("instagram") {
            Some("Instagram")
        } else {
            None
        }
    };

    if let Some(attr) = attribution {
        let attr_l = attr.to_lowercase();
        if let Some(known) = check_hint(&attr_l) {
            return known.to_string();
        }
        let cleaned = attr
            .trim()
            .trim_start_matches("Via ")
            .trim_start_matches("via ")
            .trim();
        if !cleaned.is_empty() && cleaned.len() <= 32 {
            return cleaned.to_string();
        }
    }
    if let Some(known) = check_hint(&lower_all) {
        return known.to_string();
    }
    "Windows".to_string()
}

#[cfg(target_os = "windows")]
fn win_parse_toast_xml(xml: &str) -> Option<(String, String, String)> {
    let mut texts: Vec<String> = Vec::new();
    let mut attribution: Option<String> = None;
    let mut rem = xml;

    while let Some(pos) = rem.find("<text") {
        let after = &rem[pos + 5..];
        // Must be `<text>` or `<text ...>` (not `<textbox>`)
        let next_b = after.as_bytes().first().copied().unwrap_or(0);
        if next_b != b'>' && next_b != b' ' && next_b != b'\t' && next_b != b'\r' && next_b != b'\n'
        {
            rem = after;
            continue;
        }
        let Some(gt) = after.find('>') else { break };
        let header = &after[..gt];
        let body_start = &after[gt + 1..];
        if header.ends_with('/') {
            rem = body_start;
            continue;
        }
        let Some(close_pos) = body_start.find("</text>") else {
            break;
        };
        let raw_content = &body_start[..close_pos];
        rem = &body_start[close_pos + 7..];

        let cleaned = strip_html_and_trim(raw_content);
        if cleaned.is_empty() {
            continue;
        }
        let is_attr = header.contains("placement=\"attribution\"")
            || header.contains("placement='attribution'");
        if is_attr {
            attribution = Some(cleaned);
        } else {
            texts.push(cleaned);
        }
    }

    if texts.is_empty() {
        return None;
    }
    let app_name = win_infer_toast_app(xml, attribution.as_deref());
    let summary = texts[0].clone();
    let body = if texts.len() > 1 {
        texts[1..].join(" ")
    } else {
        String::new()
    };
    Some((app_name, summary, body))
}

#[cfg(target_os = "windows")]
fn win_scan_buffer_for_toasts(
    buf: &[u8],
    startup_ft: i64,
    now_ft: i64,
    seed_only: bool,
    seen: &mut std::collections::HashSet<u64>,
    hub: &NotificationHub,
) {
    let needle = b"<toast";
    let end_needle = b"</toast>";
    let mut idx = 0usize;

    while idx + needle.len() < buf.len() {
        let Some(rel) = buf[idx..].windows(needle.len()).position(|w| w == needle) else {
            break;
        };
        let start = idx + rel;
        let after_start = start + needle.len();
        let next_b = buf.get(after_start).copied().unwrap_or(0);
        if next_b != b'>' && next_b != b' ' && next_b != b'\t' && next_b != b'\r' && next_b != b'\n'
        {
            idx = after_start;
            continue;
        }
        let search_limit = (start + 16384).min(buf.len());
        let Some(end_rel) = buf[after_start..search_limit]
            .windows(end_needle.len())
            .position(|w| w == end_needle)
        else {
            idx = after_start;
            continue;
        };
        let end = after_start + end_rel + end_needle.len();
        let xml_bytes = &buf[start..end];
        idx = end;

        let arrival_opt = win_find_recent_arrival_time(buf, start, now_ft);
        let xml_hash = fnv1a_64(xml_bytes);
        let key = if let Some(arr) = arrival_opt {
            xml_hash ^ (arr as u64).rotate_left(17)
        } else {
            xml_hash
        };

        if seed_only {
            seen.insert(xml_hash);
            seen.insert(key);
            continue;
        }

        // If ArrivalTime is clearly from before startup, ignore (rewritten page during Action Center dismissal)
        if let Some(arr) = arrival_opt {
            if arr < startup_ft - 3 * 10_000_000 {
                seen.insert(key);
                continue;
            }
        } else if seen.contains(&xml_hash) {
            continue;
        }

        if !seen.insert(key) {
            continue;
        }
        if arrival_opt.is_none() {
            seen.insert(xml_hash);
        }

        let xml_str = String::from_utf8_lossy(xml_bytes);
        if let Some((app, summary, body)) = win_parse_toast_xml(&xml_str) {
            hub.push_notification(&app, "", &summary, &body, 1, "windows");
        }
    }
}

#[cfg(target_os = "windows")]
fn win_scan_wal_frames(
    wal: &[u8],
    start_frame: usize,
    startup_ft: i64,
    now_ft: i64,
    seed_only: bool,
    seen: &mut std::collections::HashSet<u64>,
    hub: &NotificationHub,
) -> (usize, [u8; 8]) {
    if wal.len() < 32 {
        return (0, [0u8; 8]);
    }
    let magic = u32::from_be_bytes([wal[0], wal[1], wal[2], wal[3]]);
    if magic != 0x377F_0682 && magic != 0x377F_0683 {
        // Not a standard WAL header; scan raw slice safely
        win_scan_buffer_for_toasts(wal, startup_ft, now_ft, seed_only, seen, hub);
        return (0, [0u8; 8]);
    }
    let page_size = u32::from_be_bytes([wal[8], wal[9], wal[10], wal[11]]) as usize;
    if !(512..=65536).contains(&page_size) || !page_size.is_power_of_two() {
        return (0, [0u8; 8]);
    }
    let mut salt = [0u8; 8];
    salt.copy_from_slice(&wal[16..24]);

    let frame_stride = 24 + page_size;
    let total_frames = (wal.len() - 32) / frame_stride;
    let mut valid_frames = 0usize;

    for fi in 0..total_frames {
        let f_off = 32 + fi * frame_stride;
        if wal[f_off + 8..f_off + 16] != salt {
            break;
        }
        valid_frames = fi + 1;
        if fi < start_frame {
            continue;
        }
        let payload = &wal[f_off + 24..f_off + frame_stride];
        win_scan_buffer_for_toasts(payload, startup_ft, now_ft, seed_only, seen, hub);
    }

    (valid_frames, salt)
}

#[cfg(target_os = "windows")]
fn win_read_shm_header(path: &std::path::Path) -> Option<[u8; 48]> {
    use std::io::Read;
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_SHARE_ALL: u32 = 0x1 | 0x2 | 0x4;
    let mut f = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_ALL)
        .open(path)
        .ok()?;
    let mut hdr = [0u8; 48];
    f.read_exact(&mut hdr).ok()?;
    Some(hdr)
}

#[cfg(target_os = "windows")]
fn win_run_wpndatabase_listener(hub: NotificationHub) {
    use std::collections::HashSet;
    use std::path::PathBuf;

    let Ok(local_app_data) = std::env::var("LOCALAPPDATA") else {
        return;
    };
    let notif_dir = PathBuf::from(local_app_data)
        .join("Microsoft")
        .join("Windows")
        .join("Notifications");
    let db_path = notif_dir.join("wpndatabase.db");
    let wal_path = notif_dir.join("wpndatabase.db-wal");
    let shm_path = notif_dir.join("wpndatabase.db-shm");

    let startup_ft = win_now_filetime_ticks();
    let mut seen: HashSet<u64> = HashSet::with_capacity(256);
    let mut last_shm_hdr = win_read_shm_header(&shm_path);
    let mut last_wal_sig = win_file_sig(&wal_path);
    let mut last_db_sig = win_file_sig(&db_path);
    let mut last_wal_salt = [0u8; 8];
    let mut last_wal_frames = 0usize;

    // Seed existing notifications so old Action Center history isn't replayed on startup
    if let Some(db_bytes) = win_read_shared_file(&db_path, 32 * 1024 * 1024) {
        win_scan_buffer_for_toasts(&db_bytes, startup_ft, startup_ft, true, &mut seen, &hub);
    }
    if let Some(wal_bytes) = win_read_shared_file(&wal_path, 16 * 1024 * 1024) {
        let (frames, salt) =
            win_scan_wal_frames(&wal_bytes, 0, startup_ft, startup_ft, true, &mut seen, &hub);
        last_wal_frames = frames;
        last_wal_salt = salt;
    }

    loop {
        thread::sleep(Duration::from_millis(450));

        // Reading 48 bytes from wpndatabase.db-shm catches EVERY SQLite WAL commit on Windows 10
        // immediately (even when NTFS delays updating LastWriteTime/length on an open handle!)
        let cur_shm_hdr = win_read_shm_header(&shm_path);
        let cur_wal_sig = win_file_sig(&wal_path);
        let cur_db_sig = win_file_sig(&db_path);
        let shm_changed = cur_shm_hdr != last_shm_hdr;
        let wal_changed = shm_changed || cur_wal_sig != last_wal_sig;
        let db_changed = cur_db_sig != last_db_sig;

        if !wal_changed && !db_changed {
            continue;
        }

        last_shm_hdr = cur_shm_hdr;
        let now_ft = win_now_filetime_ticks();
        let mut scanned_wal = false;

        if wal_changed {
            last_wal_sig = cur_wal_sig;
            if let Some(wal_bytes) = win_read_shared_file(&wal_path, 16 * 1024 * 1024) {
                let same_salt = wal_bytes.len() >= 24 && wal_bytes[16..24] == last_wal_salt;
                let start_frame = if same_salt { last_wal_frames } else { 0 };
                let (frames, salt) = win_scan_wal_frames(
                    &wal_bytes,
                    start_frame,
                    startup_ft,
                    now_ft,
                    false,
                    &mut seen,
                    &hub,
                );
                if frames > 0 {
                    scanned_wal = true;
                }
                last_wal_frames = frames;
                last_wal_salt = salt;
            } else {
                last_wal_frames = 0;
                last_wal_salt = [0u8; 8];
            }
        }

        if db_changed {
            last_db_sig = cur_db_sig;
            if !scanned_wal {
                if let Some(db_bytes) = win_read_shared_file(&db_path, 32 * 1024 * 1024) {
                    win_scan_buffer_for_toasts(&db_bytes, startup_ft, now_ft, false, &mut seen, &hub);
                }
            }
        }

        if seen.len() > 2048 {
            seen.clear();
        }
    }
}

#[cfg(target_os = "windows")]
fn win_has_package_identity() -> bool {
    use windows::core::{s, w};
    use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};

    type GetCurrentPackageFullNameFn = unsafe extern "system" fn(*mut u32, *mut u16) -> i32;
    const APPMODEL_ERROR_NO_PACKAGE: i32 = 15700;

    unsafe {
        let Ok(hmod) = GetModuleHandleW(w!("kernel32.dll")) else {
            return false;
        };
        let Some(proc_addr) = GetProcAddress(hmod, s!("GetCurrentPackageFullName")) else {
            return false;
        };
        let func: GetCurrentPackageFullNameFn = std::mem::transmute(proc_addr);
        let mut len: u32 = 0;
        let rc = func(&mut len, std::ptr::null_mut());
        rc != APPMODEL_ERROR_NO_PACKAGE
    }
}

#[cfg(target_os = "linux")]
#[derive(PartialEq)]
enum CaptureMode {
    Idle,
    FreedesktopNotify,
    GtkAddNotification,
}

#[cfg(target_os = "linux")]
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

#[cfg(target_os = "linux")]
impl RawNotifyBuilder {
    fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(target_os = "linux")]
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

#[cfg(target_os = "linux")]
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

#[cfg(target_os = "linux")]
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
#[allow(dead_code)]
pub fn set_system_notification_banners_hidden(hide_banners: bool) {
    #[cfg(target_os = "linux")]
    {
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
    #[cfg(not(target_os = "linux"))]
    {
        let _ = hide_banners;
    }
}
