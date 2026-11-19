use std::sync::Arc;

use crate::album_art::AlbumArt;

#[cfg(target_os = "macos")]
use std::process::Command;

#[cfg(target_os = "windows")]
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession, GlobalSystemMediaTransportControlsSessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus,
};
#[cfg(target_os = "windows")]
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

#[cfg(target_os = "linux")]
use std::collections::HashMap;
#[cfg(target_os = "linux")]
use std::sync::Mutex;
#[cfg(target_os = "linux")]
use zbus::blocking::Connection;
#[cfg(target_os = "linux")]
use zbus::zvariant::{ObjectPath, OwnedValue, Value};

#[cfg(target_os = "linux")]
static SESSION_BUS: Mutex<Option<Connection>> = Mutex::new(None);

#[cfg(target_os = "linux")]
fn get_session_bus() -> Option<Connection> {
    let mut guard = SESSION_BUS.lock().ok()?;
    if let Some(ref conn) = *guard {
        return Some(conn.clone());
    }
    match Connection::session() {
        Ok(conn) => {
            *guard = Some(conn.clone());
            Some(conn)
        }
        Err(_) => None,
    }
}

#[cfg(target_os = "linux")]
fn reset_session_bus() {
    if let Ok(mut guard) = SESSION_BUS.lock() {
        *guard = None;
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct MediaStatus {
    pub active_bus: Option<String>,
    pub track_id: Option<String>,
    pub has_player: bool,
    pub player_name: String,
    pub title: String,
    pub artist: String,
    pub art_path: Option<String>,
    pub art_image: Option<Arc<AlbumArt>>,
    pub playing: bool,
    pub playback_rate: f64,
    pub timeline_live: bool,
    pub position_secs: u64,
    pub position_f64: f64,
    pub length_secs: u64,
}

impl Default for MediaStatus {
    fn default() -> Self {
        Self {
            active_bus: None,
            track_id: None,
            has_player: false,
            player_name: String::new(),
            title: String::new(),
            artist: String::new(),
            art_path: None,
            art_image: None,
            playing: false,
            playback_rate: 1.0,
            timeline_live: false,
            position_secs: 0,
            position_f64: 0.0,
            length_secs: 0,
        }
    }
}

#[cfg(target_os = "linux")]
fn value_as_string(v: &Value<'_>) -> Option<String> {
    match v {
        Value::Str(s) => {
            let trimmed = s.as_str().trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Value::ObjectPath(p) => {
            let trimmed = p.as_str().trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Value::Array(arr) => {
            let mut parts = Vec::new();
            for item in arr.iter() {
                if let Some(s) = value_as_string(item) {
                    parts.push(s);
                }
            }
            if parts.is_empty() {
                None
            } else {
                Some(parts.join(", "))
            }
        }
        Value::Value(inner) => value_as_string(inner),
        _ => None,
    }
}

#[cfg(target_os = "linux")]
fn value_as_i64(v: &Value<'_>) -> Option<i64> {
    match v {
        Value::I64(n) => Some(*n),
        Value::U64(n) => Some(*n as i64),
        Value::I32(n) => Some(*n as i64),
        Value::U32(n) => Some(*n as i64),
        Value::Value(inner) => value_as_i64(inner),
        _ => None,
    }
}

#[cfg(target_os = "linux")]
fn format_player_name(bus: &str) -> String {
    let raw = bus
        .trim_start_matches("org.mpris.MediaPlayer2.")
        .split('.')
        .next()
        .unwrap_or("Media");
    match raw.to_lowercase().as_str() {
        "chromium" | "chrome" => "Google Chrome".to_string(),
        "firefox" => "Firefox".to_string(),
        "spotify" => "Spotify".to_string(),
        "vlc" => "VLC Media".to_string(),
        other => {
            let mut c = other.chars();
            match c.next() {
                None => "Media".to_string(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn win_ensure_com() {
    thread_local! {
        static INIT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }
    INIT.with(|done| {
        if !done.get() {
            unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            }
            done.set(true);
        }
    });
}

#[cfg(target_os = "windows")]
fn win_format_app_name(aumid: &str) -> String {
    let low = aumid.to_lowercase();
    if low.contains("spotify") {
        "Spotify".to_string()
    } else if low.contains("chrome") {
        "Google Chrome".to_string()
    } else if low.contains("msedge") || low.contains("edge") {
        "Microsoft Edge".to_string()
    } else if low.contains("firefox") {
        "Firefox".to_string()
    } else if low.contains("vlc") {
        "VLC Media".to_string()
    } else if low.contains("brave") {
        "Brave".to_string()
    } else if low.contains("opera") {
        "Opera".to_string()
    } else if low.contains("zune") || low.contains("mediaplayer") {
        "Media Player".to_string()
    } else {
        let base = aumid
            .rsplit('\\')
            .next()
            .unwrap_or(aumid)
            .split('!')
            .next()
            .unwrap_or("Media")
            .trim_end_matches(".exe");
        let mut c = base.chars();
        match c.next() {
            None => "Media".to_string(),
            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        }
    }
}

#[cfg(target_os = "windows")]
thread_local! {
    static WIN_SMTC_MGR: std::cell::RefCell<Option<GlobalSystemMediaTransportControlsSessionManager>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(target_os = "windows")]
fn win_get_session_manager() -> Option<GlobalSystemMediaTransportControlsSessionManager> {
    win_ensure_com();
    WIN_SMTC_MGR.with(|cell| {
        if let Some(ref mgr) = *cell.borrow() {
            return Some(mgr.clone());
        }
        let mgr = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
            .ok()?
            .get()
            .ok()?;
        *cell.borrow_mut() = Some(mgr.clone());
        Some(mgr)
    })
}

#[cfg(target_os = "windows")]
fn win_reset_session_manager() {
    WIN_SMTC_MGR.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

#[cfg(target_os = "windows")]
fn win_pick_session() -> Option<GlobalSystemMediaTransportControlsSession> {
    let pick_from = |mgr: &GlobalSystemMediaTransportControlsSessionManager| -> Result<Option<GlobalSystemMediaTransportControlsSession>, ()> {
        let list = mgr.GetSessions().map_err(|_| ())?;
        let mut fallback = mgr.GetCurrentSession().ok();
        let count = list.Size().unwrap_or(0);
        for i in 0..count {
            if let Ok(sess) = list.GetAt(i) {
                let is_playing = sess
                    .GetPlaybackInfo()
                    .ok()
                    .and_then(|pi| pi.PlaybackStatus().ok())
                    .map(|st| st == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing)
                    .unwrap_or(false);
                if is_playing {
                    return Ok(Some(sess));
                }
                if fallback.is_none() {
                    fallback = Some(sess);
                }
            }
        }
        Ok(fallback)
    };

    if let Some(mgr) = win_get_session_manager() {
        if let Ok(res) = pick_from(&mgr) {
            return res;
        }
        // Manager proxy became stale (e.g. SMTC service restart); refresh once
        win_reset_session_manager();
        if let Some(fresh) = win_get_session_manager() {
            if let Ok(res) = pick_from(&fresh) {
                return res;
            }
        }
    }
    None
}

#[allow(dead_code)]
pub fn find_all_mpris_buses() -> Vec<String> {
    #[cfg(target_os = "linux")]
    {
        let Some(conn) = get_session_bus() else {
            return Vec::new();
        };
        let reply = match conn.call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "ListNames",
            &(),
        ) {
            Ok(r) => r,
            Err(_) => {
                reset_session_bus();
                return Vec::new();
            }
        };
        let names: Vec<String> = reply.body().deserialize().unwrap_or_default();
        names
            .into_iter()
            .filter(|name| name.starts_with("org.mpris.MediaPlayer2."))
            .collect()
    }
    #[cfg(not(target_os = "linux"))]
    {
        Vec::new()
    }
}

#[cfg(target_os = "linux")]
fn decode_file_uri_path(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = &raw[i + 1..i + 3];
            if let Ok(b) = u8::from_str_radix(hex, 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

#[cfg(target_os = "windows")]
struct WinThumbCache {
    track_key: String,
    art: Option<Arc<AlbumArt>>,
    attempts: u8,
}

#[cfg(target_os = "windows")]
thread_local! {
    static WIN_THUMB_CACHE: std::cell::RefCell<WinThumbCache> = const {
        std::cell::RefCell::new(WinThumbCache {
            track_key: String::new(),
            art: None,
            attempts: 0,
        })
    };
}

#[cfg(target_os = "windows")]
fn win_read_thumbnail_art(
    props: &windows::Media::Control::GlobalSystemMediaTransportControlsSessionMediaProperties,
) -> Option<Arc<AlbumArt>> {
    use windows::Storage::Streams::DataReader;

    let thumb_ref = props.Thumbnail().ok()?;
    let stream = thumb_ref.OpenReadAsync().ok()?.get().ok()?;
    let size = stream.Size().ok()?;
    if size == 0 || size > 6 * 1024 * 1024 {
        return None;
    }
    let reader = DataReader::CreateDataReader(&stream).ok()?;
    let loaded = reader.LoadAsync(size as u32).ok()?.get().ok()?;
    if loaded == 0 {
        return None;
    }
    let mut buf = vec![0u8; loaded as usize];
    reader.ReadBytes(&mut buf).ok()?;
    crate::album_art::decode_to_squircle(&buf)
}

#[cfg(target_os = "windows")]
fn win_cached_thumbnail_art(
    track_key: &str,
    props: &windows::Media::Control::GlobalSystemMediaTransportControlsSessionMediaProperties,
) -> Option<Arc<AlbumArt>> {
    WIN_THUMB_CACHE.with(|cell| {
        let mut cache = cell.borrow_mut();
        if cache.track_key != track_key {
            cache.track_key.clear();
            cache.track_key.push_str(track_key);
            cache.art = None;
            cache.attempts = 0;
        }
        if let Some(ref art) = cache.art {
            return Some(Arc::clone(art));
        }
        // Retry up to 3 times on track change because Spotify/Chrome often populate Thumbnail ~500ms after Title
        if cache.attempts < 3 {
            cache.attempts += 1;
            if let Some(decoded) = win_read_thumbnail_art(props) {
                cache.art = Some(Arc::clone(&decoded));
                return Some(decoded);
            }
        }
        None
    })
}

#[cfg(target_os = "linux")]
fn get_player_properties(
    conn: &Connection,
    bus: &str,
) -> Option<HashMap<String, OwnedValue>> {
    let reply = conn
        .call_method(
            Some(bus),
            "/org/mpris/MediaPlayer2",
            Some("org.freedesktop.DBus.Properties"),
            "GetAll",
            &("org.mpris.MediaPlayer2.Player",),
        )
        .ok()?;
    reply.body().deserialize().ok()
}

pub fn query_media_status() -> MediaStatus {
    #[cfg(target_os = "linux")]
    {
        if let Some(conn) = get_session_bus() {
            let buses = find_all_mpris_buses();
            let mut fallback: Option<MediaStatus> = None;

            for bus in buses {
                let Some(props) = get_player_properties(&conn, &bus) else {
                    continue;
                };

                let playback = props
                    .get("PlaybackStatus")
                    .and_then(|v| value_as_string(v))
                    .unwrap_or_default();
                let playing = playback.eq_ignore_ascii_case("Playing");
                let player_name = format_player_name(&bus);

                let mut title = None;
                let mut artist = None;
                let mut art_url = None;
                let mut len_us: u64 = 0;
                let mut track_id = None;

                if let Some(meta_owned) = props.get("Metadata") {
                    let meta_val: &Value<'_> = meta_owned;
                    let dict_opt = match meta_val {
                        Value::Dict(d) => Some(d),
                        Value::Value(inner) => match inner.as_ref() {
                            Value::Dict(d) => Some(d),
                            _ => None,
                        },
                        _ => None,
                    };
                    if let Some(dict) = dict_opt {
                        for (k, v) in dict.iter() {
                            if let Some(key) = value_as_string(k) {
                                match key.as_str() {
                                    "xesam:title" => title = value_as_string(v),
                                    "xesam:artist" => artist = value_as_string(v),
                                    "mpris:artUrl" => art_url = value_as_string(v),
                                    "mpris:length" => {
                                        len_us = value_as_i64(v).unwrap_or(0).max(0) as u64;
                                    }
                                    "mpris:trackid" => track_id = value_as_string(v),
                                    _ => {}
                                }
                            }
                        }
                    }
                }

                let title = title.unwrap_or_else(|| "Media Aktif".to_string());
                let artist = artist.unwrap_or_else(|| player_name.clone());
                let art_path = art_url.and_then(|u| {
                    let trimmed = u.trim();
                    if let Some(stripped) = trimmed.strip_prefix("file://") {
                        let decoded = decode_file_uri_path(stripped);
                        if std::path::Path::new(&decoded).exists() {
                            return Some(decoded);
                        }
                    } else if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
                        return Some(trimmed.to_string());
                    }
                    None
                });

                let pos_us = props
                    .get("Position")
                    .and_then(|v| value_as_i64(v))
                    .unwrap_or(0)
                    .max(0) as u64;

                let status = MediaStatus {
                    active_bus: Some(bus),
                    track_id,
                    has_player: true,
                    player_name,
                    title,
                    artist,
                    art_path,
                    art_image: None,
                    playing,
                    playback_rate: 1.0,
                    timeline_live: true,
                    position_secs: pos_us / 1_000_000,
                    position_f64: pos_us as f64 / 1_000_000.0,
                    length_secs: len_us / 1_000_000,
                };

                if playing {
                    return status;
                }
                if fallback.is_none() {
                    fallback = Some(status);
                }
            }

            if let Some(fb) = fallback {
                return fb;
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        let script = r#"
            tell application "System Events"
                set spRunning to (name of processes) contains "Spotify"
            end tell
            if spRunning then
                tell application "Spotify"
                    set pState to player state as string
                    if pState is "playing" or pState is "paused" then
                        set tName to name of current track
                        set tArtist to artist of current track
                        set tPos to player position
                        set tDur to (duration of current track) / 1000
                        set tArt to artwork url of current track
                        return "Spotify|" & pState & "|" & tName & "|" & tArtist & "|" & (tPos as string) & "|" & (tDur as string) & "|" & tArt
                    end if
                end tell
            end if
            return ""
        "#;
        if let Ok(out) = Command::new("osascript").args(["-e", script]).output() {
            let txt = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let parts: Vec<&str> = txt.split('|').collect();
            if parts.len() >= 6 {
                let playing = parts[1].eq_ignore_ascii_case("playing");
                let pos_f = parts[4].replace(',', ".").parse::<f64>().unwrap_or(0.0);
                let len_f = parts[5].replace(',', ".").parse::<f64>().unwrap_or(0.0);
                let art_path = parts
                    .get(6)
                    .map(|s| s.trim())
                    .filter(|s| s.starts_with("http://") || s.starts_with("https://"))
                    .map(|s| s.to_string());
                return MediaStatus {
                    active_bus: Some(parts[0].to_string()),
                    track_id: None,
                    has_player: true,
                    player_name: parts[0].to_string(),
                    title: parts[2].to_string(),
                    artist: parts[3].to_string(),
                    art_path,
                    art_image: None,
                    playing,
                    playback_rate: 1.0,
                    timeline_live: true,
                    position_secs: pos_f.max(0.0) as u64,
                    position_f64: pos_f.max(0.0),
                    length_secs: len_f.max(0.0) as u64,
                };
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(sess) = win_pick_session() {
            let aumid = sess
                .SourceAppUserModelId()
                .map(|s| s.to_string_lossy())
                .unwrap_or_else(|_| "Media".to_string());
            let player_name = win_format_app_name(&aumid);
            let playback_info = sess.GetPlaybackInfo().ok();
            let playing = playback_info
                .as_ref()
                .and_then(|pi| pi.PlaybackStatus().ok())
                .map(|st| st == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing)
                .unwrap_or(false);
            let playback_rate = playback_info
                .as_ref()
                .and_then(|pi| pi.PlaybackRate().ok())
                .and_then(|r| r.Value().ok())
                .filter(|&r| r.is_finite() && r > 0.05 && r <= 4.0)
                .unwrap_or(1.0);

            let (title, artist, art_image) = sess
                .TryGetMediaPropertiesAsync()
                .ok()
                .and_then(|op| op.get().ok())
                .map(|props| {
                    let t = props
                        .Title()
                        .map(|s| s.to_string_lossy().trim().to_string())
                        .unwrap_or_default();
                    let a = props
                        .Artist()
                        .map(|s| s.to_string_lossy().trim().to_string())
                        .unwrap_or_default();
                    let final_t = if t.is_empty() {
                        "Media Aktif".to_string()
                    } else {
                        t
                    };
                    let final_a = if a.is_empty() {
                        player_name.clone()
                    } else {
                        a
                    };
                    let tkey = format!("{}||{}||{}", aumid, final_t, final_a);
                    let thumb = win_cached_thumbnail_art(&tkey, &props);
                    (final_t, final_a, thumb)
                })
                .unwrap_or_else(|| ("Media Aktif".to_string(), player_name.clone(), None));

            const UNIX_EPOCH_FILETIME_TICKS: i64 = 116_444_736_000_000_000;
            let (pos_f64, len_secs, timeline_live) = sess
                .GetTimelineProperties()
                .ok()
                .map(|tl| {
                    let start_ticks = tl.StartTime().map(|d| d.Duration).unwrap_or(0).max(0);
                    let pos_ticks = tl.Position().map(|d| d.Duration).unwrap_or(0).max(0);
                    let end_ticks = tl.EndTime().map(|d| d.Duration).unwrap_or(0).max(0);
                    let mut pos = (pos_ticks - start_ticks).max(0) as f64 / 10_000_000.0;
                    let len = (end_ticks - start_ticks).max(0) as f64 / 10_000_000.0;
                    let last_upd_ticks = tl
                        .LastUpdatedTime()
                        .map(|t| t.UniversalTime)
                        .unwrap_or(0);
                    let has_upd = last_upd_ticks > UNIX_EPOCH_FILETIME_TICKS;
                    if playing && has_upd {
                        // Windows SMTC only refreshes Position/LastUpdatedTime on state changes
                        // (play/pause/seek/track change), never every second; do NOT cap delta at 30s!
                        let now_unix = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default();
                        let now_ticks = UNIX_EPOCH_FILETIME_TICKS
                            + (now_unix.as_secs() as i64) * 10_000_000
                            + (now_unix.subsec_nanos() as i64) / 100;
                        let raw_delta_s =
                            ((now_ticks - last_upd_ticks).max(0) as f64) / 10_000_000.0;
                        let max_delta = if len > 0.5 { len } else { 21_600.0 };
                        pos += (raw_delta_s * playback_rate).min(max_delta);
                        if len > 0.5 {
                            pos = pos.min(len);
                        }
                    }
                    let live = has_upd || pos_ticks > start_ticks;
                    (pos.max(0.0), len.round() as u64, live)
                })
                .unwrap_or((0.0, 0, false));

            return MediaStatus {
                active_bus: Some(aumid),
                track_id: None,
                has_player: true,
                player_name,
                title,
                artist,
                art_path: None,
                art_image,
                playing,
                playback_rate,
                timeline_live,
                position_secs: pos_f64 as u64,
                position_f64: pos_f64,
                length_secs: len_secs,
            };
        }
    }

    MediaStatus {
        active_bus: None,
        track_id: None,
        has_player: false,
        player_name: "Tidak ada pemutar".to_string(),
        title: "Tidak ada media diputar".to_string(),
        artist: "Putar musik di Spotify, Chrome, atau VLC".to_string(),
        art_path: None,
        art_image: None,
        playing: false,
        playback_rate: 1.0,
        timeline_live: false,
        position_secs: 0,
        position_f64: 0.0,
        length_secs: 0,
    }
}

pub fn seek_media_to(
    cached_bus: Option<&str>,
    cached_track_id: Option<&str>,
    target_secs: f64,
) -> bool {
    #[cfg(target_os = "linux")]
    {
        let Some(conn) = get_session_bus() else {
            return false;
        };
        let target_us = (target_secs.max(0.0) * 1_000_000.0).round() as i64;

        let call_set_position = |bus: &str, tid: &str| -> bool {
            let Ok(obj_path) = ObjectPath::try_from(tid) else {
                return false;
            };
            conn.call_method(
                Some(bus),
                "/org/mpris/MediaPlayer2",
                Some("org.mpris.MediaPlayer2.Player"),
                "SetPosition",
                &(obj_path, target_us),
            )
            .is_ok()
        };

        if let Some(bus) = cached_bus {
            let default_tid = if bus.to_lowercase().contains("firefox") {
                "/org/mpris/MediaPlayer2/firefox"
            } else {
                "/org/mpris/MediaPlayer2/TrackList/NoTrack"
            };
            let tid = cached_track_id
                .filter(|t| t.starts_with('/'))
                .unwrap_or(default_tid);
            if call_set_position(bus, tid) {
                return true;
            }
        }

        let status = query_media_status();
        let Some(bus) = status
            .active_bus
            .or_else(|| cached_bus.map(|s| s.to_string()))
            .or_else(|| find_all_mpris_buses().into_iter().next())
        else {
            return false;
        };

        let default_tid = if bus.to_lowercase().contains("firefox") {
            "/org/mpris/MediaPlayer2/firefox"
        } else {
            "/org/mpris/MediaPlayer2/TrackList/NoTrack"
        };
        let tid = status
            .track_id
            .as_deref()
            .or(cached_track_id)
            .filter(|t| t.starts_with('/'))
            .unwrap_or(default_tid);

        if call_set_position(&bus, tid) {
            return true;
        }

        let cur_us = (status.position_f64.max(0.0) * 1_000_000.0).round() as i64;
        let offset_us = target_us - cur_us;
        conn.call_method(
            Some(bus.as_str()),
            "/org/mpris/MediaPlayer2",
            Some("org.mpris.MediaPlayer2.Player"),
            "Seek",
            &(offset_us,),
        )
        .is_ok()
    }

    #[cfg(target_os = "windows")]
    {
        let _ = (cached_bus, cached_track_id);
        if let Some(sess) = win_pick_session() {
            let start_ticks = sess
                .GetTimelineProperties()
                .ok()
                .and_then(|tl| tl.StartTime().ok())
                .map(|d| d.Duration.max(0))
                .unwrap_or(0);
            let ticks = start_ticks + (target_secs.max(0.0) * 10_000_000.0).round() as i64;
            return sess
                .TryChangePlaybackPositionAsync(ticks)
                .ok()
                .and_then(|op| op.get().ok())
                .unwrap_or(false);
        }
        false
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = (cached_bus, cached_track_id, target_secs);
        false
    }
}

pub fn send_mpris_command(method: &str) -> bool {
    #[cfg(target_os = "linux")]
    {
        let Some(conn) = get_session_bus() else {
            return false;
        };
        let status = query_media_status();
        let Some(bus) = status
            .active_bus
            .or_else(|| find_all_mpris_buses().into_iter().next())
        else {
            return false;
        };
        conn.call_method(
            Some(bus.as_str()),
            "/org/mpris/MediaPlayer2",
            Some("org.mpris.MediaPlayer2.Player"),
            method,
            &(),
        )
        .is_ok()
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(sess) = win_pick_session() {
            let op = match method {
                "PlayPause" => sess.TryTogglePlayPauseAsync(),
                "Next" => sess.TrySkipNextAsync(),
                "Previous" => sess.TrySkipPreviousAsync(),
                _ => return false,
            };
            return op.ok().and_then(|o| o.get().ok()).unwrap_or(false);
        }
        false
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = method;
        false
    }
}

#[allow(dead_code)]
pub fn format_mm_ss(secs: u64) -> String {
    let m = secs / 60;
    let s = secs % 60;
    format!("{:02}:{:02}", m, s)
}
