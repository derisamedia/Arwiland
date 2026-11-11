use std::process::Command;

#[allow(dead_code)]
#[derive(Clone, Debug, Default)]
pub struct MediaStatus {
    pub active_bus: Option<String>,
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

pub fn find_all_mpris_buses() -> Vec<String> {
    #[cfg(target_os = "linux")]
    {
        let Ok(out) = Command::new("busctl")
            .args(["--user", "list", "--acquired", "--no-pager"])
            .output()
        else {
            return Vec::new();
        };
        if !out.status.success() {
            return Vec::new();
        }
        let text = String::from_utf8_lossy(&out.stdout);
        let mut buses = Vec::new();
        for line in text.lines() {
            if let Some(bus_name) = line.split_whitespace().next() {
                if bus_name.starts_with("org.mpris.MediaPlayer2.") {
                    buses.push(bus_name.to_string());
                }
            }
        }
        buses
    }
    #[cfg(not(target_os = "linux"))]
    {
        Vec::new()
    }
}

#[cfg(target_os = "linux")]
fn get_all_player_properties(bus: &str) -> Option<String> {
    let out = Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--timeout",
            "1",
            "--dest",
            bus,
            "--object-path",
            "/org/mpris/MediaPlayer2",
            "--method",
            "org.freedesktop.DBus.Properties.GetAll",
            "org.mpris.MediaPlayer2.Player",
        ])
        .output()
        .ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        None
    }
}

/// Parses a GVariant string value after `'key': <...>` in gdbus output.
/// Handles both `<"string with 'apostrophe'">`, `<'string'>`, and array `<['artist']>` / `<["artist"]>`.
#[cfg(target_os = "linux")]
fn extract_gvariant_str(raw: &str, key: &str) -> Option<String> {
    let key_pattern = format!("'{}':", key);
    let pos = raw.find(&key_pattern)?;
    let after_key = &raw[pos + key_pattern.len()..];
    let lt_pos = after_key.find('<')?;
    let mut inner = after_key[lt_pos + 1..].trim_start();

    // If array of strings like ['SlowBass Nation'] or ["Artist"]
    if inner.starts_with('[') {
        inner = inner[1..].trim_start();
    }

    let quote = inner.chars().next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }
    let content_start = &inner[quote.len_utf8()..];

    // Find closing matching quote followed by `]>` or `>` or `,`
    let mut end_offset = None;
    for (idx, ch) in content_start.char_indices() {
        if ch == quote {
            let tail = &content_start[idx + quote.len_utf8()..];
            if tail.starts_with('>')
                || tail.starts_with("]>")
                || tail.starts_with("',")
                || tail.starts_with("\",")
            {
                end_offset = Some(idx);
                break;
            }
        }
    }

    let end_idx = end_offset.or_else(|| content_start.find(quote))?;
    let val = content_start[..end_idx].trim();
    if val.is_empty() {
        None
    } else {
        Some(val.to_string())
    }
}

/// Extracts an integer GVariant property like `'mpris:length': <int64 433861000>` or `'Position': <int64 112781069>`
#[cfg(target_os = "linux")]
fn extract_gvariant_i64(raw: &str, key: &str) -> Option<i64> {
    let key_pattern = format!("'{}':", key);
    let pos = raw.find(&key_pattern)?;
    let after_key = &raw[pos + key_pattern.len()..];
    let lt_pos = after_key.find('<')?;
    let gt_pos = after_key[lt_pos..].find('>')?;
    let inside = &after_key[lt_pos + 1..lt_pos + gt_pos];
    for tok in inside.split_whitespace() {
        if let Ok(v) = tok.parse::<i64>() {
            return Some(v);
        }
    }
    None
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

pub fn query_media_status() -> MediaStatus {
    #[cfg(target_os = "linux")]
    {
        let buses = find_all_mpris_buses();
        let mut fallback: Option<MediaStatus> = None;

        for bus in buses {
            let Some(raw) = get_all_player_properties(&bus) else {
                continue;
            };

            let playback = extract_gvariant_str(&raw, "PlaybackStatus").unwrap_or_default();
            let playing = playback.eq_ignore_ascii_case("Playing");
            let player_name = format_player_name(&bus);
            let title = extract_gvariant_str(&raw, "xesam:title")
                .unwrap_or_else(|| "Media Aktif".to_string());
            let artist =
                extract_gvariant_str(&raw, "xesam:artist").unwrap_or_else(|| player_name.clone());

            let art_url = extract_gvariant_str(&raw, "mpris:artUrl");
            let art_path = art_url.and_then(|u| {
                if let Some(stripped) = u.strip_prefix("file://") {
                    if std::path::Path::new(stripped).exists() {
                        return Some(stripped.to_string());
                    }
                }
                None
            });

            let pos_us = extract_gvariant_i64(&raw, "Position").unwrap_or(0).max(0) as u64;
            let len_us = extract_gvariant_i64(&raw, "mpris:length")
                .unwrap_or(0)
                .max(0) as u64;

            let status = MediaStatus {
                active_bus: Some(bus),
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

    MediaStatus {
        active_bus: None,
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

pub fn send_mpris_command(method: &str) -> bool {
    #[cfg(target_os = "linux")]
    {
        let status = query_media_status();
        let Some(bus) = status
            .active_bus
            .or_else(|| find_all_mpris_buses().into_iter().next())
        else {
            return false;
        };
        let full_method = format!("org.mpris.MediaPlayer2.Player.{}", method);
        if let Ok(out) = Command::new("gdbus")
            .args([
                "call",
                "--session",
                "--timeout",
                "1",
                "--dest",
                &bus,
                "--object-path",
                "/org/mpris/MediaPlayer2",
                "--method",
                &full_method,
            ])
            .output()
        {
            return out.status.success();
        }
        false
    }

    #[cfg(target_os = "macos")]
    {
        let cmd = match method {
            "PlayPause" => "tell application \"Spotify\" to playpause",
            "Next" => "tell application \"Spotify\" to next track",
            "Previous" => "tell application \"Spotify\" to previous track",
            _ => return false,
        };
        Command::new("osascript")
            .args(["-e", cmd])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let vk = match method {
            "PlayPause" => 179, // VK_MEDIA_PLAY_PAUSE
            "Next" => 176,      // VK_MEDIA_NEXT_TRACK
            "Previous" => 177,  // VK_MEDIA_PREV_TRACK
            _ => return false,
        };
        let ps = format!("(New-Object -ComObject WScript.Shell).SendKeys([char]{})", vk);
        Command::new("powershell")
            .creation_flags(CREATE_NO_WINDOW)
            .args(["-NoProfile", "-NonInteractive", "-Command", &ps])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
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
