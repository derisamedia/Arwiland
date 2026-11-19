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
#[derive(Clone, Debug, Default)]
pub struct MediaStatus {
    pub active_bus: Option<String>,
    pub track_id: Option<String>,
    pub has_player: bool,
    pub player_name: String,
    pub title: String,
    pub artist: String,
    pub art_path: Option<String>,
    pub playing: bool,
    pub position_secs: u64,
    pub position_f64: f64,
    pub length_secs: u64,
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
fn win_pick_session() -> Option<GlobalSystemMediaTransportControlsSession> {
    win_ensure_com();
    let mgr = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
        .ok()?
        .get()
        .ok()?;
    let mut fallback = mgr.GetCurrentSession().ok();
    if let Ok(list) = mgr.GetSessions() {
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
                    return Some(sess);
                }
                if fallback.is_none() {
                    fallback = Some(sess);
                }
            }
        }
    }
    fallback
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
                    if let Some(stripped) = u.strip_prefix("file://") {
                        if std::path::Path::new(stripped).exists() {
                            return Some(stripped.to_string());
                        }
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
                    playing,
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
                        return "Spotify|" & pState & "|" & tName & "|" & tArtist & "|" & (tPos as string) & "|" & (tDur as string)
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
                return MediaStatus {
                    active_bus: Some(parts[0].to_string()),
                    track_id: None,
                    has_player: true,
                    player_name: parts[0].to_string(),
                    title: parts[2].to_string(),
                    artist: parts[3].to_string(),
                    art_path: None,
                    playing,
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
            let playing = sess
                .GetPlaybackInfo()
                .ok()
                .and_then(|pi| pi.PlaybackStatus().ok())
                .map(|st| st == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing)
                .unwrap_or(false);

            let (title, artist) = sess
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
                    (
                        if t.is_empty() {
                            "Media Aktif".to_string()
                        } else {
                            t
                        },
                        if a.is_empty() {
                            player_name.clone()
                        } else {
                            a
                        },
                    )
                })
                .unwrap_or_else(|| ("Media Aktif".to_string(), player_name.clone()));

            let (pos_f64, len_secs) = sess
                .GetTimelineProperties()
                .ok()
                .map(|tl| {
                    let pos_ticks = tl.Position().map(|d| d.Duration).unwrap_or(0).max(0);
                    let end_ticks = tl.EndTime().map(|d| d.Duration).unwrap_or(0).max(0);
                    let mut pos = pos_ticks as f64 / 10_000_000.0;
                    let len = (end_ticks as f64 / 10_000_000.0).max(0.0);
                    if playing {
                        if let Ok(last_upd) = tl.LastUpdatedTime() {
                            // Windows FILETIME epoch = 1601-01-01; Unix epoch delta = 11644473600s
                            if last_upd.UniversalTime > 116_444_736_000_000_000 {
                                let upd_unix_s = (last_upd.UniversalTime
                                    - 116_444_736_000_000_000)
                                    as f64
                                    / 10_000_000.0;
                                let now_unix_s = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .map(|d| d.as_secs_f64())
                                    .unwrap_or(upd_unix_s);
                                let delta = (now_unix_s - upd_unix_s).clamp(0.0, 30.0);
                                pos += delta;
                                if len > 0.5 {
                                    pos = pos.min(len);
                                }
                            }
                        }
                    }
                    (pos.max(0.0), len.round() as u64)
                })
                .unwrap_or((0.0, 0));

            return MediaStatus {
                active_bus: Some(aumid),
                track_id: None,
                has_player: true,
                player_name,
                title,
                artist,
                art_path: None,
                playing,
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
        playing: false,
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
            let ticks = (target_secs.max(0.0) * 10_000_000.0).round() as i64;
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
