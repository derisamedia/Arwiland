use serde::Deserialize;
use std::collections::HashMap;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Clone, Debug)]
pub struct LyricWord {
    pub text: String,
    pub start_sec: f64,
    pub end_sec: f64,
    pub space_after: bool,
}

#[derive(Clone, Debug)]
pub struct LyricLine {
    pub time_sec: f64,
    pub end_sec: f64,
    pub text: String,
    pub words: Vec<LyricWord>,
    pub is_word_synced: bool,
}

#[derive(Clone, Debug)]
pub struct LyricSnapshot {
    pub prev_line_text: String,
    pub current_text: String,
    pub next_line_text: String,
    pub words: Vec<LyricWord>,
    pub active_word_idx: usize,
    pub word_progress: f64,
    pub line_progress: f64,
    pub is_word_synced: bool,
    pub provider_badge: String,
    pub provider_detail: String,
    pub accent_rgb: (f64, f64, f64),
}

#[derive(Clone, Debug)]
pub enum TrackLyricsState {
    Loading,
    Synced {
        provider_badge: String,
        provider_detail: String,
        accent_rgb: (f64, f64, f64),
        lines: Vec<LyricLine>,
    },
    Unsupported {
        provider_badge: String,
        provider_detail: String,
        accent_rgb: (f64, f64, f64),
    },
}

#[derive(Deserialize)]
struct AppleTtmlResp {
    ttml: Option<String>,
}

#[derive(Deserialize)]
struct LrcLibItem {
    #[serde(rename = "syncedLyrics")]
    synced_lyrics: Option<String>,
}

#[derive(Deserialize)]
struct SimpMusicResp {
    data: Option<Vec<SimpMusicItem>>,
}

#[derive(Deserialize)]
struct SimpMusicItem {
    #[serde(rename = "richSyncLyrics")]
    rich_sync_lyrics: Option<String>,
    #[serde(rename = "syncedLyrics")]
    synced_lyrics: Option<String>,
}

#[derive(Deserialize)]
struct NetEaseSearchResp {
    result: Option<NetEaseSearchResult>,
}

#[derive(Deserialize)]
struct NetEaseSearchResult {
    songs: Option<Vec<NetEaseSong>>,
}

#[derive(Deserialize)]
struct NetEaseSong {
    id: u64,
}

#[derive(Deserialize)]
struct NetEaseLyricResp {
    yrc: Option<NetEaseLyricField>,
    klyric: Option<NetEaseLyricField>,
}

#[derive(Deserialize)]
struct NetEaseLyricField {
    lyric: Option<String>,
}

#[derive(Deserialize)]
struct YtJson3Resp {
    events: Option<Vec<YtJson3Event>>,
}

#[derive(Deserialize)]
struct YtJson3Event {
    #[serde(rename = "tStartMs")]
    t_start_ms: Option<f64>,
    #[serde(rename = "dDurationMs")]
    d_duration_ms: Option<f64>,
    segs: Option<Vec<YtJson3Seg>>,
}

#[derive(Deserialize)]
struct YtJson3Seg {
    utf8: Option<String>,
    #[serde(rename = "tOffsetMs")]
    t_offset_ms: Option<f64>,
}

pub struct LyricsManager {
    cache: Arc<Mutex<HashMap<String, TrackLyricsState>>>,
}

impl LyricsManager {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn clear_cache(&self) {
        if let Ok(mut guard) = self.cache.lock() {
            guard.clear();
        }
    }

    pub fn get_lyric_snapshot(
        &self,
        title: &str,
        artist: &str,
        pos_sec: f64,
        length_sec: f64,
        provider_pref: &str,
        offset_ms: i32,
    ) -> LyricSnapshot {
        let clean_title = clean_track_title(title);
        let key = format!(
            "{}||{}||{}",
            provider_pref.to_lowercase(),
            clean_title.to_lowercase(),
            artist.to_lowercase()
        );

        let state_opt = {
            let guard = self.cache.lock().unwrap();
            guard.get(&key).cloned()
        };

        let adj_pos = (pos_sec + (offset_ms as f64 / 1000.0) + 0.12).max(0.0);

        match state_opt {
            None => {
                {
                    let mut guard = self.cache.lock().unwrap();
                    guard.insert(key.clone(), TrackLyricsState::Loading);
                }
                let cache_clone = self.cache.clone();
                let q_title = clean_title.clone();
                let q_artist = artist.to_string();
                let prov = provider_pref.to_string();

                thread::spawn(move || {
                    let fetched = fetch_multi_provider(&prov, &q_title, &q_artist);
                    let mut guard = cache_clone.lock().unwrap();
                    guard.insert(key, fetched);
                });

                build_intro_dots_snapshot(
                    adj_pos,
                    0.0,
                    (adj_pos + 2.4).max(3.0),
                    "MEMUAT",
                    "Mencari lirik per-kata",
                    provider_accent(provider_pref),
                )
            }
            Some(TrackLyricsState::Loading) => build_intro_dots_snapshot(
                adj_pos,
                0.0,
                (adj_pos + 2.4).max(3.0),
                "MEMUAT",
                "Mencari lirik per-kata",
                provider_accent(provider_pref),
            ),
            Some(TrackLyricsState::Synced {
                provider_badge,
                provider_detail,
                accent_rgb,
                lines,
            }) => {
                // Strict Per-Kata Gate: only display lines that support word-by-word sync!
                let word_lines: Vec<&LyricLine> = lines
                    .iter()
                    .filter(|l| l.is_word_synced && !l.words.is_empty())
                    .collect();

                if word_lines.is_empty() {
                    return build_empty_snapshot(&provider_badge, &provider_detail, accent_rgb);
                }

                // Show animated countdown dots ("●  ●  ●") before the first vocal line starts!
                let intro_end = word_lines[0].time_sec.max(2.0);
                if adj_pos < intro_end {
                    let mut snap = build_intro_dots_snapshot(
                        adj_pos,
                        0.0,
                        intro_end,
                        &provider_badge,
                        &provider_detail,
                        accent_rgb,
                    );
                    snap.next_line_text = word_lines[0].text.clone();
                    return snap;
                }

                let mut cur_idx = 0;
                for (i, line) in word_lines.iter().enumerate() {
                    if line.time_sec <= adj_pos {
                        cur_idx = i;
                    } else {
                        break;
                    }
                }

                let cur_line = word_lines[cur_idx];
                let next_line_start = word_lines
                    .get(cur_idx + 1)
                    .map(|l| l.time_sec)
                    .unwrap_or_else(|| {
                        if length_sec > cur_line.time_sec + 1.0 {
                            length_sec
                        } else {
                            cur_line.end_sec.max(cur_line.time_sec + 4.0)
                        }
                    });

                let prev_line_text = if cur_idx > 0 {
                    word_lines[cur_idx - 1].text.clone()
                } else {
                    String::new()
                };
                let next_line_text = word_lines
                    .get(cur_idx + 1)
                    .map(|l| l.text.clone())
                    .unwrap_or_default();

                // If there is a long instrumental break before the next vocal line, show countdown dots!
                if cur_idx + 1 < word_lines.len()
                    && next_line_start - cur_line.end_sec >= 4.0
                    && adj_pos > cur_line.end_sec + 0.45
                    && adj_pos < next_line_start
                {
                    let mut snap = build_intro_dots_snapshot(
                        adj_pos,
                        cur_line.end_sec + 0.45,
                        next_line_start,
                        &provider_badge,
                        &provider_detail,
                        accent_rgb,
                    );
                    snap.prev_line_text = cur_line.text.clone();
                    snap.next_line_text = next_line_text;
                    return snap;
                }

                // Hide lyrics cleanly after the final vocal line finishes
                if cur_idx + 1 == word_lines.len() && adj_pos > cur_line.end_sec + 2.5 {
                    return build_empty_snapshot(&provider_badge, &provider_detail, accent_rgb);
                }

                let effective_end = if cur_line.end_sec > cur_line.time_sec + 0.3 {
                    cur_line.end_sec.min(next_line_start)
                } else {
                    next_line_start
                };
                let line_dur = (effective_end - cur_line.time_sec).clamp(0.4, 12.0);
                let raw_line_prog = ((adj_pos - cur_line.time_sec) / line_dur).clamp(0.0, 1.0);

                let (words_resolved, active_word_idx, word_prog, vocal_line_prog) =
                    resolve_line_words(cur_line, adj_pos, raw_line_prog);

                LyricSnapshot {
                    prev_line_text,
                    current_text: cur_line.text.clone(),
                    next_line_text,
                    words: words_resolved,
                    active_word_idx,
                    word_progress: word_prog,
                    line_progress: vocal_line_prog,
                    is_word_synced: true,
                    provider_badge,
                    provider_detail,
                    accent_rgb,
                }
            }
            Some(TrackLyricsState::Unsupported {
                provider_badge,
                provider_detail,
                accent_rgb,
            }) => build_empty_snapshot(&provider_badge, &provider_detail, accent_rgb),
        }
    }
}

