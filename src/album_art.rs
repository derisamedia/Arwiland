use image::imageops::FilterType;
use serde::Deserialize;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

pub const ALBUM_ART_SIZE: u32 = 58;
const ALBUM_ART_RADIUS: f32 = 14.0;
const MAX_RAW_BYTES: usize = 6 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct AlbumArt {
    pub size: u32,
    /// Premultiplied RGBA8 pixels with anti-aliased rounded-square (squircle) mask baked in.
    pub premul_rgba: Vec<u8>,
}

fn silent_cmd(prog: &str) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(prog);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd
}

/// Decodes JPEG/PNG/WebP bytes, center-crops to square, downscales to 58x58,
/// and bakes an anti-aliased r=14 rounded-square alpha mask + subtle 1px inner border.
pub fn decode_to_squircle(raw_bytes: &[u8]) -> Option<Arc<AlbumArt>> {
    if raw_bytes.is_empty() || raw_bytes.len() > MAX_RAW_BYTES {
        return None;
    }
    let img = image::load_from_memory(raw_bytes).ok()?;
    let w = img.width();
    let h = img.height();
    if w == 0 || h == 0 {
        return None;
    }

    let side = w.min(h);
    let crop_x = (w - side) / 2;
    let crop_y = (h - side) / 2;
    let cropped = img.crop_imm(crop_x, crop_y, side, side);
    let resized = cropped.resize_exact(ALBUM_ART_SIZE, ALBUM_ART_SIZE, FilterType::Triangle);
    let rgba = resized.to_rgba8();

    let sz = ALBUM_ART_SIZE;
    let half = sz as f32 * 0.5;
    let r = ALBUM_ART_RADIUS;
    let mut premul = vec![0u8; (sz * sz * 4) as usize];

    for py in 0..sz {
        for px in 0..sz {
            let cx = px as f32 + 0.5 - half;
            let cy = py as f32 + 0.5 - half;
            let qx = cx.abs() - (half - r);
            let qy = cy.abs() - (half - r);
            let dist = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r;
            let mask_a = (0.5 - dist).clamp(0.0, 1.0);
            if mask_a <= 0.001 {
                continue;
            }

            let p = rgba.get_pixel(px, py);
            let mut fr = p[0] as f32;
            let mut fg = p[1] as f32;
            let mut fb = p[2] as f32;
            let src_a = (p[3] as f32 / 255.0) * mask_a;

            // Subtle 1px inner glass rim so dark album covers stay cleanly framed on #111114
            if dist >= -1.25 && dist < 0.0 {
                let rim = 0.12;
                fr = fr * (1.0 - rim) + 255.0 * rim;
                fg = fg * (1.0 - rim) + 255.0 * rim;
                fb = fb * (1.0 - rim) + 255.0 * rim;
            }

            let idx = ((py * sz + px) * 4) as usize;
            premul[idx] = (fr * src_a + 0.5).clamp(0.0, 255.0) as u8;
            premul[idx + 1] = (fg * src_a + 0.5).clamp(0.0, 255.0) as u8;
            premul[idx + 2] = (fb * src_a + 0.5).clamp(0.0, 255.0) as u8;
            premul[idx + 3] = (src_a * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
        }
    }

    Some(Arc::new(AlbumArt {
        size: sz,
        premul_rgba: premul,
    }))
}

#[derive(Deserialize)]
struct ItunesResp {
    results: Option<Vec<ItunesItem>>,
}

#[derive(Deserialize)]
struct ItunesItem {
    #[serde(rename = "artworkUrl100")]
    artwork_url_100: Option<String>,
}

fn fetch_url_bytes(url: &str) -> Option<Vec<u8>> {
    let out = silent_cmd("curl")
        .args([
            "-4",
            "--connect-timeout",
            "2",
            "--max-time",
            "4",
            "-sL",
            "--max-filesize",
            "6291456",
            url,
        ])
        .output()
        .ok()?;
    if out.status.success() && !out.stdout.is_empty() && out.stdout.len() <= MAX_RAW_BYTES {
        Some(out.stdout)
    } else {
        None
    }
}

fn clean_query_title(raw: &str) -> String {
    let mut s = raw.to_string();
    for (open, close) in [('(', ')'), ('[', ']')] {
        while let (Some(a), Some(b)) = (s.find(open), s.find(close)) {
            if b > a {
                s.replace_range(a..=b, "");
            } else {
                break;
            }
        }
    }
    let t = s.trim().trim_matches('-').trim();
    if t.is_empty() {
        raw.trim().to_string()
    } else {
        t.to_string()
    }
}

fn fetch_itunes_fallback(title: &str, artist: &str) -> Option<Arc<AlbumArt>> {
    let ct = clean_query_title(title);
    if ct.is_empty()
        || ct.eq_ignore_ascii_case("Media Aktif")
        || ct.eq_ignore_ascii_case("Tidak ada media diputar")
    {
        return None;
    }
    let clean_artist = match artist.to_lowercase().as_str() {
        "google chrome" | "microsoft edge" | "firefox" | "brave" | "opera" | "spotify"
        | "vlc media" | "media player" | "media" => "",
        _ => artist.trim(),
    };
    let term = if clean_artist.is_empty() {
        ct
    } else {
        format!("{} {}", ct, clean_artist)
    };

    let out = silent_cmd("curl")
        .args([
            "-4",
            "--connect-timeout",
            "2",
            "--max-time",
            "3",
            "-sG",
            "https://itunes.apple.com/search",
            "--data-urlencode",
            &format!("term={}", term),
            "-d",
            "entity=song&limit=1",
        ])
        .output()
        .ok()?;
    if !out.status.success() || out.stdout.is_empty() {
        return None;
    }
    let resp: ItunesResp = serde_json::from_slice(&out.stdout).ok()?;
    let art_url = resp
        .results?
        .into_iter()
        .find_map(|item| item.artwork_url_100)?;
    let hi_url = art_url.replace("100x100bb", "200x200bb");
    let bytes = fetch_url_bytes(&hi_url).or_else(|| fetch_url_bytes(&art_url))?;
    decode_to_squircle(&bytes)
}

fn load_art_blocking(source: Option<&str>, title: &str, artist: &str) -> Option<Arc<AlbumArt>> {
    if let Some(src) = source {
        let trimmed = src.trim();
        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            if let Some(bytes) = fetch_url_bytes(trimmed) {
                if let Some(art) = decode_to_squircle(&bytes) {
                    return Some(art);
                }
            }
        } else if !trimmed.is_empty() {
            if let Ok(bytes) = std::fs::read(trimmed) {
                if let Some(art) = decode_to_squircle(&bytes) {
                    return Some(art);
                }
            }
        }
    }
    fetch_itunes_fallback(title, artist)
}

struct ArtSlot {
    key: String,
    art: Option<Arc<AlbumArt>>,
}

/// Ultra-lightweight single-track async album art loader (~13 KB RAM).
pub struct AlbumArtManager {
    slot: Arc<Mutex<ArtSlot>>,
    seq: Arc<AtomicU64>,
}

impl AlbumArtManager {
    pub fn new() -> Self {
        Self {
            slot: Arc::new(Mutex::new(ArtSlot {
                key: String::new(),
                art: None,
            })),
            seq: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn seq(&self) -> u64 {
        self.seq.load(Ordering::Acquire)
    }

    /// Synchronizes current track album art without ever blocking the UI thread.
    pub fn resolve(
        &self,
        has_player: bool,
        title: &str,
        artist: &str,
        art_source: Option<&str>,
        predecoded: Option<&Arc<AlbumArt>>,
    ) -> Option<Arc<AlbumArt>> {
        if !has_player {
            if let Ok(mut g) = self.slot.lock() {
                if !g.key.is_empty() || g.art.is_some() {
                    g.key.clear();
                    g.art = None;
                }
            }
            return None;
        }

        if let Some(art) = predecoded {
            return Some(Arc::clone(art));
        }

        let key = format!(
            "{}||{}||{}",
            title.trim(),
            artist.trim(),
            art_source.unwrap_or("")
        );

        let mut guard = self.slot.lock().ok()?;
        if guard.key == key {
            return guard.art.clone();
        }

        guard.key = key.clone();
        guard.art = None;
        drop(guard);

        let slot_clone = Arc::clone(&self.slot);
        let seq_clone = Arc::clone(&self.seq);
        let src_owned = art_source.map(|s| s.to_string());
        let title_owned = title.to_string();
        let artist_owned = artist.to_string();

        thread::spawn(move || {
            let loaded = load_art_blocking(src_owned.as_deref(), &title_owned, &artist_owned);
            if let Ok(mut g) = slot_clone.lock() {
                if g.key == key {
                    g.art = loaded;
                    seq_clone.fetch_add(1, Ordering::Release);
                }
            }
        });

        None
    }
}