/// Color Consistency Lock (`taste-SKILL.md` §4.2): Singular Emerald accent `(0.20, 0.83, 0.60)` (`#34d399`)
pub fn provider_accent(_provider: &str) -> (f64, f64, f64) {
    (0.20, 0.83, 0.60)
}

fn build_empty_snapshot(badge: &str, detail: &str, accent_rgb: (f64, f64, f64)) -> LyricSnapshot {
    LyricSnapshot {
        prev_line_text: String::new(),
        current_text: String::new(),
        next_line_text: String::new(),
        words: Vec::new(),
        active_word_idx: 0,
        word_progress: 0.0,
        line_progress: 0.0,
        is_word_synced: false,
        provider_badge: badge.to_string(),
        provider_detail: detail.to_string(),
        accent_rgb,
    }
}

fn resolve_line_words(
    line: &LyricLine,
    adj_pos: f64,
    raw_line_prog: f64,
) -> (Vec<LyricWord>, usize, f64, f64) {
    let mut active_idx = 0;
    let mut active_wp = 0.0;
    let n = line.words.len();

    for (i, w) in line.words.iter().enumerate() {
        if adj_pos >= w.end_sec {
            active_idx = i;
            active_wp = 1.0;
        } else if adj_pos >= w.start_sec {
            active_idx = i;
            let w_dur = (w.end_sec - w.start_sec).max(0.04);
            active_wp = ((adj_pos - w.start_sec) / w_dur).clamp(0.0, 1.0);
            break;
        } else {
            if i == 0 {
                active_idx = 0;
                active_wp = 0.0;
            }
            break;
        }
    }

    let vocal_prog = if n > 0 {
        ((active_idx as f64 + active_wp) / (n as f64)).clamp(0.0, 1.0)
    } else {
        raw_line_prog
    };
    (line.words.clone(), active_idx, active_wp, vocal_prog)
}

fn build_intro_dots_snapshot(
    pos_sec: f64,
    span_start: f64,
    span_end: f64,
    badge: &str,
    detail: &str,
    accent_rgb: (f64, f64, f64),
) -> LyricSnapshot {
    let total_dur = (span_end - span_start).max(1.2);
    let step = total_dur / 3.0;
    let words = vec![
        LyricWord {
            text: "●".to_string(),
            start_sec: span_start,
            end_sec: span_start + step,
            space_after: true,
        },
        LyricWord {
            text: "●".to_string(),
            start_sec: span_start + step,
            end_sec: span_start + step * 2.0,
            space_after: true,
        },
        LyricWord {
            text: "●".to_string(),
            start_sec: span_start + step * 2.0,
            end_sec: span_end,
            space_after: false,
        },
    ];

    let elapsed = (pos_sec - span_start).clamp(0.0, total_dur);
    let line_prog = (elapsed / total_dur).clamp(0.0, 1.0);
    let mut active_word_idx = 0;
    let mut word_prog = 0.0;
    for (i, w) in words.iter().enumerate() {
        if pos_sec <= w.end_sec || i + 1 == words.len() {
            active_word_idx = i;
            let w_dur = (w.end_sec - w.start_sec).max(0.05);
            word_prog = ((pos_sec - w.start_sec) / w_dur).clamp(0.0, 1.0);
            break;
        }
    }

    LyricSnapshot {
        prev_line_text: String::new(),
        current_text: "●  ●  ●".to_string(),
        next_line_text: String::new(),
        words,
        active_word_idx,
        word_progress: word_prog,
        line_progress: line_prog,
        is_word_synced: true,
        provider_badge: badge.to_string(),
        provider_detail: detail.to_string(),
        accent_rgb,
    }
}

fn clean_track_title(raw: &str) -> String {
    let mut s = raw.to_string();
    for bracket in [('(', ')'), ('[', ']')] {
        while let (Some(a), Some(b)) = (s.find(bracket.0), s.find(bracket.1)) {
            if b > a {
                s.replace_range(a..=b, "");
            } else {
                break;
            }
        }
    }
    let noise_words = [
        "STADIUM BREAKBEAT",
        "SLOWBASS",
        "FULL BASS",
        "OFFICIAL MUSIC VIDEO",
        "OFFICIAL VIDEO",
        "OFFICIAL AUDIO",
        "LYRIC VIDEO",
        "LYRICS",
        "REMIX",
        "DJ ",
    ];
    let mut upper = s.to_uppercase();
    for nw in noise_words {
        if let Some(pos) = upper.find(nw) {
            s.replace_range(pos..pos + nw.len(), "");
            upper = s.to_uppercase();
        }
    }
    let trimmed = s.trim().trim_matches('-').trim();
    if trimmed.is_empty() {
        raw.to_string()
    } else {
        trimmed.to_string()
    }
}

fn is_metadata_credit_line(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.starts_with("作词")
        || lower.starts_with("作曲")
        || lower.starts_with("编曲")
        || lower.starts_with("制作")
        || lower.starts_with("词：")
        || lower.starts_with("曲：")
        || lower.starts_with("lyricist")
        || lower.starts_with("lyrics by")
        || lower.starts_with("composed by")
        || lower.starts_with("produced by")
        || lower.starts_with("composer")
        || lower.starts_with("arranged by")
}

fn parse_time_str(s: &str) -> Option<f64> {
    let s = s.trim().trim_end_matches('s');
    let parts: Vec<&str> = s.split(':').collect();
    match parts.len() {
        1 => parts[0].parse::<f64>().ok(),
        2 => {
            let m = parts[0].parse::<f64>().ok()?;
            let sec = parts[1].parse::<f64>().ok()?;
            Some(m * 60.0 + sec)
        }
        3 => {
            let h = parts[0].parse::<f64>().ok()?;
            let m = parts[1].parse::<f64>().ok()?;
            let sec = parts[2].parse::<f64>().ok()?;
            Some(h * 3600.0 + m * 60.0 + sec)
        }
        _ => None,
    }
}

fn extract_xml_attr<'a>(tag_header: &'a str, attr_name: &str) -> Option<&'a str> {
    let pat = format!("{}=\"", attr_name);
    let start = tag_header.find(&pat)? + pat.len();
    let rem = &tag_header[start..];
    let end = rem.find('"')?;
    Some(&rem[..end])
}

fn unescape_xml(s: &str) -> String {
    s.replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&quot;", "\"")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

/// Parses Apple Music & YouLy Plus Word-Synced TTML (`<p begin="..." end="..."><span begin="..." end="...">word</span></p>`)
fn parse_apple_ttml(ttml: &str) -> Vec<LyricLine> {
    let mut out = Vec::new();
    let mut rem = ttml;

    while let Some(p_pos) = rem.find("<p ") {
        let after_p = &rem[p_pos + 3..];
        let Some(p_gt) = after_p.find('>') else {
            break;
        };
        let p_header = &after_p[..p_gt];
        let p_body_start = &after_p[p_gt + 1..];
        let Some(p_end_pos) = p_body_start.find("</p>") else {
            break;
        };
        let p_inner = &p_body_start[..p_end_pos];
        rem = &p_body_start[p_end_pos + 4..];

        let line_begin = extract_xml_attr(p_header, "begin")
            .and_then(parse_time_str)
            .unwrap_or(0.0);
        let line_end = extract_xml_attr(p_header, "end")
            .and_then(parse_time_str)
            .unwrap_or(line_begin + 3.5);

        let mut words: Vec<LyricWord> = Vec::new();
        let mut span_rem = p_inner;
        while let Some(s_pos) = span_rem.find("<span") {
            let after_s = &span_rem[s_pos + 5..];
            let Some(s_gt) = after_s.find('>') else {
                break;
            };
            let s_header = &after_s[..s_gt];
            let s_content_start = &after_s[s_gt + 1..];
            let Some(s_close) = s_content_start.find("</span>") else {
                break;
            };
            let raw_txt = unescape_xml(&s_content_start[..s_close]);
            let after_close = &s_content_start[s_close + 7..];
            let next_span_idx = after_close.find("<span").unwrap_or(after_close.len());
            let between_text = &after_close[..next_span_idx];
            let has_space_after = raw_txt.ends_with(char::is_whitespace)
                || between_text.contains(char::is_whitespace);

            span_rem = &after_close[next_span_idx..];

            let trimmed = raw_txt.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let (Some(wb), Some(we)) = (
                extract_xml_attr(s_header, "begin").and_then(parse_time_str),
                extract_xml_attr(s_header, "end").and_then(parse_time_str),
            ) {
                words.push(LyricWord {
                    text: trimmed.to_string(),
                    start_sec: wb,
                    end_sec: we.max(wb + 0.04),
                    space_after: has_space_after,
                });
            }
        }

        if let Some(last_w) = words.last_mut() {
            last_w.space_after = false;
        }

        if !words.is_empty() {
            let mut full_text = String::new();
            for w in &words {
                full_text.push_str(&w.text);
                if w.space_after {
                    full_text.push(' ');
                }
            }
            let full_text = full_text.trim().to_string();
            if !full_text.is_empty() && !is_metadata_credit_line(&full_text) {
                let first_start = words.first().map(|w| w.start_sec).unwrap_or(line_begin);
                let last_end = words.last().map(|w| w.end_sec).unwrap_or(line_end);
                out.push(LyricLine {
                    time_sec: first_start,
                    end_sec: last_end.max(line_end),
                    text: full_text,
                    words,
                    is_word_synced: true,
                });
            }
        }
    }

    out.sort_by(|a, b| {
        a.time_sec
            .partial_cmp(&b.time_sec)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

/// Parses KuGou `krc` Word-by-Word Karaoke format:
/// `[11516,5622]<0,844,0>Walaupun <844,1429,0>jauh <2641,320,0>dan <2961,1291,0>walaupun <4252,1370,0>berliku`
fn parse_kugou_krc(krc_text: &str) -> Vec<LyricLine> {
    let mut out = Vec::new();
    for raw_line in krc_text.lines() {
        let line = raw_line.trim();
        if !line.starts_with('[') || !line.contains('<') {
            continue;
        }
        let Some(close_b) = line.find(']') else {
            continue;
        };
        let header = &line[1..close_b];
        let mut h_parts = header.split(',');
        let Some(start_ms_s) = h_parts.next() else {
            continue;
        };
        let dur_ms: f64 = h_parts
            .next()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(3000.0)
            .max(200.0);
        let Ok(line_start_ms) = start_ms_s.parse::<f64>() else {
            continue;
        };

        let body = &line[close_b + 1..];
        let mut words: Vec<LyricWord> = Vec::new();
        let mut rem = body;

        while let Some(open_lt) = rem.find('<') {
            let Some(close_gt_rel) = rem[open_lt..].find('>') else {
                break;
            };
            let close_gt = open_lt + close_gt_rel;
            let timing_str = &rem[open_lt + 1..close_gt];
            let mut t_parts = timing_str.split(',');
            let word_offset_ms = t_parts
                .next()
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);
            let word_dur_ms = t_parts
                .next()
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(250.0)
                .max(30.0);

            let after_gt = &rem[close_gt + 1..];
            let next_lt = after_gt.find('<').unwrap_or(after_gt.len());
            let raw_tok = after_gt[..next_lt].replace('＇', "'");
            rem = &after_gt[next_lt..];

            let has_trailing_space =
                raw_tok.ends_with(char::is_whitespace) || raw_tok.ends_with('-');
            let trimmed = raw_tok.trim();
            if trimmed.is_empty() {
                if let Some(last) = words.last_mut() {
                    last.space_after = true;
                }
                continue;
            }

            let w_start = (line_start_ms + word_offset_ms) / 1000.0;
            let w_end = (line_start_ms + word_offset_ms + word_dur_ms) / 1000.0;

            if (trimmed == "," || trimmed == "." || trimmed == "!" || trimmed == "?")
                && !words.is_empty()
            {
                if let Some(last) = words.last_mut() {
                    last.text.push_str(trimmed);
                    last.end_sec = w_end.max(last.end_sec);
                    last.space_after = true;
                }
            } else {
                words.push(LyricWord {
                    text: trimmed.to_string(),
                    start_sec: w_start,
                    end_sec: w_end,
                    space_after: has_trailing_space && !raw_tok.ends_with('-'),
                });
            }
        }

        if let Some(last) = words.last_mut() {
            last.space_after = false;
        }
        if words.is_empty() {
            continue;
        }

        let mut full_text = String::new();
        for w in &words {
            full_text.push_str(&w.text);
            if w.space_after {
                full_text.push(' ');
            }
        }
        let full_text = full_text.trim().to_string();
        let line_start_sec = line_start_ms / 1000.0;
        let line_end_sec = (line_start_ms + dur_ms) / 1000.0;

        if full_text.is_empty()
            || is_metadata_credit_line(&full_text)
            || (line_start_sec < 2.0 && (full_text.contains(" - ") || full_text.contains('：')))
        {
            continue;
        }

        out.push(LyricLine {
            time_sec: line_start_sec,
            end_sec: line_end_sec,
            text: full_text,
            words,
            is_word_synced: true,
        });
    }

    out.sort_by(|a, b| {
        a.time_sec
            .partial_cmp(&b.time_sec)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

/// Parses NetEase `yrc` Word-by-Word format:
/// `[10400,9660](10400,2370,0)Baby(12770,30,0),(12800,1020,0)please (13820,900,0)try ...`
fn parse_netease_yrc(yrc_text: &str) -> Vec<LyricLine> {
    let mut out = Vec::new();
    for raw_line in yrc_text.lines() {
        let line = raw_line.trim();
        if !line.starts_with('[') {
            continue;
        }
        let Some(close_b) = line.find(']') else {
            continue;
        };
        let header = &line[1..close_b];
        let mut h_parts = header.split(',');
        let Some(start_ms_s) = h_parts.next() else {
            continue;
        };
        let dur_ms: f64 = h_parts
            .next()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(3000.0)
            .max(200.0);
        let Ok(line_start_ms) = start_ms_s.parse::<f64>() else {
            continue;
        };

        let body = &line[close_b + 1..];
        let mut words: Vec<LyricWord> = Vec::new();
        let mut rem = body;

        while let Some(open_p) = rem.find('(') {
            let Some(close_p_rel) = rem[open_p..].find(')') else {
                break;
            };
            let close_p = open_p + close_p_rel;
            let timing_str = &rem[open_p + 1..close_p];
            let mut t_parts = timing_str.split(',');
            let tok_start_ms = t_parts
                .next()
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(line_start_ms);
            let tok_dur_ms = t_parts
                .next()
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(250.0)
                .max(30.0);

            let after_p = &rem[close_p + 1..];
            let next_open = after_p.find('(').unwrap_or(after_p.len());
            let raw_tok = after_p[..next_open].replace('＇', "'");
            rem = &after_p[next_open..];

            let has_trailing_space = raw_tok.ends_with(char::is_whitespace);
            let trimmed = raw_tok.trim();
            if trimmed.is_empty() {
                if let Some(last) = words.last_mut() {
                    last.space_after = true;
                }
                continue;
            }

            let w_start = tok_start_ms / 1000.0;
            let w_end = (tok_start_ms + tok_dur_ms) / 1000.0;

            if (trimmed == "," || trimmed == "." || trimmed == "!" || trimmed == "?")
                && !words.is_empty()
            {
                if let Some(last) = words.last_mut() {
                    last.text.push_str(trimmed);
                    last.end_sec = w_end.max(last.end_sec);
                    last.space_after = true;
                }
            } else {
                words.push(LyricWord {
                    text: trimmed.to_string(),
                    start_sec: w_start,
                    end_sec: w_end,
                    space_after: has_trailing_space,
                });
            }
        }

        if let Some(last) = words.last_mut() {
            last.space_after = false;
        }
        if words.is_empty() {
            continue;
        }

        let mut full_text = String::new();
        for w in &words {
            full_text.push_str(&w.text);
            if w.space_after {
                full_text.push(' ');
            }
        }
        let full_text = full_text.trim().to_string();
        if full_text.is_empty() || is_metadata_credit_line(&full_text) {
            continue;
        }

        let line_start_sec = line_start_ms / 1000.0;
        let line_end_sec = (line_start_ms + dur_ms) / 1000.0;
        out.push(LyricLine {
            time_sec: line_start_sec,
            end_sec: line_end_sec,
            text: full_text,
            words,
            is_word_synced: true,
        });
    }

    out.sort_by(|a, b| {
        a.time_sec
            .partial_cmp(&b.time_sec)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

/// Parses Enhanced LRC `[mm:ss.xx] <mm:ss.xx> word <mm:ss.xx> word` (strictly word-synced lines)
fn parse_enhanced_lrc(lrc_text: &str) -> Vec<LyricLine> {
    let mut out: Vec<LyricLine> = Vec::new();
    for raw_line in lrc_text.lines() {
        let line = raw_line.trim();
        if !line.starts_with('[') {
            continue;
        }
        let Some(close_bracket) = line.find(']') else {
            continue;
        };
        let time_str = &line[1..close_bracket];
        let Some(line_start_sec) = parse_time_str(time_str) else {
            continue;
        };

        let raw_body = unescape_xml(line[close_bracket + 1..].trim()).replace('＇', "'");
        if raw_body.is_empty() || !raw_body.contains('<') || !raw_body.contains('>') {
            continue;
        }

        let mut words: Vec<LyricWord> = Vec::new();
        let mut rem = raw_body.as_str();
        while let Some(lt) = rem.find('<') {
            let Some(gt_rel) = rem[lt..].find('>') else {
                break;
            };
            let gt = lt + gt_rel;
            let tag_time_s = &rem[lt + 1..gt];
            let after_gt = &rem[gt + 1..];
            let next_lt = after_gt.find('<').unwrap_or(after_gt.len());
            let seg_txt = after_gt[..next_lt].trim();
            rem = &after_gt[next_lt..];

            if let Some(w_start) = parse_time_str(tag_time_s) {
                if let Some(prev) = words.last_mut() {
                    prev.end_sec = w_start.max(prev.start_sec + 0.05);
                }
                if !seg_txt.is_empty() {
                    words.push(LyricWord {
                        text: seg_txt.to_string(),
                        start_sec: w_start,
                        end_sec: w_start + 0.45,
                        space_after: true,
                    });
                }
            }
        }
        if let Some(last) = words.last_mut() {
            last.space_after = false;
        }
        if words.len() >= 2 {
            let full_text = words
                .iter()
                .map(|w| w.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            if !is_metadata_credit_line(&full_text) {
                let end_sec = words
                    .last()
                    .map(|w| w.end_sec)
                    .unwrap_or(line_start_sec + 3.5);
                out.push(LyricLine {
                    time_sec: line_start_sec,
                    end_sec,
                    text: full_text,
                    words,
                    is_word_synced: true,
                });
            }
        }
    }

    out.sort_by(|a, b| {
        a.time_sec
            .partial_cmp(&b.time_sec)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

/// Parses YouTube TimedText `fmt=json3` events (strictly word-level ASR `tOffsetMs`)
fn parse_youtube_json3_words(json_bytes: &[u8]) -> Vec<LyricLine> {
    let Ok(resp) = serde_json::from_slice::<YtJson3Resp>(json_bytes) else {
        return Vec::new();
    };
    let Some(events) = resp.events else {
        return Vec::new();
    };
    let mut out = Vec::new();

    for ev in events {
        let Some(start_ms) = ev.t_start_ms else {
            continue;
        };
        let dur_ms = ev.d_duration_ms.unwrap_or(3200.0).max(300.0);
        let line_start = start_ms / 1000.0;
        let line_end = (start_ms + dur_ms) / 1000.0;
        let Some(segs) = ev.segs else {
            continue;
        };

        let mut words: Vec<LyricWord> = Vec::new();
        let mut has_word_offsets = false;
        for seg in &segs {
            let Some(ref raw_u) = seg.utf8 else {
                continue;
            };
            let cleaned = raw_u.replace('\n', " ").replace('♪', "").trim().to_string();
            if cleaned.is_empty() {
                continue;
            }
            let offset_ms = seg.t_offset_ms.unwrap_or(0.0);
            if offset_ms > 0.0 {
                has_word_offsets = true;
            }
            let w_start = (start_ms + offset_ms) / 1000.0;
            if let Some(prev) = words.last_mut() {
                if w_start > prev.start_sec + 0.03 {
                    prev.end_sec = w_start;
                }
            }
            for tok in cleaned.split_whitespace() {
                words.push(LyricWord {
                    text: tok.to_string(),
                    start_sec: w_start,
                    end_sec: (w_start + 0.38).min(line_end),
                    space_after: true,
                });
            }
        }
        if !has_word_offsets || words.len() < 2 {
            continue;
        }
        if let Some(last) = words.last_mut() {
            last.space_after = false;
            last.end_sec = last.end_sec.max(line_end.min(last.start_sec + 0.8));
        }
        let full_text = words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        if full_text.is_empty() {
            continue;
        }
        out.push(LyricLine {
            time_sec: line_start,
            end_sec: line_end,
            text: full_text,
            words,
            is_word_synced: true,
        });
    }
    out
}

fn retag_word_state(
    state: TrackLyricsState,
    provider_id: &str,
    badge: &str,
    label_prefix: &str,
) -> Option<TrackLyricsState> {
    let accent = provider_accent(provider_id);
    match state {
        TrackLyricsState::Synced { mut lines, .. } => {
            lines.retain(|l| l.is_word_synced && !l.words.is_empty());
            if lines.len() >= 3 {
                Some(TrackLyricsState::Synced {
                    provider_badge: badge.to_string(),
                    provider_detail: format!("{} (Per-Kata) - {} Baris", label_prefix, lines.len()),
                    accent_rgb: accent,
                    lines,
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

fn try_word_sync_cascade(
    clean_title: &str,
    artist: &str,
    provider_id: &str,
    badge: &str,
    label_prefix: &str,
) -> Option<TrackLyricsState> {
    if let Some(st) = try_fetch_kugou_krc(clean_title, artist) {
        return retag_word_state(st, provider_id, badge, label_prefix);
    }
    if let Some(st) = try_fetch_apple_ttml(clean_title, artist) {
        return retag_word_state(st, provider_id, badge, label_prefix);
    }
    if let Some(st) = try_fetch_netease(clean_title, artist) {
        return retag_word_state(st, provider_id, badge, label_prefix);
    }
    if let Some(st) = try_fetch_lrclib_enhanced(clean_title, artist) {
        return retag_word_state(st, provider_id, badge, label_prefix);
    }
    None
}

fn fetch_multi_provider(provider_pref: &str, clean_title: &str, artist: &str) -> TrackLyricsState {
    match provider_pref.to_lowercase().as_str() {
        "yt_captions" => {
            if let Some(st) = try_fetch_youtube_captions(clean_title, artist) {
                return st;
            }
            if let Some(st) = try_word_sync_cascade(
                clean_title,
                artist,
                "yt_captions",
                "YTCAP",
                "Youtube Auto Captions",
            ) {
                return st;
            }
        }
        "youlyplus" => {
            if let Some(st) = try_fetch_youlyplus(clean_title, artist) {
                return st;
            }
            if let Some(st) =
                try_word_sync_cascade(clean_title, artist, "youlyplus", "YOULY+", "YouLy Plus")
            {
                return st;
            }
        }
        "apple" => {
            if let Some(st) = try_fetch_apple_ttml(clean_title, artist) {
                return st;
            }
            if let Some(st) =
                try_word_sync_cascade(clean_title, artist, "apple", "APPLE", "Apple Music")
            {
                return st;
            }
        }
        "simpmusic" => {
            if let Some(st) = try_fetch_simpmusic(clean_title, artist) {
                return st;
            }
            if let Some(st) =
                try_word_sync_cascade(clean_title, artist, "simpmusic", "SIMP", "SimpMusic")
            {
                return st;
            }
        }
        "lrclib" => {
            if let Some(st) = try_fetch_lrclib_enhanced(clean_title, artist) {
                return st;
            }
            if let Some(st) =
                try_word_sync_cascade(clean_title, artist, "lrclib", "LRCLIB", "LrcLib")
            {
                return st;
            }
        }
        "kugou" => {
            if let Some(st) = try_fetch_kugou_krc(clean_title, artist) {
                return st;
            }
            if let Some(st) =
                try_word_sync_cascade(clean_title, artist, "kugou", "KUGOU", "KuGou KRC")
            {
                return st;
            }
        }
        "netease" => {
            if let Some(st) = try_fetch_netease(clean_title, artist) {
                return st;
            }
            if let Some(st) =
                try_word_sync_cascade(clean_title, artist, "netease", "NETEASE", "NetEase YRC")
            {
                return st;
            }
        }
        "qqmusic" => {
            if let Some(st) =
                try_word_sync_cascade(clean_title, artist, "qqmusic", "QQMUSIC", "QQMusic QRC")
            {
                return st;
            }
        }
        "deezer" => {
            if let Some(st) =
                try_word_sync_cascade(clean_title, artist, "deezer", "DEEZER", "Deezer Sync")
            {
                return st;
            }
        }
        "yt_subtitle" => {
            if let Some(st) = try_fetch_youtube_captions(clean_title, artist) {
                return retag_word_state(st, "yt_subtitle", "YTSUB", "Youtube Subtitle")
                    .unwrap_or_else(|| TrackLyricsState::Unsupported {
                        provider_badge: "YTSUB".to_string(),
                        provider_detail: "Tidak mendukung lirik per-kata".to_string(),
                        accent_rgb: provider_accent("yt_subtitle"),
                    });
            }
            if let Some(st) = try_word_sync_cascade(
                clean_title,
                artist,
                "yt_subtitle",
                "YTSUB",
                "Youtube Subtitle",
            ) {
                return st;
            }
        }
        "yt_music" => {
            if let Some(st) =
                try_word_sync_cascade(clean_title, artist, "yt_music", "YTMUSIC", "Youtube Music")
            {
                return st;
            }
        }
        _ => {
            // Auto Mode: Strictly Word-by-Word (Per-Kata) Providers!
            // 1. KuGou KRC (Word-Sync) -> 2. Apple TTML (Word-Sync) -> 3. NetEase YRC (Word-Sync) -> 4. LrcLib Enhanced -> 5. YouTube ASR
            if let Some(st) = try_fetch_kugou_krc(clean_title, artist) {
                return st;
            }
            if let Some(st) = try_fetch_apple_ttml(clean_title, artist) {
                return st;
            }
            if let Some(st) = try_fetch_netease(clean_title, artist) {
                return st;
            }
            if let Some(st) = try_fetch_lrclib_enhanced(clean_title, artist) {
                return st;
            }
            if let Some(st) = try_fetch_youtube_captions(clean_title, artist) {
                return st;
            }
        }
    }

    TrackLyricsState::Unsupported {
        provider_badge: "OFF".to_string(),
        provider_detail: "Lirik per-kata tidak tersedia untuk lagu ini".to_string(),
        accent_rgb: provider_accent(provider_pref),
    }
}

/// KuGou KRC (`fmt=krc`) Word-by-Word Karaoke Fetcher & Zlib/XOR Decoder
fn try_fetch_kugou_krc(clean_title: &str, artist: &str) -> Option<TrackLyricsState> {
    let py_script = r#"
import sys, urllib.request, urllib.parse, json, base64, zlib
title = sys.argv[1]
artist = sys.argv[2]
q = urllib.parse.quote((title + " " + artist).strip())
s_url = f"http://mobilecdn.kugou.com/api/v3/search/song?format=json&page=1&pagesize=4&keyword={q}"
s_data = json.loads(urllib.request.urlopen(s_url, timeout=3).read().decode("utf-8", errors="ignore"))
infos = s_data.get("data", {}).get("info") or []
for info in infos[:3]:
    h = info.get("hash")
    if not h: continue
    c_url = f"http://lyrics.kugou.com/search?ver=1&man=yes&client=pc&hash={h}"
    c_data = json.loads(urllib.request.urlopen(c_url, timeout=3).read().decode("utf-8", errors="ignore"))
    for cand in (c_data.get("candidates") or [])[:2]:
        cid, akey = cand.get("id"), cand.get("accesskey")
        if not cid or not akey: continue
        d_url = f"http://lyrics.kugou.com/download?ver=1&client=pc&id={cid}&accesskey={akey}&fmt=krc&charset=utf8"
        d_data = json.loads(urllib.request.urlopen(d_url, timeout=3).read().decode("utf-8", errors="ignore"))
        b64 = d_data.get("content")
        if not b64: continue
        raw = base64.b64decode(b64)
        if len(raw) > 4 and raw[:3] == b"krc":
            key = [64, 71, 97, 119, 94, 50, 116, 71, 81, 54, 49, 45, 206, 210, 110, 105]
            dec = bytes(b ^ key[i % 16] for i, b in enumerate(raw[4:]))
            sys.stdout.write(zlib.decompress(dec).decode("utf-8", errors="replace"))
            sys.exit(0)
"#;

    let out = Command::new("python3")
        .args(["-c", py_script, clean_title, artist])
        .output()
        .ok()?;

    if !out.status.success() || out.stdout.is_empty() {
        return None;
    }
    let krc_text = String::from_utf8_lossy(&out.stdout);
    let parsed = parse_kugou_krc(&krc_text);
    if parsed.len() >= 3 {
        return Some(TrackLyricsState::Synced {
            provider_badge: "KUGOU".to_string(),
            provider_detail: format!("KuGou KRC (Per-Kata) - {} Baris", parsed.len()),
            accent_rgb: provider_accent("kugou"),
            lines: parsed,
        });
    }
    None
}

/// Youtube Auto Captions (`yt_captions`) Word-Level ASR `json3`
fn try_fetch_youtube_captions(clean_title: &str, artist: &str) -> Option<TrackLyricsState> {
    let query = format!("{} {} official", clean_title, artist);
    let search_out = Command::new("curl")
        .args([
            "-4",
            "--connect-timeout",
            "2",
            "--max-time",
            "3",
            "-sG",
            "-A",
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/124.0.0.0 Safari/537.36",
            "https://www.youtube.com/results",
            "--data-urlencode",
            &format!("search_query={}", query),
        ])
        .output()
        .ok()?;

    if !search_out.status.success() {
        return None;
    }
    let html = String::from_utf8_lossy(&search_out.stdout);
    let vid_marker = "\"videoId\":\"";
    let vid_pos = html.find(vid_marker)? + vid_marker.len();
    let vid_end = html[vid_pos..].find('"')?;
    let video_id = &html[vid_pos..vid_pos + vid_end];
    if video_id.len() < 8 {
        return None;
    }

    let watch_out = Command::new("curl")
        .args([
            "-4",
            "--connect-timeout",
            "2",
            "--max-time",
            "3",
            "-s",
            "-A",
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/124.0.0.0 Safari/537.36",
            &format!("https://www.youtube.com/watch?v={}", video_id),
        ])
        .output()
        .ok()?;

    if !watch_out.status.success() {
        return None;
    }
    let watch_html = String::from_utf8_lossy(&watch_out.stdout);
    let cap_pos = watch_html.find("\"captionTracks\":[")?;
    let cap_slice = &watch_html[cap_pos..];
    let cap_end = cap_slice.find(']')?;
    let tracks_block = &cap_slice[..cap_end];

    let mut chosen_url: Option<String> = None;
    let mut rem = tracks_block;
    while let Some(b_pos) = rem.find("\"baseUrl\":\"") {
        let after_b = &rem[b_pos + 11..];
        let Some(q_end) = after_b.find('"') else {
            break;
        };
        let raw_url = after_b[..q_end].replace("\\u0026", "&");
        let next_chunk = &after_b[q_end..after_b.len().min(q_end + 220)];
        let is_asr_track = next_chunk.contains("\"kind\":\"asr\"");
        if chosen_url.is_none() || is_asr_track {
            chosen_url = Some(raw_url);
            if is_asr_track {
                break;
            }
        }
        rem = &after_b[q_end..];
    }

    let base_url = chosen_url?;
    let json3_url = format!("{}&fmt=json3", base_url);
    if let Ok(cap_out) = Command::new("curl")
        .args([
            "-4",
            "--connect-timeout",
            "2",
            "--max-time",
            "3",
            "-s",
            &json3_url,
        ])
        .output()
    {
        if cap_out.status.success() && !cap_out.stdout.is_empty() {
            let parsed = parse_youtube_json3_words(&cap_out.stdout);
            if parsed.len() >= 3 {
                return Some(TrackLyricsState::Synced {
                    provider_badge: "YTCAP".to_string(),
                    provider_detail: format!(
                        "Youtube Auto Captions (Per-Kata) - {} Baris",
                        parsed.len()
                    ),
                    accent_rgb: provider_accent("yt_captions"),
                    lines: parsed,
                });
            }
        }
    }
    None
}

/// YouLy Plus (`youlyplus`) Word-Synced TTML
fn try_fetch_youlyplus(clean_title: &str, artist: &str) -> Option<TrackLyricsState> {
    let out = Command::new("curl")
        .args([
            "-4",
            "--connect-timeout",
            "2",
            "--max-time",
            "3",
            "-sG",
            "https://lyrics-api.boidu.dev/getLyrics",
            "--data-urlencode",
            &format!("s={}", clean_title),
            "--data-urlencode",
            &format!("a={}", artist),
        ])
        .output()
        .ok()?;

    if out.status.success() {
        if let Ok(resp) = serde_json::from_slice::<AppleTtmlResp>(&out.stdout) {
            if let Some(ttml_str) = resp.ttml {
                let parsed = parse_apple_ttml(&ttml_str);
                if parsed.len() >= 3 {
                    return Some(TrackLyricsState::Synced {
                        provider_badge: "YOULY+".to_string(),
                        provider_detail: format!("YouLy Plus (Per-Kata) - {} Baris", parsed.len()),
                        accent_rgb: provider_accent("youlyplus"),
                        lines: parsed,
                    });
                }
            }
        }
    }
    None
}

/// Apple Music (`apple`) Word-Synced TTML
fn try_fetch_apple_ttml(clean_title: &str, artist: &str) -> Option<TrackLyricsState> {
    let out = Command::new("curl")
        .args([
            "-4",
            "--connect-timeout",
            "2",
            "--max-time",
            "3",
            "-sG",
            "https://lyrics-api.boidu.dev/getLyrics",
            "--data-urlencode",
            &format!("s={}", clean_title),
            "--data-urlencode",
            &format!("a={}", artist),
        ])
        .output()
        .ok()?;

    if !out.status.success() {
        return None;
    }
    let resp: AppleTtmlResp = serde_json::from_slice(&out.stdout).ok()?;
    let ttml_str = resp.ttml?;
    let parsed = parse_apple_ttml(&ttml_str);
    if parsed.len() >= 3 {
        return Some(TrackLyricsState::Synced {
            provider_badge: "APPLE".to_string(),
            provider_detail: format!("Apple Music (Per-Kata) - {} Baris", parsed.len()),
            accent_rgb: provider_accent("apple"),
            lines: parsed,
        });
    }
    None
}

/// SimpMusic (`simpmusic`) RichSync Word-Synced Lyrics
fn try_fetch_simpmusic(clean_title: &str, artist: &str) -> Option<TrackLyricsState> {
    let query = format!("{} {}", clean_title, artist);
    if let Ok(out) = Command::new("curl")
        .args([
            "-4",
            "--connect-timeout",
            "2",
            "--max-time",
            "3",
            "-sG",
            "-A",
            "SimpMusic/0.2.12 (Android)",
            "https://api-lyrics.simpmusic.org/v1/search",
            "--data-urlencode",
            &format!("q={}", query),
        ])
        .output()
    {
        if out.status.success() {
            if let Ok(resp) = serde_json::from_slice::<SimpMusicResp>(&out.stdout) {
                if let Some(items) = resp.data {
                    for item in items {
                        if let Some(ref rich) = item.rich_sync_lyrics {
                            let parsed = parse_enhanced_lrc(rich);
                            if parsed.len() >= 3 {
                                return Some(TrackLyricsState::Synced {
                                    provider_badge: "SIMP".to_string(),
                                    provider_detail: format!(
                                        "SimpMusic RichSync (Per-Kata) - {} Baris",
                                        parsed.len()
                                    ),
                                    accent_rgb: provider_accent("simpmusic"),
                                    lines: parsed,
                                });
                            }
                        }
                        if let Some(ref synced) = item.synced_lyrics {
                            let parsed = parse_enhanced_lrc(synced);
                            if parsed.len() >= 3 {
                                return Some(TrackLyricsState::Synced {
                                    provider_badge: "SIMP".to_string(),
                                    provider_detail: format!(
                                        "SimpMusic (Per-Kata) - {} Baris",
                                        parsed.len()
                                    ),
                                    accent_rgb: provider_accent("simpmusic"),
                                    lines: parsed,
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// LrcLib (`lrclib`) Enhanced `<mm:ss.xx>` Word-Sync
fn try_fetch_lrclib_enhanced(clean_title: &str, artist: &str) -> Option<TrackLyricsState> {
    let query = format!("{} {}", clean_title, artist);
    let out = Command::new("curl")
        .args([
            "-4",
            "--connect-timeout",
            "2",
            "--max-time",
            "3",
            "-sG",
            "https://lrclib.net/api/search",
            "--data-urlencode",
            &format!("q={}", query),
        ])
        .output()
        .ok()?;

    if out.status.success() {
        if let Ok(items) = serde_json::from_slice::<Vec<LrcLibItem>>(&out.stdout) {
            for item in &items {
                if let Some(ref synced) = item.synced_lyrics {
                    let parsed = parse_enhanced_lrc(synced);
                    if parsed.len() >= 3 {
                        return Some(TrackLyricsState::Synced {
                            provider_badge: "LRCLIB".to_string(),
                            provider_detail: format!(
                                "LrcLib Enhanced (Per-Kata) - {} Baris",
                                parsed.len()
                            ),
                            accent_rgb: provider_accent("lrclib"),
                            lines: parsed,
                        });
                    }
                }
            }
        }
    }
    None
}

/// NetEase (`netease`) 163 YRC & K-Lyric Word-by-Word
fn try_fetch_netease(clean_title: &str, artist: &str) -> Option<TrackLyricsState> {
    let query = format!("{} {}", clean_title, artist);
    let search_out = Command::new("curl")
        .args([
            "-4",
            "--connect-timeout",
            "2",
            "--max-time",
            "3",
            "-sG",
            "https://music.163.com/api/search/get",
            "--data-urlencode",
            &format!("s={}", query),
            "-d",
            "type=1&limit=2",
        ])
        .output()
        .ok()?;

    if !search_out.status.success() {
        return None;
    }
    let resp = serde_json::from_slice::<NetEaseSearchResp>(&search_out.stdout).ok()?;
    let songs = resp.result.and_then(|r| r.songs)?;

    for song in songs.into_iter().take(2) {
        let lyr_out = Command::new("curl")
            .args([
                "-4",
                "--connect-timeout",
                "2",
                "--max-time",
                "3",
                "-s",
                &format!(
                    "https://music.163.com/api/song/lyric?os=pc&id={}&lv=-1&kv=-1&tv=-1&yv=-1",
                    song.id
                ),
            ])
            .output()
            .ok()?;

        if !lyr_out.status.success() {
            continue;
        }
        if let Ok(lresp) = serde_json::from_slice::<NetEaseLyricResp>(&lyr_out.stdout) {
            if let Some(yrc_str) = lresp.yrc.and_then(|y| y.lyric) {
                let y_parsed = parse_netease_yrc(&yrc_str);
                if y_parsed.len() >= 4 {
                    return Some(TrackLyricsState::Synced {
                        provider_badge: "NETEASE".to_string(),
                        provider_detail: format!(
                            "NetEase YRC (Per-Kata) - {} Baris",
                            y_parsed.len()
                        ),
                        accent_rgb: provider_accent("netease"),
                        lines: y_parsed,
                    });
                }
            }
            if let Some(k_str) = lresp.klyric.and_then(|k| k.lyric) {
                let k_parsed = parse_netease_yrc(&k_str);
                if k_parsed.len() >= 4 {
                    return Some(TrackLyricsState::Synced {
                        provider_badge: "NETEASE".to_string(),
                        provider_detail: format!(
                            "NetEase K-Lyric (Per-Kata) - {} Baris",
                            k_parsed.len()
                        ),
                        accent_rgb: provider_accent("netease"),
                        lines: k_parsed,
                    });
                }
            }
        }
    }
    None
}
