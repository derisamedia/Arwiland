#[cfg(not(target_os = "windows"))]
use fontdue::{Font, FontSettings, Metrics};
use std::collections::HashMap;
use std::f32::consts::PI;
use tiny_skia::{
    Color, FillRule, GradientStop, LineCap, LinearGradient, Paint, Path, PathBuilder, Pixmap,
    Point, SpreadMode, Stroke, Transform,
};

use crate::album_art::AlbumArt;
use crate::lyrics::lyric_provider_label;
use crate::notifications::IslandNotification;
use crate::runtime::{ActiveTab, InputField, RuntimeContext, CANVAS_WIDTH, COMPACT_H};
use crate::state::{resolve_accent_palette, ACCENT_PRESETS};

#[derive(Clone, Copy, Default, Debug)]
pub struct HitRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl HitRect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn contains(&self, px: f32, py: f32) -> bool {
        self.w > 0.0
            && self.h > 0.0
            && px >= self.x
            && px <= self.x + self.w
            && py >= self.y
            && py <= self.y + self.h
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiAction {
    SelectTab(ActiveTab),
    ToggleStartup,
    ToggleSettings,
    TogglePin,
    Quit,
    MediaPrev,
    MediaPlayPause,
    MediaNext,
    MediaSeekBar,
    VolMute,
    VolBar,
    NotifTest,
    NotifMarkRead,
    NotifClear,
    NotifRemove(u64),
    PomoAdjust(i32),
    PomoStartPause,
    PomoReset,
    PomoToggleMode,
    AlarmAdjustHour(i32),
    AlarmAdjustMin(i32),
    AlarmToggle,
    AlarmSnooze5,
    FocusInput(InputField),
    TodoAdd,
    TodoToggle(usize),
    TodoDelete(usize),
    ShelfPickFile,
    ShelfClear,
    ShelfOpen(usize),
    ShelfRemove(usize),
    PrayerCycleCity,
    PrayerSearchSubmit,
    SettingCycleIslandStyle,
    SettingTogglePosition,
    SettingCycleAnimPreset,
    SettingToggleHoverExpand,
    SettingCycleLyricProvider(i32),
    SettingCycleLyricStyle,
    SettingCompactWidth(i32),
    SettingYOffset(i32),
    SettingLyricOffset(i32),
    SettingToggleNotif,
    SettingSelectAccent(usize),
}

#[derive(Default)]
pub struct UiHitMap {
    pub pill: HitRect,
    pub seek_bar: HitRect,
    pub vol_bar: HitRect,
    pub items: Vec<(HitRect, UiAction)>,
}

impl UiHitMap {
    pub fn clear(&mut self) {
        self.pill = HitRect::default();
        self.seek_bar = HitRect::default();
        self.vol_bar = HitRect::default();
        self.items.clear();
    }

    pub fn push(&mut self, rect: HitRect, action: UiAction) {
        self.items.push((rect, action));
    }

    pub fn hit_test(&self, x: f32, y: f32) -> Option<UiAction> {
        for (r, act) in self.items.iter().rev() {
            if r.contains(x, y) {
                return Some(*act);
            }
        }
        None
    }
}

struct CachedGlyph {
    width: usize,
    height: usize,
    xmin: i32,
    ymin: i32,
    advance_width: f32,
    bitmap: Vec<u8>,
}

#[cfg(not(target_os = "windows"))]
struct FallbackFontData {
    #[cfg(target_os = "linux")]
    ptr: *const u8,
    #[cfg(target_os = "linux")]
    len: usize,
    #[cfg(not(target_os = "linux"))]
    bytes: Vec<u8>,
}

#[cfg(not(target_os = "windows"))]
impl FallbackFontData {
    fn open(path: &str) -> Option<Self> {
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::io::AsRawFd;
            let file = std::fs::File::open(path).ok()?;
            let len = file.metadata().ok()?.len() as usize;
            if len < 64 {
                return None;
            }
            let ptr = unsafe {
                libc::mmap(
                    std::ptr::null_mut(),
                    len,
                    libc::PROT_READ,
                    libc::MAP_PRIVATE,
                    file.as_raw_fd(),
                    0,
                )
            };
            if ptr == libc::MAP_FAILED || ptr.is_null() {
                return None;
            }
            Some(Self {
                ptr: ptr as *const u8,
                len,
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            let bytes = std::fs::read(path).ok()?;
            if bytes.len() < 64 {
                return None;
            }
            Some(Self { bytes })
        }
    }

    fn as_slice(&self) -> &[u8] {
        #[cfg(target_os = "linux")]
        unsafe {
            std::slice::from_raw_parts(self.ptr, self.len)
        }
        #[cfg(not(target_os = "linux"))]
        {
            &self.bytes
        }
    }
}

#[cfg(all(not(target_os = "windows"), target_os = "linux"))]
impl Drop for FallbackFontData {
    fn drop(&mut self) {
        if !self.ptr.is_null() && self.len > 0 {
            unsafe {
                libc::munmap(self.ptr as *mut libc::c_void, self.len);
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
struct SkiaOutline {
    pb: PathBuilder,
    scale: f32,
    off_x: f32,
    off_y: f32,
}

#[cfg(not(target_os = "windows"))]
impl ttf_parser::OutlineBuilder for SkiaOutline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.pb.move_to(x * self.scale + self.off_x, self.off_y - y * self.scale);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.pb.line_to(x * self.scale + self.off_x, self.off_y - y * self.scale);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.pb.quad_to(
            x1 * self.scale + self.off_x,
            self.off_y - y1 * self.scale,
            x * self.scale + self.off_x,
            self.off_y - y * self.scale,
        );
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.pb.cubic_to(
            x1 * self.scale + self.off_x,
            self.off_y - y1 * self.scale,
            x2 * self.scale + self.off_x,
            self.off_y - y2 * self.scale,
            x * self.scale + self.off_x,
            self.off_y - y * self.scale,
        );
    }
    fn close(&mut self) {
        self.pb.close();
    }
}

#[cfg(not(target_os = "windows"))]
pub struct FontEngine {
    regular: Font,
    bold: Font,
    /// Lazy mmap-backed CJK/Unicode fallback font (0 bytes RAM until CJK glyph is drawn!)
    cjk_fallback: Option<FallbackFontData>,
    cjk_loaded: bool,
    /// Key: (bold, size_half_px, subpixel_phase_0_or_1, char)
    cache: HashMap<(bool, u16, u8, char), CachedGlyph>,
    /// Precomputed gamma-compensated coverage LUT (0..=255 -> 0.0..=1.0) for silky-smooth light-on-dark AA
    gamma_lut: [f32; 256],
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum WinFontFace {
    Primary, // Segoe UI
    Jp,      // Yu Gothic UI (Hiragana, Katakana, Kanji)
    Zh,      // Microsoft YaHei UI (Chinese Hanzi, CJK Unified Ideographs)
    Kr,      // Malgun Gothic (Korean Hangul)
}

#[cfg(target_os = "windows")]
fn pick_win_font_face(ch: char) -> WinFontFace {
    match ch as u32 {
        // Hiragana, Katakana, Katakana Phonetic Extensions, Halfwidth Katakana
        0x3040..=0x30FF | 0x31F0..=0x31FF | 0xFF65..=0xFF9F => WinFontFace::Jp,
        // Hangul Syllables, Jamo
        0xAC00..=0xD7AF | 0x1100..=0x11FF | 0x3130..=0x318F => WinFontFace::Kr,
        // CJK Radicals, Symbols & Punctuation, Bopomofo, CJK Unified Ideographs, Compatibility, Fullwidth
        0x2E80..=0x303F | 0x3100..=0x9FFF | 0xF900..=0xFAFF | 0xFF00..=0xFFEF => WinFontFace::Zh,
        _ => WinFontFace::Primary,
    }
}

#[cfg(target_os = "windows")]
pub struct FontEngine {
    hdc: windows::Win32::Graphics::Gdi::HDC,
    orig_font: windows::Win32::Graphics::Gdi::HGDIOBJ,
    hfonts: HashMap<(WinFontFace, bool, u16), windows::Win32::Graphics::Gdi::HFONT>,
    scratch_buf: Vec<u8>,
    reg_cap_ratio: f32,
    bold_cap_ratio: f32,
    /// Key: (bold, size_half_px, subpixel_phase_0_or_1, char)
    cache: HashMap<(bool, u16, u8, char), CachedGlyph>,
    /// Precomputed gamma-compensated coverage LUT (0..=255 -> 0.0..=1.0) for silky-smooth light-on-dark AA
    gamma_lut: [f32; 256],
}

#[cfg(target_os = "windows")]
impl Drop for FontEngine {
    fn drop(&mut self) {
        use windows::Win32::Graphics::Gdi::{DeleteDC, DeleteObject, SelectObject};
        unsafe {
            if !self.hdc.is_invalid() {
                if !self.orig_font.is_invalid() {
                    SelectObject(self.hdc, self.orig_font);
                }
                for (_, hf) in self.hfonts.drain() {
                    if !hf.is_invalid() {
                        let _ = DeleteObject(hf);
                    }
                }
                let _ = DeleteDC(self.hdc);
            }
        }
    }
}

fn build_gamma_lut() -> [f32; 256] {
    let mut gamma_lut = [0.0f32; 256];
    for (i, slot) in gamma_lut.iter_mut().enumerate() {
        let lin = i as f32 / 255.0;
        *slot = lin.powf(0.76);
    }
    gamma_lut
}

#[cfg(not(target_os = "windows"))]
impl FontEngine {
    pub fn load() -> Result<Self, String> {
        // Exact original GNOME UI font (Cantarell 98 KB) + clean modern Linux fallbacks
        let regular_candidates = [
            "/usr/share/fonts/abattis-cantarell-fonts/Cantarell-Regular.otf",
            "/usr/share/fonts/cantarell/Cantarell-Regular.otf",
            "/usr/share/fonts/open-sans/OpenSans-Regular.ttf",
            "/usr/share/fonts/inter/Inter-Regular.ttf",
            "/usr/share/fonts/liberation-sans-fonts/LiberationSans-Regular.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            "/usr/share/fonts/google-droid-sans-fonts/DroidSans.ttf",
            "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/adwaita-sans-fonts/AdwaitaSans-Regular.ttf",
        ];
        let bold_candidates = [
            "/usr/share/fonts/abattis-cantarell-fonts/Cantarell-Bold.otf",
            "/usr/share/fonts/cantarell/Cantarell-Bold.otf",
            "/usr/share/fonts/open-sans/OpenSans-Semibold.ttf",
            "/usr/share/fonts/open-sans/OpenSans-Bold.ttf",
            "/usr/share/fonts/inter/Inter-SemiBold.ttf",
            "/usr/share/fonts/inter/Inter-Bold.ttf",
            "/usr/share/fonts/liberation-sans-fonts/LiberationSans-Bold.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
            "/usr/share/fonts/google-droid-sans-fonts/DroidSans-Bold.ttf",
            "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans-Bold.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
            "/usr/share/fonts/adwaita-sans-fonts/AdwaitaSans-Regular.ttf",
        ];

        let reg_bytes = regular_candidates
            .iter()
            .find_map(|p| std::fs::read(p).ok())
            .ok_or_else(|| "No system sans-serif font found".to_string())?;
        let bold_bytes = bold_candidates
            .iter()
            .find_map(|p| std::fs::read(p).ok())
            .unwrap_or_else(|| reg_bytes.clone());

        let settings = FontSettings {
            scale: 28.0,
            load_substitutions: false,
            ..FontSettings::default()
        };
        let regular = Font::from_bytes(reg_bytes, settings)
            .map_err(|e| format!("Failed to parse regular font: {e}"))?;
        let bold = Font::from_bytes(bold_bytes, settings)
            .map_err(|e| format!("Failed to parse bold font: {e}"))?;

        Ok(Self {
            regular,
            bold,
            cjk_fallback: None,
            cjk_loaded: false,
            cache: HashMap::with_capacity(256),
            gamma_lut: build_gamma_lut(),
        })
    }

    fn ensure_cjk_fallback(&mut self) {
        if self.cjk_loaded {
            return;
        }
        self.cjk_loaded = true;
        let candidates = [
            "/usr/share/fonts/google-droid-sans-fonts/DroidSansFallbackFull.ttf",
            "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/google-noto-sans-cjk-vf-fonts/NotoSansCJK-VF.ttc",
            "/usr/share/fonts/wqy-microhei/wqy-microhei.ttc",
            "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
            "/usr/share/fonts/wqy-zenhei/wqy-zenhei.ttc",
            "/usr/share/fonts/google-droid-sans-fonts/DroidSansJapanese.ttf",
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/Hiragino Sans GB.ttc",
        ];
        self.cjk_fallback = candidates.iter().find_map(|p| FallbackFontData::open(p));
    }

    fn rasterize_fallback_cjk(&self, size: f32, phase: u8, ch: char) -> Option<CachedGlyph> {
        let fb = self.cjk_fallback.as_ref()?;
        let face = ttf_parser::Face::parse(fb.as_slice(), 0).ok()?;
        let gid = face.glyph_index(ch)?;
        let upem = face.units_per_em() as f32;
        if upem <= 1.0 {
            return None;
        }
        let scale = size / upem;
        let adv = face
            .glyph_hor_advance(gid)
            .map(|a| a as f32 * scale)
            .unwrap_or(size);
        let Some(bbox) = face.glyph_bounding_box(gid) else {
            return Some(CachedGlyph {
                width: 0,
                height: 0,
                xmin: 0,
                ymin: 0,
                advance_width: adv,
                bitmap: Vec::new(),
            });
        };

        let shift_x = if phase == 1 { 0.5f32 } else { 0.0f32 };
        let xmin = (bbox.x_min as f32 * scale + shift_x).floor() as i32;
        let ymin = (bbox.y_min as f32 * scale).floor() as i32;
        let xmax = (bbox.x_max as f32 * scale + shift_x).ceil() as i32;
        let ymax = (bbox.y_max as f32 * scale).ceil() as i32;
        let w = (xmax - xmin).clamp(1, 128) as usize;
        let h = (ymax - ymin).clamp(1, 128) as usize;

        let mut outline = SkiaOutline {
            pb: PathBuilder::new(),
            scale,
            off_x: shift_x - xmin as f32,
            off_y: ymax as f32,
        };
        face.outline_glyph(gid, &mut outline)?;
        let path = outline.pb.finish()?;
        let mut pm = Pixmap::new(w as u32, h as u32)?;
        let paint = paint_rgba(255, 255, 255, 255);
        pm.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);

        let mut bmp = Vec::with_capacity(w * h);
        for px in pm.data().chunks_exact(4) {
            bmp.push(px[3]);
        }
        Some(CachedGlyph {
            width: w,
            height: h,
            xmin,
            ymin,
            advance_width: adv,
            bitmap: bmp,
        })
    }

    /// 2x supersampled glyph rasterization with 2-phase horizontal subpixel alignment (0.0 / 0.5 px)
    fn rasterize_2x(font: &Font, size: f32, phase: u8, ch: char) -> CachedGlyph {
        let m1: Metrics = font.metrics(ch, size);
        let (m2, bmp2) = font.rasterize(ch, size * 2.0);
        if m2.width == 0 || m2.height == 0 {
            return CachedGlyph {
                width: 0,
                height: 0,
                xmin: m1.xmin,
                ymin: m1.ymin,
                advance_width: m1.advance_width,
                bitmap: Vec::new(),
            };
        }

        // Shift by +1 subpixel (0.5 physical px) in 2x space when phase == 1
        let shift_x = if phase == 1 { 1isize } else { 0isize };
        let x2_min = m2.xmin as isize + shift_x;
        let y2_min = m2.ymin as isize;
        let x2_max = x2_min + m2.width as isize;
        let y2_max = y2_min + m2.height as isize;

        // Align downsampled 1x grid to even 2x coordinates
        let gx0 = x2_min.div_euclid(2);
        let gy0 = y2_min.div_euclid(2);
        let gx1 = (x2_max + 1).div_euclid(2);
        let gy1 = (y2_max + 1).div_euclid(2);

        let w1 = (gx1 - gx0).max(0) as usize;
        let h1 = (gy1 - gy0).max(0) as usize;
        let mut out = vec![0u8; w1 * h1];

        let sample2 = |sx: isize, sy_from_bottom: isize| -> u32 {
            let rx = sx - x2_min;
            let ry_from_bottom = sy_from_bottom - y2_min;
            if rx < 0 || rx >= m2.width as isize || ry_from_bottom < 0 || ry_from_bottom >= m2.height as isize {
                return 0;
            }
            let row = (m2.height as isize - 1 - ry_from_bottom) as usize;
            bmp2[row * m2.width + rx as usize] as u32
        };

        for r1 in 0..h1 {
            // r1 = 0 is top row in output bitmap -> corresponds to gy = gy1 - 1 - r1
            let gy = gy1 - 1 - r1 as isize;
            let sy_base = gy * 2;
            for c1 in 0..w1 {
                let gx = gx0 + c1 as isize;
                let sx_base = gx * 2;
                let sum = sample2(sx_base, sy_base)
                    + sample2(sx_base + 1, sy_base)
                    + sample2(sx_base, sy_base + 1)
                    + sample2(sx_base + 1, sy_base + 1);
                out[r1 * w1 + c1] = ((sum + 2) / 4).min(255) as u8;
            }
        }

        CachedGlyph {
            width: w1,
            height: h1,
            xmin: gx0 as i32,
            ymin: gy0 as i32,
            advance_width: m1.advance_width,
            bitmap: out,
        }
    }

    fn glyph(&mut self, bold: bool, size: f32, phase: u8, ch: char) -> &CachedGlyph {
        let size_key = (size * 2.0).round() as u16;
        let q_size = size_key as f32 * 0.5;
        if self.cache.len() > 1200 {
            self.cache.clear();
        }
        let key = (bold, size_key, phase, ch);
        if !self.cache.contains_key(&key) {
            let has_in_primary = {
                let font = if bold { &self.bold } else { &self.regular };
                font.lookup_glyph_index(ch) != 0
            };
            let g = if !has_in_primary && ch != ' ' && ch != '\t' && ch != '\n' && ch != '\r' {
                self.ensure_cjk_fallback();
                if let Some(fb_g) = self.rasterize_fallback_cjk(q_size, phase, ch) {
                    fb_g
                } else {
                    let font = if bold { &self.bold } else { &self.regular };
                    Self::rasterize_2x(font, q_size, phase, ch)
                }
            } else {
                let font = if bold { &self.bold } else { &self.regular };
                Self::rasterize_2x(font, q_size, phase, ch)
            };
            self.cache.insert(key, g);
        }
        self.cache.get(&key).unwrap()
    }

    fn kern(&self, bold: bool, prev: Option<char>, ch: char, size: f32) -> f32 {
        let Some(p) = prev else { return 0.0 };
        let font = if bold { &self.bold } else { &self.regular };
        font.horizontal_kern(p, ch, size).unwrap_or(0.0)
    }

    /// Exact baseline Y to vertically center text around `center_y`
    pub fn centered_baseline(&self, center_y: f32, size: f32, bold: bool) -> f32 {
        let font = if bold { &self.bold } else { &self.regular };
        let cap_h = font.metrics('H', size).height as f32;
        if cap_h > 2.0 {
            center_y + cap_h * 0.5
        } else {
            center_y + size * 0.36
        }
    }
}

#[cfg(target_os = "windows")]
impl FontEngine {
    fn create_gdi_font(
        face: WinFontFace,
        bold: bool,
        height_2x: i32,
    ) -> windows::Win32::Graphics::Gdi::HFONT {
        use windows::core::w;
        use windows::Win32::Graphics::Gdi::{
            CreateFontW, ANTIALIASED_QUALITY, CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DEFAULT_PITCH,
            FF_SWISS, FW_BOLD, FW_NORMAL, OUT_TT_PRECIS,
        };
        let weight = if bold {
            FW_BOLD.0 as i32
        } else {
            FW_NORMAL.0 as i32
        };
        let face_name = match face {
            WinFontFace::Primary => w!("Segoe UI"),
            WinFontFace::Jp => w!("Yu Gothic UI"),
            WinFontFace::Zh => w!("Microsoft YaHei UI"),
            WinFontFace::Kr => w!("Malgun Gothic"),
        };
        unsafe {
            CreateFontW(
                -height_2x.max(2),
                0,
                0,
                0,
                weight,
                0,
                0,
                0,
                DEFAULT_CHARSET.0 as u32,
                OUT_TT_PRECIS.0 as u32,
                CLIP_DEFAULT_PRECIS.0 as u32,
                ANTIALIASED_QUALITY.0 as u32,
                (DEFAULT_PITCH.0 | FF_SWISS.0) as u32,
                face_name,
            )
        }
    }

    fn identity_mat2() -> windows::Win32::Graphics::Gdi::MAT2 {
        use windows::Win32::Graphics::Gdi::{FIXED, MAT2};
        MAT2 {
            eM11: FIXED { fract: 0, value: 1 },
            eM12: FIXED { fract: 0, value: 0 },
            eM21: FIXED { fract: 0, value: 0 },
            eM22: FIXED { fract: 0, value: 1 },
        }
    }

    fn measure_cap_ratio(hdc: windows::Win32::Graphics::Gdi::HDC, bold: bool) -> f32 {
        use windows::Win32::Graphics::Gdi::{
            DeleteObject, GetGlyphOutlineW, SelectObject, GDI_ERROR, GGO_METRICS, GLYPHMETRICS,
        };
        unsafe {
            let hf = Self::create_gdi_font(WinFontFace::Primary, bold, 80);
            if hf.is_invalid() {
                return 0.72;
            }
            let prev = SelectObject(hdc, hf);
            let mut gm = GLYPHMETRICS::default();
            let mat = Self::identity_mat2();
            let rc = GetGlyphOutlineW(hdc, 'H' as u32, GGO_METRICS, &mut gm, 0, None, &mat);
            SelectObject(hdc, prev);
            let _ = DeleteObject(hf);
            if rc != GDI_ERROR as u32 && gm.gmBlackBoxY > 10 {
                (gm.gmBlackBoxY as f32 / 80.0).clamp(0.55, 0.85)
            } else {
                0.72
            }
        }
    }

    pub fn load() -> Result<Self, String> {
        use windows::Win32::Graphics::Gdi::{CreateCompatibleDC, HDC, HGDIOBJ};
        let hdc = unsafe { CreateCompatibleDC(HDC::default()) };
        if hdc.is_invalid() {
            return Err("CreateCompatibleDC failed for FontEngine".to_string());
        }
        let reg_cap_ratio = Self::measure_cap_ratio(hdc, false);
        let bold_cap_ratio = Self::measure_cap_ratio(hdc, true);

        Ok(Self {
            hdc,
            orig_font: HGDIOBJ::default(),
            hfonts: HashMap::with_capacity(24),
            scratch_buf: Vec::with_capacity(2048),
            reg_cap_ratio,
            bold_cap_ratio,
            cache: HashMap::with_capacity(256),
            gamma_lut: build_gamma_lut(),
        })
    }

    fn select_hfont(&mut self, face: WinFontFace, bold: bool, size_key: u16) {
        use windows::Win32::Graphics::Gdi::{DeleteObject, SelectObject};
        if self.hfonts.len() > 64 {
            unsafe {
                if !self.orig_font.is_invalid() {
                    SelectObject(self.hdc, self.orig_font);
                }
                for (_, hf) in self.hfonts.drain() {
                    if !hf.is_invalid() {
                        let _ = DeleteObject(hf);
                    }
                }
            }
        }
        let height_2x = (size_key as i32).max(2); // size_key = round(size * 2.0) == exact 2x px height!
        let hf = *self
            .hfonts
            .entry((face, bold, size_key))
            .or_insert_with(|| Self::create_gdi_font(face, bold, height_2x));
        if !hf.is_invalid() {
            unsafe {
                let prev = SelectObject(self.hdc, hf);
                if self.orig_font.is_invalid() {
                    self.orig_font = prev;
                }
            }
        }
    }

    /// Zero-TTF-heap native Win32 GDI 2x supersampled glyph rasterizer (`GetGlyphOutlineW` + `GGO_GRAY8_BITMAP`),
    /// with lazy CJK fallback (`Yu Gothic UI` / `Microsoft YaHei UI` / `Malgun Gothic`).
    fn rasterize_2x_gdi(&mut self, bold: bool, size_key: u16, phase: u8, ch: char) -> CachedGlyph {
        use windows::Win32::Foundation::SIZE;
        use windows::Win32::Graphics::Gdi::{
            GetGlyphOutlineW, GetTextExtentPoint32W, GDI_ERROR, GGO_GRAY8_BITMAP, GLYPHMETRICS,
        };

        let primary_face = pick_win_font_face(ch);
        self.select_hfont(primary_face, bold, size_key);
        let code = if (ch as u32) <= 0xFFFF {
            ch as u32
        } else {
            '?' as u32
        };
        let mut gm = GLYPHMETRICS::default();
        let mat = Self::identity_mat2();

        let mut buf_size = unsafe {
            GetGlyphOutlineW(self.hdc, code, GGO_GRAY8_BITMAP, &mut gm, 0, None, &mat)
        };
        // Cross-fallback between JP (Yu Gothic UI) and ZH (Microsoft YaHei UI) if one pack isn't installed
        if (buf_size == GDI_ERROR as u32 || (buf_size == 0 && !ch.is_whitespace()))
            && matches!(primary_face, WinFontFace::Jp | WinFontFace::Zh)
        {
            let alt = if primary_face == WinFontFace::Jp {
                WinFontFace::Zh
            } else {
                WinFontFace::Jp
            };
            self.select_hfont(alt, bold, size_key);
            buf_size = unsafe {
                GetGlyphOutlineW(self.hdc, code, GGO_GRAY8_BITMAP, &mut gm, 0, None, &mat)
            };
        }
        if buf_size == GDI_ERROR as u32 {
            let u16_ch = [code as u16];
            let mut sz = SIZE::default();
            let adv = unsafe {
                if GetTextExtentPoint32W(self.hdc, &u16_ch, &mut sz).as_bool() {
                    sz.cx as f32 * 0.5
                } else {
                    size_key as f32 * 0.25
                }
            };
            return CachedGlyph {
                width: 0,
                height: 0,
                xmin: 0,
                ymin: 0,
                advance_width: adv,
                bitmap: Vec::new(),
            };
        }

        let adv = gm.gmCellIncX as f32 * 0.5;
        if buf_size == 0 || gm.gmBlackBoxX == 0 || gm.gmBlackBoxY == 0 {
            return CachedGlyph {
                width: 0,
                height: 0,
                xmin: 0,
                ymin: 0,
                advance_width: adv,
                bitmap: Vec::new(),
            };
        }

        self.scratch_buf.resize(buf_size as usize, 0);
        let got = unsafe {
            GetGlyphOutlineW(
                self.hdc,
                code,
                GGO_GRAY8_BITMAP,
                &mut gm,
                buf_size,
                Some(self.scratch_buf.as_mut_ptr() as *mut core::ffi::c_void),
                &mat,
            )
        };
        if got == GDI_ERROR as u32 {
            return CachedGlyph {
                width: 0,
                height: 0,
                xmin: 0,
                ymin: 0,
                advance_width: adv,
                bitmap: Vec::new(),
            };
        }

        let w2 = gm.gmBlackBoxX as usize;
        let h2 = gm.gmBlackBoxY as usize;
        let stride = (w2 + 3) & !3;

        let shift_x = if phase == 1 { 1isize } else { 0isize };
        let x2_min = gm.gmptGlyphOrigin.x as isize + shift_x;
        let y2_min = gm.gmptGlyphOrigin.y as isize - h2 as isize;
        let x2_max = x2_min + w2 as isize;
        let y2_max = y2_min + h2 as isize;

        let gx0 = x2_min.div_euclid(2);
        let gy0 = y2_min.div_euclid(2);
        let gx1 = (x2_max + 1).div_euclid(2);
        let gy1 = (y2_max + 1).div_euclid(2);

        let w1 = (gx1 - gx0).max(0) as usize;
        let h1 = (gy1 - gy0).max(0) as usize;
        let mut out = vec![0u8; w1 * h1];

        let buf = &self.scratch_buf;
        let sample2 = |sx: isize, sy_from_bottom: isize| -> u32 {
            let rx = sx - x2_min;
            let ry_from_bottom = sy_from_bottom - y2_min;
            if rx < 0 || rx >= w2 as isize || ry_from_bottom < 0 || ry_from_bottom >= h2 as isize {
                return 0;
            }
            let row = (h2 as isize - 1 - ry_from_bottom) as usize;
            let idx = row * stride + rx as usize;
            buf.get(idx).copied().unwrap_or(0) as u32
        };

        for r1 in 0..h1 {
            let gy = gy1 - 1 - r1 as isize;
            let sy_base = gy * 2;
            for c1 in 0..w1 {
                let gx = gx0 + c1 as isize;
                let sx_base = gx * 2;
                // Each GGO_GRAY8_BITMAP pixel is 0..=64; 4-pixel sum is 0..=256 -> exact 0..=255
                let sum = sample2(sx_base, sy_base)
                    + sample2(sx_base + 1, sy_base)
                    + sample2(sx_base, sy_base + 1)
                    + sample2(sx_base + 1, sy_base + 1);
                out[r1 * w1 + c1] = ((sum * 255 + 128) / 256).min(255) as u8;
            }
        }

        CachedGlyph {
            width: w1,
            height: h1,
            xmin: gx0 as i32,
            ymin: gy0 as i32,
            advance_width: adv,
            bitmap: out,
        }
    }

    fn glyph(&mut self, bold: bool, size: f32, phase: u8, ch: char) -> &CachedGlyph {
        let size_key = (size * 2.0).round() as u16;
        if self.cache.len() > 1200 {
            self.cache.clear();
        }
        let key = (bold, size_key, phase, ch);
        if !self.cache.contains_key(&key) {
            let g = self.rasterize_2x_gdi(bold, size_key, phase, ch);
            self.cache.insert(key, g);
        }
        self.cache.get(&key).unwrap()
    }

    fn kern(&self, _bold: bool, _prev: Option<char>, _ch: char, _size: f32) -> f32 {
        0.0
    }

    /// Exact baseline Y to vertically center text around `center_y`
    pub fn centered_baseline(&self, center_y: f32, size: f32, bold: bool) -> f32 {
        let ratio = if bold {
            self.bold_cap_ratio
        } else {
            self.reg_cap_ratio
        };
        center_y + size * ratio * 0.5
    }
}

impl FontEngine {

    pub fn measure_width(&mut self, text: &str, size: f32, bold: bool) -> f32 {
        let mut w = 0.0f32;
        let mut prev = None;
        for ch in text.chars() {
            w += self.kern(bold, prev, ch, size);
            w += self.glyph(bold, size, 0, ch).advance_width;
            prev = Some(ch);
        }
        w
    }

    pub fn truncate_to_width(&mut self, text: &str, size: f32, bold: bool, max_w: f32) -> String {
        if self.measure_width(text, size, bold) <= max_w {
            return text.to_string();
        }
        let ell_w = self.measure_width("…", size, bold);
        let budget = (max_w - ell_w).max(0.0);
        let mut out = String::new();
        let mut acc = 0.0f32;
        let mut prev = None;
        for ch in text.chars() {
            let k = self.kern(bold, prev, ch, size);
            let cw = self.glyph(bold, size, 0, ch).advance_width;
            if acc + k + cw > budget {
                break;
            }
            out.push(ch);
            acc += k + cw;
            prev = Some(ch);
        }
        out.push('…');
        out
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_text_clipped(
        &mut self,
        pixmap: &mut Pixmap,
        x: f32,
        baseline_y: f32,
        text: &str,
        size: f32,
        bold: bool,
        rgba: (u8, u8, u8, u8),
        clip: Option<HitRect>,
    ) -> f32 {
        if rgba.3 == 0 || text.is_empty() {
            return 0.0;
        }
        let pw = pixmap.width() as i32;
        let ph = pixmap.height() as i32;
        let (cx0, cy0, cx1, cy1) = if let Some(c) = clip {
            (
                (c.x.floor() as i32).max(0),
                (c.y.floor() as i32).max(0),
                ((c.x + c.w).ceil() as i32).min(pw),
                ((c.y + c.h).ceil() as i32).min(ph),
            )
        } else {
            (0, 0, pw, ph)
        };
        if cx0 >= cx1 || cy0 >= cy1 {
            return 0.0;
        }

        let data = pixmap.data_mut();
        let (sr, sg, sb, sa) = rgba;
        let sa_f = sa as f32 / 255.0;
        let base_y_i = baseline_y.round() as i32;

        let mut cursor_x = x;
        let mut prev = None;
        for ch in text.chars() {
            cursor_x += self.kern(bold, prev, ch, size);
            let floor_x = cursor_x.floor();
            let frac = cursor_x - floor_x;
            let (ix, phase) = if frac < 0.25 {
                (floor_x as i32, 0u8)
            } else if frac < 0.75 {
                (floor_x as i32, 1u8)
            } else {
                (floor_x as i32 + 1, 0u8)
            };

            let _ = self.glyph(bold, size, phase, ch);
            let size_key = (size * 2.0).round() as u16;
            let g = self.cache.get(&(bold, size_key, phase, ch)).unwrap();
            let gx = ix + g.xmin;
            let gy = base_y_i - g.height as i32 - g.ymin;

            if gx < cx1 && gx + g.width as i32 > cx0 && gy < cy1 && gy + g.height as i32 > cy0 {
                for row in 0..g.height {
                    let dy = gy + row as i32;
                    if dy < cy0 || dy >= cy1 {
                        continue;
                    }
                    let row_off = row * g.width;
                    for col in 0..g.width {
                        let dx = gx + col as i32;
                        if dx < cx0 || dx >= cx1 {
                            continue;
                        }
                        let cov = g.bitmap[row_off + col];
                        if cov == 0 {
                            continue;
                        }
                        let alpha = self.gamma_lut[cov as usize] * sa_f;
                        let inv = 1.0 - alpha;
                        let idx = ((dy * pw + dx) * 4) as usize;

                        data[idx] = (sr as f32 * alpha + data[idx] as f32 * inv + 0.5).min(255.0) as u8;
                        data[idx + 1] =
                            (sg as f32 * alpha + data[idx + 1] as f32 * inv + 0.5).min(255.0) as u8;
                        data[idx + 2] =
                            (sb as f32 * alpha + data[idx + 2] as f32 * inv + 0.5).min(255.0) as u8;
                        data[idx + 3] =
                            (255.0 * alpha + data[idx + 3] as f32 * inv + 0.5).min(255.0) as u8;
                    }
                }
            }
            cursor_x += g.advance_width;
            prev = Some(ch);
        }
        cursor_x - x
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_text(
        &mut self,
        pixmap: &mut Pixmap,
        x: f32,
        baseline_y: f32,
        text: &str,
        size: f32,
        bold: bool,
        rgba: (u8, u8, u8, u8),
        max_w: Option<f32>,
    ) -> f32 {
        let rendered = if let Some(mw) = max_w {
            self.truncate_to_width(text, size, bold, mw)
        } else {
            text.to_string()
        };
        self.draw_text_clipped(pixmap, x, baseline_y, &rendered, size, bold, rgba, None)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_text_centered_y(
        &mut self,
        pixmap: &mut Pixmap,
        x: f32,
        center_y: f32,
        text: &str,
        size: f32,
        bold: bool,
        rgba: (u8, u8, u8, u8),
        max_w: Option<f32>,
    ) -> f32 {
        let by = self.centered_baseline(center_y, size, bold);
        self.draw_text(pixmap, x, by, text, size, bold, rgba, max_w)
    }
}

/// Silky cubic ease-in-out (0..=1 -> 0..=1) with gentle acceleration and deceleration.
fn ease_in_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) * 0.5
    }
}

fn alpha_scale(rgba: (u8, u8, u8, u8), mul: f32) -> (u8, u8, u8, u8) {
    (
        rgba.0,
        rgba.1,
        rgba.2,
        ((rgba.3 as f32) * mul.clamp(0.0, 1.0)).round() as u8,
    )
}

fn paint_rgba(r: u8, g: u8, b: u8, a: u8) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(Color::from_rgba8(r, g, b, a));
    p.anti_alias = true;
    p
}

fn rounded_rect_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
    if w <= 0.5 || h <= 0.5 {
        return None;
    }
    let r = r.min(w * 0.5).min(h * 0.5).max(0.0);
    let k = r * 0.5522848;
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    pb.close();
    pb.finish()
}

/// Exact replica of original `cairo_notch_path_at`: smooth concave top ears blending into top edge.
fn notch_path(x0: f32, w: f32, h: f32, ear_r: f32, bot_r: f32) -> Option<Path> {
    let er = ear_r.min(w * 0.16).min(h * 0.45).max(2.0);
    let br = bot_r.min((w - 2.0 * er) * 0.45).min(h * 0.5).max(4.0);
    let k_er = er * 0.5522848;
    let k_br = br * 0.5522848;

    let left = x0 + er;
    let right = x0 + w - er;

    let mut pb = PathBuilder::new();
    pb.move_to(x0, 0.0);
    pb.cubic_to(x0 + k_er, 0.0, left, er - k_er, left, er);
    pb.line_to(left, h - br);
    pb.cubic_to(left, h - br + k_br, left + br - k_br, h, left + br, h);
    pb.line_to(right - br, h);
    pb.cubic_to(right - br + k_br, h, right, h - br + k_br, right, h - br);
    pb.line_to(right, er);
    pb.cubic_to(right, er - k_er, x0 + w - k_er, 0.0, x0 + w, 0.0);
    pb.close();
    pb.finish()
}

/// Vertical mirror of `notch_path` anchored flush to the bottom edge (`base_y`).
fn bottom_notch_path(x0: f32, base_y: f32, w: f32, h: f32, ear_r: f32, top_r: f32) -> Option<Path> {
    let er = ear_r.min(w * 0.16).min(h * 0.45).max(2.0);
    let tr = top_r.min((w - 2.0 * er) * 0.45).min(h * 0.5).max(4.0);
    let k_er = er * 0.5522848;
    let k_tr = tr * 0.5522848;

    let left = x0 + er;
    let right = x0 + w - er;
    let top_y = base_y - h;

    let mut pb = PathBuilder::new();
    pb.move_to(x0, base_y);
    pb.cubic_to(x0 + k_er, base_y, left, base_y - er + k_er, left, base_y - er);
    pb.line_to(left, top_y + tr);
    pb.cubic_to(left, top_y + tr - k_tr, left + tr - k_tr, top_y, left + tr, top_y);
    pb.line_to(right - tr, top_y);
    pb.cubic_to(right - tr + k_tr, top_y, right, top_y + tr - k_tr, right, top_y + tr);
    pb.line_to(right, base_y - er);
    pb.cubic_to(right, base_y - er + k_er, x0 + w - k_er, base_y, x0 + w, base_y);
    pb.close();
    pb.finish()
}

/// Exact bottom-strip silhouette for rows `bar_top..base_y` so expanded content never bleeds over the bottom compact bar.
fn bottom_compact_strip_path(
    style: &str,
    x0: f32,
    bar_top: f32,
    base_y: f32,
    w: f32,
    ear_r: f32,
    corner_r: f32,
) -> Option<Path> {
    let strip_h = (base_y - bar_top).max(2.0);
    if style == "notch" {
        let er = ear_r.min(w * 0.16).min(strip_h * 0.9).max(2.0);
        let k_er = er * 0.5522848;
        let left = x0 + er;
        let right = x0 + w - er;
        let mut pb = PathBuilder::new();
        pb.move_to(left, bar_top);
        pb.line_to(right, bar_top);
        pb.line_to(right, base_y - er);
        pb.cubic_to(right, base_y - er + k_er, x0 + w - k_er, base_y, x0 + w, base_y);
        pb.line_to(x0, base_y);
        pb.cubic_to(x0 + k_er, base_y, left, base_y - er + k_er, left, base_y - er);
        pb.close();
        pb.finish()
    } else {
        let bot = base_y - 1.0;
        let r = corner_r.min(w * 0.5).min((bot - bar_top).max(1.0)).max(0.0);
        let k = r * 0.5522848;
        let mut pb = PathBuilder::new();
        pb.move_to(x0, bar_top);
        pb.line_to(x0 + w, bar_top);
        pb.line_to(x0 + w, bot - r);
        pb.cubic_to(x0 + w, bot - r + k, x0 + w - r + k, bot, x0 + w - r, bot);
        pb.line_to(x0 + r, bot);
        pb.cubic_to(x0 + r - k, bot, x0, bot - r + k, x0, bot - r);
        pb.close();
        pb.finish()
    }
}

fn fill_round_rect(pixmap: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, rgba: (u8, u8, u8, u8)) {
    if rgba.3 == 0 {
        return;
    }
    if let Some(path) = rounded_rect_path(x, y, w, h, r) {
        let paint = paint_rgba(rgba.0, rgba.1, rgba.2, rgba.3);
        pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

fn fill_circle(pixmap: &mut Pixmap, cx: f32, cy: f32, r: f32, rgba: (u8, u8, u8, u8)) {
    if rgba.3 == 0 {
        return;
    }
    if let Some(path) = PathBuilder::from_circle(cx, cy, r) {
        let paint = paint_rgba(rgba.0, rgba.1, rgba.2, rgba.3);
        pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

fn draw_line(pixmap: &mut Pixmap, x1: f32, y1: f32, x2: f32, y2: f32, width: f32, rgba: (u8, u8, u8, u8)) {
    if rgba.3 == 0 {
        return;
    }
    let mut pb = PathBuilder::new();
    pb.move_to(x1, y1);
    pb.line_to(x2, y2);
    if let Some(path) = pb.finish() {
        let paint = paint_rgba(rgba.0, rgba.1, rgba.2, rgba.3);
        let stroke = Stroke {
            width,
            line_cap: LineCap::Round,
            ..Stroke::default()
        };
        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }
}

fn stroke_circle(pixmap: &mut Pixmap, cx: f32, cy: f32, r: f32, width: f32, rgba: (u8, u8, u8, u8)) {
    if rgba.3 == 0 {
        return;
    }
    if let Some(path) = PathBuilder::from_circle(cx, cy, r) {
        let paint = paint_rgba(rgba.0, rgba.1, rgba.2, rgba.3);
        let stroke = Stroke {
            width,
            ..Stroke::default()
        };
        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }
}

// =============================================================================
// CRISP VECTOR ICONS
// =============================================================================

fn draw_music_note_icon(pixmap: &mut Pixmap, cx: f32, cy: f32, scale: f32, rgba: (u8, u8, u8, u8)) {
    fill_circle(pixmap, cx - 2.6 * scale, cy + 3.0 * scale, 2.2 * scale, rgba);
    fill_circle(pixmap, cx + 3.0 * scale, cy + 2.0 * scale, 2.2 * scale, rgba);
    draw_line(
        pixmap,
        cx - 0.8 * scale,
        cy + 2.6 * scale,
        cx - 0.8 * scale,
        cy - 4.0 * scale,
        1.5 * scale,
        rgba,
    );
    draw_line(
        pixmap,
        cx + 4.8 * scale,
        cy + 1.6 * scale,
        cx + 4.8 * scale,
        cy - 5.0 * scale,
        1.5 * scale,
        rgba,
    );
    draw_line(
        pixmap,
        cx - 0.8 * scale,
        cy - 4.0 * scale,
        cx + 4.8 * scale,
        cy - 5.0 * scale,
        2.0 * scale,
        rgba,
    );
}

fn draw_tab_icon(pixmap: &mut Pixmap, idx: usize, cx: f32, cy: f32, rgba: (u8, u8, u8, u8)) {
    match idx {
        0 => draw_music_note_icon(pixmap, cx, cy, 1.0, rgba),
        1 => {
            fill_circle(pixmap, cx, cy - 1.4, 3.6, rgba);
            fill_round_rect(pixmap, cx - 4.8, cy + 1.1, 9.6, 2.2, 1.1, rgba);
            fill_circle(pixmap, cx, cy + 4.4, 1.4, rgba);
        }
        2 => {
            stroke_circle(pixmap, cx, cy + 0.5, 4.7, 1.45, rgba);
            draw_line(pixmap, cx, cy + 0.5, cx, cy - 2.2, 1.4, rgba);
            draw_line(pixmap, cx, cy + 0.5, cx + 2.3, cy + 1.5, 1.4, rgba);
            draw_line(pixmap, cx - 2.0, cy - 5.5, cx + 2.0, cy - 5.5, 1.4, rgba);
        }
        3 => {
            draw_line(pixmap, cx - 4.6, cy - 2.5, cx - 3.2, cy - 1.1, 1.45, rgba);
            draw_line(pixmap, cx - 3.2, cy - 1.1, cx - 1.2, cy - 3.6, 1.45, rgba);
            draw_line(pixmap, cx + 0.8, cy - 2.4, cx + 4.9, cy - 2.4, 1.45, rgba);
            draw_line(pixmap, cx - 4.2, cy + 2.4, cx - 1.6, cy + 2.4, 1.45, rgba);
            draw_line(pixmap, cx + 0.8, cy + 2.4, cx + 4.9, cy + 2.4, 1.45, rgba);
        }
        4 => {
            fill_round_rect(pixmap, cx - 5.2, cy - 3.9, 4.4, 2.3, 1.0, rgba);
            fill_round_rect(pixmap, cx - 5.2, cy - 2.3, 10.4, 6.6, 1.8, rgba);
        }
        _ => {
            let mut pb = PathBuilder::new();
            pb.move_to(cx + 1.6, cy - 4.9);
            pb.cubic_to(cx - 4.2, cy - 4.5, cx - 5.3, cy + 3.7, cx + 1.6, cy + 4.9);
            pb.cubic_to(cx - 1.6, cy + 2.5, cx - 1.6, cy - 2.5, cx + 1.6, cy - 4.9);
            pb.close();
            if let Some(p) = pb.finish() {
                let paint = paint_rgba(rgba.0, rgba.1, rgba.2, rgba.3);
                pixmap.fill_path(&p, &paint, FillRule::Winding, Transform::identity(), None);
            }
        }
    }
}

fn draw_header_action_icon(pixmap: &mut Pixmap, kind: u8, cx: f32, cy: f32, rgba: (u8, u8, u8, u8)) {
    match kind {
        0 => {
            stroke_circle(pixmap, cx, cy + 0.6, 4.1, 1.45, rgba);
            draw_line(pixmap, cx, cy - 4.7, cx, cy - 0.5, 1.55, rgba);
        }
        1 => {
            stroke_circle(pixmap, cx, cy, 3.1, 1.5, rgba);
            for i in 0..6 {
                let a = (i as f32) * PI / 3.0;
                draw_line(
                    pixmap,
                    cx + a.cos() * 3.7,
                    cy + a.sin() * 3.7,
                    cx + a.cos() * 5.1,
                    cy + a.sin() * 5.1,
                    1.5,
                    rgba,
                );
            }
        }
        2 => {
            fill_circle(pixmap, cx, cy - 2.1, 2.7, rgba);
            draw_line(pixmap, cx, cy + 0.4, cx, cy + 4.9, 1.55, rgba);
        }
        _ => {
            draw_line(pixmap, cx - 3.4, cy - 3.4, cx + 3.4, cy + 3.4, 1.55, rgba);
            draw_line(pixmap, cx + 3.4, cy - 3.4, cx - 3.4, cy + 3.4, 1.55, rgba);
        }
    }
}

fn draw_play_icon(pixmap: &mut Pixmap, cx: f32, cy: f32, size: f32, rgba: (u8, u8, u8, u8)) {
    let half = size * 0.5;
    let mut pb = PathBuilder::new();
    pb.move_to(cx - half * 0.65, cy - half);
    pb.line_to(cx + half, cy);
    pb.line_to(cx - half * 0.65, cy + half);
    pb.close();
    if let Some(p) = pb.finish() {
        let paint = paint_rgba(rgba.0, rgba.1, rgba.2, rgba.3);
        pixmap.fill_path(&p, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

fn draw_pause_icon(pixmap: &mut Pixmap, cx: f32, cy: f32, size: f32, rgba: (u8, u8, u8, u8)) {
    let bw = (size * 0.28).max(2.2);
    let gap = size * 0.24;
    fill_round_rect(pixmap, cx - gap * 0.5 - bw, cy - size * 0.5, bw, size, 1.1, rgba);
    fill_round_rect(pixmap, cx + gap * 0.5, cy - size * 0.5, bw, size, 1.1, rgba);
}

fn draw_prev_icon(pixmap: &mut Pixmap, cx: f32, cy: f32, size: f32, rgba: (u8, u8, u8, u8)) {
    let half = size * 0.5;
    fill_round_rect(pixmap, cx - half, cy - half * 0.85, 2.0, half * 1.7, 1.0, rgba);
    let mut pb = PathBuilder::new();
    pb.move_to(cx + half, cy - half * 0.85);
    pb.line_to(cx - half + 2.4, cy);
    pb.line_to(cx + half, cy + half * 0.85);
    pb.close();
    if let Some(p) = pb.finish() {
        let paint = paint_rgba(rgba.0, rgba.1, rgba.2, rgba.3);
        pixmap.fill_path(&p, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

fn draw_next_icon(pixmap: &mut Pixmap, cx: f32, cy: f32, size: f32, rgba: (u8, u8, u8, u8)) {
    let half = size * 0.5;
    fill_round_rect(pixmap, cx + half - 2.0, cy - half * 0.85, 2.0, half * 1.7, 1.0, rgba);
    let mut pb = PathBuilder::new();
    pb.move_to(cx - half, cy - half * 0.85);
    pb.line_to(cx + half - 2.4, cy);
    pb.line_to(cx - half, cy + half * 0.85);
    pb.close();
    if let Some(p) = pb.finish() {
        let paint = paint_rgba(rgba.0, rgba.1, rgba.2, rgba.3);
        pixmap.fill_path(&p, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

fn draw_notif_category_icon(
    pixmap: &mut Pixmap,
    fonts: &mut FontEngine,
    notif: &IslandNotification,
    cx: f32,
    cy: f32,
    ease: f32,
    (nr, ng, nb): (f64, f64, f64),
) {
    let app_l = notif.app_name.to_lowercase();
    let sum_l = notif.summary.to_lowercase();
    let col = (
        ((nr * 0.30 + 0.70).min(1.0) * 255.0) as u8,
        ((ng * 0.30 + 0.70).min(1.0) * 255.0) as u8,
        ((nb * 0.30 + 0.70).min(1.0) * 255.0) as u8,
        (248.0 * ease) as u8,
    );

    if notif.urgency >= 2 {
        let ew = fonts.measure_width("!", 16.5, true);
        fonts.draw_text_centered_y(
            pixmap,
            cx - ew * 0.5,
            cy,
            "!",
            16.5,
            true,
            (255, 224, 230, (250.0 * ease) as u8),
            None,
        );
    } else if app_l.contains("whatsapp")
        || app_l.contains("zapfast")
        || app_l.contains("telegram")
        || app_l.contains("discord")
    {
        fill_round_rect(pixmap, cx - 7.5, cy - 6.2, 15.0, 11.2, 4.0, col);
        let mut pb = PathBuilder::new();
        pb.move_to(cx - 3.5, cy + 4.4);
        pb.line_to(cx - 6.5, cy + 8.0);
        pb.line_to(cx + 0.5, cy + 4.4);
        pb.close();
        if let Some(p) = pb.finish() {
            let paint = paint_rgba(col.0, col.1, col.2, col.3);
            pixmap.fill_path(&p, &paint, FillRule::Winding, Transform::identity(), None);
        }
    } else if app_l.contains("baterai")
        || app_l.contains("daya")
        || app_l.contains("power")
        || sum_l.contains("baterai")
    {
        let mut pb = PathBuilder::new();
        pb.move_to(cx + 1.2, cy - 7.5);
        pb.line_to(cx - 5.2, cy + 0.8);
        pb.line_to(cx - 0.5, cy + 0.8);
        pb.line_to(cx - 1.8, cy + 7.5);
        pb.line_to(cx + 5.2, cy - 1.0);
        pb.line_to(cx + 0.5, cy - 1.0);
        pb.close();
        if let Some(p) = pb.finish() {
            let paint = paint_rgba(col.0, col.1, col.2, col.3);
            pixmap.fill_path(&p, &paint, FillRule::Winding, Transform::identity(), None);
        }
    } else {
        draw_tab_icon(pixmap, 1, cx, cy, col);
    }
}

fn format_sec_short(sec: f64) -> String {
    let total = sec.max(0.0).round() as u64;
    format!("{}:{:02}", total / 60, total % 60)
}

// =============================================================================
// MAIN ISLAND RENDERER
// =============================================================================

pub fn render_island(
    pixmap: &mut Pixmap,
    fonts: &mut FontEngine,
    ctx: &RuntimeContext,
    hitmap: &mut UiHitMap,
) -> u16 {
    hitmap.clear();
    pixmap.fill(Color::TRANSPARENT);

    let cw = CANVAS_WIDTH as f32;
    let ch = pixmap.height() as f32;
    let w = (ctx.cur_w as f32).clamp(200.0, cw - 4.0);
    let h = (ctx.cur_h as f32).clamp(COMPACT_H as f32, ch - 2.0);
    let x0 = (cw - w) * 0.5;
    let is_bottom = ctx.app_state.island_position == "bottom";
    let y0 = if is_bottom { ch - h } else { 0.0 };

    hitmap.pill = HitRect::new(x0, y0, w, h);

    let style = ctx.app_state.island_style.as_str();
    let exp_h = ctx.expanded_target_h.max(160.0) as f32;
    let raw_prog = ((h - COMPACT_H as f32) / (exp_h - COMPACT_H as f32)).clamp(0.0, 1.0);
    let ear_r = 14.0;
    let shape_r = if style == "notch" {
        17.5 + raw_prog * 6.5
    } else if style == "dock" {
        10.0 + raw_prog * 4.0
    } else {
        19.0 + raw_prog * 7.0
    };
    let bg_rgba = match style {
        "notch" => (6, 6, 10, 255),
        "dock" => (8, 8, 12, 255),
        _ => (7, 7, 11, 255),
    };

    // 1. Exact Borderless Deep-Black Silhouette (`notch` / `dock` / `pill`), top or bottom anchored
    if style == "notch" {
        let maybe_path = if is_bottom {
            bottom_notch_path(x0, ch, w, h - 1.0, ear_r, shape_r)
        } else {
            notch_path(x0, w, h - 1.0, ear_r, shape_r)
        };
        if let Some(path) = maybe_path {
            let paint = paint_rgba(bg_rgba.0, bg_rgba.1, bg_rgba.2, bg_rgba.3);
            pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
        }
    } else {
        fill_round_rect(pixmap, x0, y0 + 1.0, w, h - 2.0, shape_r, bg_rgba);
    }

    let pad_x = if style == "notch" { 22.0 } else { 16.0 };
    let inner_left = x0 + pad_x;
    let inner_right = x0 + w - pad_x;

    let has_vol_osd = !ctx.expanded && ctx.vol_osd_remaining > 0.0;
    let has_toast = !ctx.expanded
        && !has_vol_osd
        && ctx.active_notif_toast.is_some()
        && ctx.notif_toast_remaining > 0.0;

    if has_vol_osd {
        draw_volume_osd(pixmap, fonts, ctx, hitmap, inner_left, inner_right, y0, h);
        return (h.ceil() as u16 + 4).min(pixmap.height() as u16);
    }

    if has_toast {
        if let Some(ref notif) = ctx.active_notif_toast {
            draw_live_notification_toast(pixmap, fonts, ctx, notif, inner_left, inner_right, y0, h);
            return (h.ceil() as u16 + 4).min(pixmap.height() as u16);
        }
    }

    let bar_y = if is_bottom { ch - COMPACT_H as f32 } else { 0.0 };

    if !is_bottom {
        draw_compact_top_bar(pixmap, fonts, ctx, inner_left, inner_right, bar_y);
    }

    let body_opacity = if ctx.expanded {
        ((raw_prog - 0.12) / 0.88).clamp(0.0, 1.0)
    } else {
        ((raw_prog - 0.22) / 0.78).clamp(0.0, 1.0)
    };

    if body_opacity > 0.02 && h > COMPACT_H as f32 + 24.0 {
        let slide_y = (1.0 - raw_prog) * 6.0;
        let body_x = x0 + 20.0;
        let body_w = (w - 40.0).max(120.0);
        let (body_y, body_h) = if is_bottom {
            let by = y0 + 12.0 - slide_y;
            let bh = (bar_y - 4.0) - by;
            (by, bh)
        } else {
            let by = COMPACT_H as f32 + 4.0 + slide_y;
            let bh = h - by - 12.0;
            (by, bh)
        };
        draw_expanded_body(
            pixmap,
            fonts,
            ctx,
            hitmap,
            body_x,
            body_y,
            body_w,
            body_h,
            body_opacity,
        );

        let row_bytes = pixmap.width() as usize * 4;
        if is_bottom {
            // Strictly clear any overflow pixels above the active top edge during collapse
            let cut_y = (y0.ceil().max(0.0) as usize).min(pixmap.height() as usize);
            if cut_y > 0 {
                pixmap.data_mut()[..cut_y * row_bytes].fill(0);
            }
            // Repaint clean bottom strip behind the bottom compact bar so body never bleeds into it
            let bar_top_u = (bar_y.floor().max(0.0) as usize).min(pixmap.height() as usize);
            if bar_top_u < pixmap.height() as usize {
                pixmap.data_mut()[bar_top_u * row_bytes..].fill(0);
                if let Some(strip) =
                    bottom_compact_strip_path(style, x0, bar_top_u as f32, ch, w, ear_r, shape_r)
                {
                    let paint = paint_rgba(bg_rgba.0, bg_rgba.1, bg_rgba.2, bg_rgba.3);
                    pixmap.fill_path(&strip, &paint, FillRule::Winding, Transform::identity(), None);
                }
            }
        } else {
            // Strictly clear any overflow pixels below the active notch bottom edge (prevents ghost shadow during collapse)
            let cut_y = (h.floor() as usize).min(pixmap.height() as usize);
            if cut_y < pixmap.height() as usize {
                pixmap.data_mut()[cut_y * row_bytes..].fill(0);
            }
        }
    }

    if is_bottom {
        draw_compact_top_bar(pixmap, fonts, ctx, inner_left, inner_right, bar_y);
    }

    (h.ceil() as u16 + 4).min(pixmap.height() as u16)
}

fn draw_compact_top_bar(
    pixmap: &mut Pixmap,
    fonts: &mut FontEngine,
    ctx: &RuntimeContext,
    inner_left: f32,
    inner_right: f32,
    bar_y: f32,
) {
    let center_y = bar_y + COMPACT_H as f32 * 0.5;
    let is_playing = ctx.cached_media.has_player && ctx.cached_media.playing;
    let pal = resolve_accent_palette(&ctx.app_state.accent_color);
    let (ar, ag, ab) = pal.rgb_f64();
    let (acc_r, acc_g, acc_b) = pal.light;

    let mut left_cursor = inner_left;

    if is_playing {
        let bar_w = 2.4f32;
        let gap = 2.1f32;
        for i in 0..3 {
            let phase_off = i as f64 * 1.35;
            let amp = ((ctx.wave_phase + phase_off).sin() * 0.5 + 0.5).clamp(0.18, 1.0) as f32;
            let bh = 4.2 + amp * 10.0;
            let bx = left_cursor + i as f32 * (bar_w + gap);
            let by = center_y - bh * 0.5;
            let col = if i == 1 {
                (acc_r, acc_g, acc_b, 250)
            } else {
                (
                    ((ar * 0.85 + 0.15) * 255.0) as u8,
                    ((ag * 0.85 + 0.15) * 255.0) as u8,
                    ((ab * 0.85 + 0.15) * 255.0) as u8,
                    220,
                )
            };
            fill_round_rect(pixmap, bx, by, bar_w, bh, 1.2, col);
        }
        left_cursor += 3.0 * (bar_w + gap) + 6.0;
    } else {
        let dot_col = if ctx.pomo_running {
            if ctx.pomo_break_mode {
                (251, 191, 36, 250)
            } else {
                (pal.light.0, pal.light.1, pal.light.2, 250)
            }
        } else if ctx.unread_notif_count > 0 {
            (pal.light.0, pal.light.1, pal.light.2, 242)
        } else {
            (pal.light.0, pal.light.1, pal.light.2, 210)
        };
        fill_circle(pixmap, left_cursor + 4.0, center_y, 3.2, dot_col);
        left_cursor += 13.0;
    }

    let (time_display, time_col) = if ctx.pomo_running {
        let m = ctx.pomo_remaining_secs / 60;
        let s = ctx.pomo_remaining_secs % 60;
        let col = if ctx.pomo_break_mode {
            (253, 224, 71, 255)
        } else {
            (pal.light.0, pal.light.1, pal.light.2, 255)
        };
        (format!("{:02}:{:02}", m, s), col)
    } else {
        (ctx.clock_str.clone(), (247, 250, 255, 248))
    };

    let tw = fonts.draw_text_centered_y(
        pixmap,
        left_cursor,
        center_y,
        &time_display,
        13.2,
        true,
        time_col,
        None,
    );
    left_cursor += tw + 8.0;

    if ctx.unread_notif_count > 0 {
        let b_txt = ctx.unread_notif_count.min(99).to_string();
        let bw = fonts.measure_width(&b_txt, 10.5, true);
        let pill_w = (bw + 10.0).max(16.0);
        let pill_h = 16.0;
        let py = center_y - pill_h * 0.5;
        fill_round_rect(pixmap, left_cursor, py, pill_w, pill_h, 8.0, (pal.solid.0, pal.solid.1, pal.solid.2, 68));
        fonts.draw_text_centered_y(
            pixmap,
            left_cursor + (pill_w - bw) * 0.5,
            center_y,
            &b_txt,
            10.5,
            true,
            (pal.glow.0, pal.glow.1, pal.glow.2, 252),
            None,
        );
        left_cursor += pill_w + 8.0;
    } else {
        left_cursor += 4.0;
    }

    let lyric_avail_w = (inner_right - left_cursor).max(12.0);
    let lyric_clip = HitRect::new(left_cursor, bar_y + 0.5, lyric_avail_w, COMPACT_H as f32 - 1.0);

    let has_expanded_toast =
        ctx.expanded && ctx.active_notif_toast.is_some() && ctx.notif_toast_remaining > 0.0;

    if has_expanded_toast {
        if let Some(ref notif) = ctx.active_notif_toast {
            let (nr, ng, nb) = notif.accent_rgb();
            let t_in = ctx.notif_anim_prog.clamp(0.0, 1.0) as f32;
            let ease_in = ease_in_out(t_in);
            let vis = ease_in.clamp(0.0, 1.0);
            let slide_y = (1.0 - ease_in) * 10.0;

            let pill_txt = if notif.body.is_empty() {
                format!("{} • {}", notif.app_name.to_uppercase(), notif.summary)
            } else {
                format!("{} • {} — {}", notif.app_name.to_uppercase(), notif.summary, notif.body)
            };
            let sw = fonts.measure_width(&pill_txt, 11.8, true);
            let pill_pad_l = 18.0;
            let pill_pad_r = 10.0;
            let pill_w = (sw + pill_pad_l + pill_pad_r).min(lyric_avail_w);
            let pill_x = (inner_right - pill_w).max(left_cursor);
            let pill_h = 22.0;
            let pill_y = center_y - pill_h * 0.5 + slide_y;

            // Translucent tinted accent pill + glowing indicator dot
            fill_round_rect(
                pixmap,
                pill_x,
                pill_y,
                pill_w,
                pill_h,
                11.0,
                (
                    (nr * 255.0) as u8,
                    (ng * 255.0) as u8,
                    (nb * 255.0) as u8,
                    (46.0 * vis) as u8,
                ),
            );
            fill_circle(
                pixmap,
                pill_x + 9.0,
                center_y + slide_y,
                2.8,
                (
                    (nr * 255.0) as u8,
                    (ng * 255.0) as u8,
                    (nb * 255.0) as u8,
                    (250.0 * vis) as u8,
                ),
            );

            let txt_clip = HitRect::new(
                pill_x + pill_pad_l - 2.0,
                lyric_clip.y,
                (pill_w - pill_pad_l - pill_pad_r + 4.0).max(10.0),
                lyric_clip.h,
            );
            let safe_w = (pill_w - pill_pad_l - pill_pad_r).max(16.0);
            let sx = if sw > safe_w {
                let overflow = sw - safe_w;
                let scroll_t = (((ctx.notif_scroll_phase - 0.6).max(0.0) * 0.32).sin() * 0.5 + 0.5) as f32;
                pill_x + pill_pad_l - overflow * scroll_t
            } else {
                pill_x + pill_pad_l
            };
            let col = (
                ((nr * 0.30 + 0.70).min(1.0) * 255.0) as u8,
                ((ng * 0.30 + 0.70).min(1.0) * 255.0) as u8,
                ((nb * 0.30 + 0.70).min(1.0) * 255.0) as u8,
                (250.0 * vis) as u8,
            );
            let by = fonts.centered_baseline(center_y + slide_y, 11.8, true);
            fonts.draw_text_clipped(pixmap, sx, by, &pill_txt, 11.8, true, col, Some(txt_clip));
        }
    } else if is_playing && !ctx.cur_lyric.is_empty() {
        let t = ctx.lyric_anim_prog.clamp(0.0, 1.0) as f32;
        // Fast ease-out in compact notch so lyrics appear immediately on-beat with zero delay
        let ease = 1.0 - (1.0 - t).powi(3);
        let char_len = ctx.cur_lyric.chars().count();
        let lyr_size = if char_len > 58 {
            11.8
        } else if char_len > 44 {
            12.4
        } else {
            13.0
        };
        let rest_baseline = fonts.centered_baseline(center_y, lyr_size, true);
        let slide_in_y = 12.0 * (1.0 - ease);
        let baseline = rest_baseline + slide_in_y;
        let lyric_style = ctx.app_state.lyric_style.as_str();

        // Outgoing previous lyric line glides smoothly UP & fades out inside compact notch
        if t < 0.99 && !ctx.prev_lyric.is_empty() && ctx.prev_lyric != "●  ●  ●" {
            let prev_w = fonts.measure_width(&ctx.prev_lyric, lyr_size, true);
            let prev_x = (inner_right - 3.0 - prev_w).max(left_cursor);
            let prev_y = rest_baseline - 13.0 * ease;
            let prev_a = ((1.0 - ease).powi(2) * 215.0) as u8;
            fonts.draw_text_clipped(
                pixmap,
                prev_x,
                prev_y,
                &ctx.prev_lyric,
                lyr_size,
                true,
                (220, 228, 245, prev_a),
                Some(lyric_clip),
            );
        }

        if ctx.cur_lyric == "●  ●  ●" {
            let dots_w = 34.0;
            let dx0 = (inner_right - dots_w).max(left_cursor);
            for i in 0..3 {
                let dx = dx0 + i as f32 * 13.0 + 4.0;
                let active = i <= ctx.active_word_idx;
                let r = if active { 3.5 } else { 2.8 };
                let col = if active {
                    (250, 252, 255, (245.0 * ease) as u8)
                } else {
                    (132, 140, 155, (110.0 * ease) as u8)
                };
                fill_circle(pixmap, dx, center_y + slide_in_y, r, col);
            }
        } else if ctx.lyric_is_word_synced && !ctx.lyric_words.is_empty() {
            let sp_w = fonts.measure_width(" ", lyr_size, true).max(3.8);
            let mut total_w = 0.0f32;
            let mut active_center = 0.0f32;
            let mut word_pos: Vec<(usize, f32, f32)> = Vec::with_capacity(ctx.lyric_words.len());

            for (idx, w_item) in ctx.lyric_words.iter().enumerate() {
                let ww = fonts.measure_width(&w_item.text, lyr_size, true);
                let rel_x = total_w;
                if idx == ctx.active_word_idx {
                    active_center = rel_x + ww * 0.5;
                }
                word_pos.push((idx, ww, rel_x));
                total_w += ww;
                if w_item.space_after {
                    total_w += sp_w;
                }
            }

            let safe_avail = (lyric_avail_w - 4.0).max(10.0);
            let base_x = if total_w > safe_avail {
                let max_sc = total_w - safe_avail;
                let sc = (active_center - safe_avail * 0.45).clamp(0.0, max_sc);
                left_cursor - sc
            } else {
                (inner_right - 3.0 - total_w).max(left_cursor)
            };

            let hi_col = (
                ((ar * 0.35 + 0.65).min(1.0) * 255.0) as u8,
                ((ag * 0.35 + 0.65).min(1.0) * 255.0) as u8,
                ((ab * 0.35 + 0.65).min(1.0) * 255.0) as u8,
                (255.0 * ease) as u8,
            );

            for (idx, ww, rel_x) in word_pos {
                let text = &ctx.lyric_words[idx].text;
                let wx = base_x + rel_x;
                if idx < ctx.active_word_idx {
                    let col = if lyric_style == "cinema_glow" {
                        (200, 210, 232, (145.0 * ease) as u8)
                    } else {
                        (245, 250, 255, (242.0 * ease) as u8)
                    };
                    fonts.draw_text_clipped(pixmap, wx, baseline, text, lyr_size, true, col, Some(lyric_clip));
                } else if idx == ctx.active_word_idx {
                    let bounce = if lyric_style == "word_bounce" {
                        -2.4 * ((ctx.word_prog as f32) * PI).sin()
                    } else {
                        0.0
                    };
                    let wy = baseline + bounce;
                    if lyric_style == "cinema_glow" {
                        let gx0 = (wx - 4.0).max(lyric_clip.x);
                        let gx1 = (wx + ww + 4.0).min(lyric_clip.x + lyric_clip.w);
                        if gx1 > gx0 {
                            fill_round_rect(
                                pixmap,
                                gx0,
                                center_y - 9.0 + slide_in_y,
                                gx1 - gx0,
                                18.0,
                                9.0,
                                (acc_r, acc_g, acc_b, (42.0 * ease) as u8),
                            );
                        }
                    }
                    fonts.draw_text_clipped(
                        pixmap,
                        wx,
                        wy,
                        text,
                        lyr_size,
                        true,
                        (188, 200, 232, (132.0 * ease) as u8),
                        Some(lyric_clip),
                    );
                    let fill_w = if lyric_style == "cinema_glow" {
                        ww + 1.5
                    } else {
                        (ww * (ctx.word_prog as f32).clamp(0.0, 1.0)) + 1.0
                    };
                    let word_clip = HitRect::new(
                        wx.max(lyric_clip.x),
                        lyric_clip.y,
                        (wx + fill_w).min(lyric_clip.x + lyric_clip.w) - wx.max(lyric_clip.x),
                        lyric_clip.h,
                    );
                    fonts.draw_text_clipped(pixmap, wx, wy, text, lyr_size, true, hi_col, Some(word_clip));
                } else {
                    let dim_a = if lyric_style == "cinema_glow" { 82.0 } else { 118.0 };
                    fonts.draw_text_clipped(
                        pixmap,
                        wx,
                        baseline,
                        text,
                        lyr_size,
                        true,
                        (188, 201, 235, (dim_a * ease) as u8),
                        Some(lyric_clip),
                    );
                }
            }
        } else {
            // Static / line-synced lyric (not word-by-word): render clean solid text without karaoke sweep
            let cw_f = fonts.measure_width(&ctx.cur_lyric, lyr_size, true);
            let safe_avail = (lyric_avail_w - 4.0).max(10.0);
            let base_x = if cw_f > safe_avail {
                let overflow = cw_f - safe_avail;
                left_cursor - overflow * ctx.lyric_line_prog.clamp(0.0, 1.0) as f32
            } else {
                (inner_right - 3.0 - cw_f).max(left_cursor)
            };
            fonts.draw_text_clipped(
                pixmap,
                base_x,
                baseline,
                &ctx.cur_lyric,
                lyr_size,
                true,
                (250, 252, 255, (248.0 * ease) as u8),
                Some(lyric_clip),
            );
        }
    } else {
        let info = if is_playing {
            format!("{} — {}", ctx.cached_media.title, ctx.cached_media.artist)
        } else {
            ctx.next_prayer_summary.clone()
        };
        let truncated = fonts.truncate_to_width(&info, 12.6, false, lyric_avail_w);
        let iw = fonts.measure_width(&truncated, 12.6, false);
        let ix = (inner_right - iw).max(left_cursor);
        let by = fonts.centered_baseline(center_y, 12.6, false);
        fonts.draw_text_clipped(
            pixmap,
            ix,
            by,
            &truncated,
            12.6,
            false,
            (218, 226, 244, 228),
            Some(lyric_clip),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_volume_osd(
    pixmap: &mut Pixmap,
    fonts: &mut FontEngine,
    ctx: &RuntimeContext,
    hitmap: &mut UiHitMap,
    inner_left: f32,
    inner_right: f32,
    y0: f32,
    h: f32,
) {
    let muted = ctx.cur_volume_muted || ctx.cur_volume_pct == 0;
    let pal = resolve_accent_palette(&ctx.app_state.accent_color);
    let (vr, vg, vb) = if muted {
        (245u8, 64u8, 94u8)
    } else {
        pal.light
    };
    let ease = (1.0 - (1.0 - ctx.vol_osd_anim_prog.clamp(0.0, 1.0)).powi(3)) as f32;
    let cy = y0 + h * 0.5;

    let badge_sz = 30.0;
    let badge_x = inner_left;
    let badge_y = cy - badge_sz * 0.5;
    fill_round_rect(pixmap, badge_x, badge_y, badge_sz, badge_sz, 9.5, (vr, vg, vb, (56.0 * ease) as u8));
    hitmap.push(HitRect::new(badge_x, badge_y, badge_sz, badge_sz), UiAction::VolMute);

    let scx = badge_x + badge_sz * 0.5;
    let scy = cy;
    let icon_col = (
        ((vr as f32 * 0.25 + 191.0).min(255.0)) as u8,
        ((vg as f32 * 0.25 + 191.0).min(255.0)) as u8,
        ((vb as f32 * 0.25 + 191.0).min(255.0)) as u8,
        (250.0 * ease) as u8,
    );
    let mut pb = PathBuilder::new();
    pb.move_to(scx - 6.5, scy - 2.4);
    pb.line_to(scx - 3.2, scy - 2.4);
    pb.line_to(scx + 0.8, scy - 6.0);
    pb.line_to(scx + 0.8, scy + 6.0);
    pb.line_to(scx - 3.2, scy + 2.4);
    pb.line_to(scx - 6.5, scy + 2.4);
    pb.close();
    if let Some(p) = pb.finish() {
        let paint = paint_rgba(icon_col.0, icon_col.1, icon_col.2, icon_col.3);
        pixmap.fill_path(&p, &paint, FillRule::Winding, Transform::identity(), None);
    }
    if muted {
        draw_line(pixmap, scx + 3.0, scy - 3.0, scx + 7.5, scy + 3.0, 1.7, (255, 115, 140, (250.0 * ease) as u8));
        draw_line(pixmap, scx + 7.5, scy - 3.0, scx + 3.0, scy + 3.0, 1.7, (255, 115, 140, (250.0 * ease) as u8));
    } else {
        let mut wp = PathBuilder::new();
        wp.move_to(scx + 3.2, scy - 2.8);
        wp.quad_to(scx + 5.6, scy, scx + 3.2, scy + 2.8);
        wp.move_to(scx + 5.6, scy - 4.8);
        wp.quad_to(scx + 8.8, scy, scx + 5.6, scy + 4.8);
        if let Some(p) = wp.finish() {
            let paint = paint_rgba(icon_col.0, icon_col.1, icon_col.2, icon_col.3);
            let st = Stroke { width: 1.5, line_cap: LineCap::Round, ..Stroke::default() };
            pixmap.stroke_path(&p, &paint, &st, Transform::identity(), None);
        }
    }

    let pct_str = if muted {
        "Bisu".to_string()
    } else {
        format!("{}%", ctx.disp_volume_pct.round() as u32)
    };
    let pw = fonts.measure_width(&pct_str, 13.4, true);
    let pct_col = if muted {
        (255, 115, 140, (245.0 * ease) as u8)
    } else {
        (245, 250, 255, (245.0 * ease) as u8)
    };
    fonts.draw_text_centered_y(pixmap, inner_right - pw - 2.0, cy, &pct_str, 13.4, true, pct_col, None);

    let slider_left = badge_x + badge_sz + 12.0;
    let slider_right = (inner_right - 48.0).max(slider_left + 60.0);
    let slider_w = slider_right - slider_left;
    let track_h = if ctx.vol_osd_dragging { 7.8 } else { 6.2 };
    let track_y = cy - track_h * 0.5;
    let bar_hit = HitRect::new(slider_left, cy - 14.0, slider_w, 28.0);
    hitmap.vol_bar = bar_hit;
    hitmap.push(bar_hit, UiAction::VolBar);

    fill_round_rect(pixmap, slider_left, track_y, slider_w, track_h, track_h * 0.5, (255, 255, 255, (36.0 * ease) as u8));
    let frac = if muted { 0.0 } else { (ctx.disp_volume_pct as f32 / 100.0).clamp(0.0, 1.0) };
    if frac > 0.005 {
        let fill_w = (slider_w * frac).max(track_h);
        fill_round_rect(pixmap, slider_left, track_y, fill_w, track_h, track_h * 0.5, (vr, vg, vb, (245.0 * ease) as u8));
    }
    let thumb_x = slider_left + slider_w * frac;
    let knob_r = if ctx.vol_osd_dragging { 7.2 } else { 5.8 };
    fill_circle(pixmap, thumb_x, cy, knob_r, (252, 252, 255, (250.0 * ease) as u8));
    fill_circle(pixmap, thumb_x, cy, 2.3, (vr, vg, vb, (242.0 * ease) as u8));
}

#[allow(clippy::too_many_arguments)]
fn draw_live_notification_toast(
    pixmap: &mut Pixmap,
    fonts: &mut FontEngine,
    ctx: &RuntimeContext,
    notif: &IslandNotification,
    inner_left: f32,
    inner_right: f32,
    y0: f32,
    h: f32,
) {
    let (nr, ng, nb) = notif.accent_rgb();
    let ease = ease_in_out(ctx.notif_anim_prog as f32).clamp(0.0, 1.0);
    let slide_y = 0.0f32;

    let badge_sz = 38.0;
    let badge_x = inner_left;
    let badge_y = y0 + (h - badge_sz) * 0.5 + slide_y;
    fill_round_rect(
        pixmap,
        badge_x,
        badge_y,
        badge_sz,
        badge_sz,
        11.0,
        ((nr * 255.0) as u8, (ng * 255.0) as u8, (nb * 255.0) as u8, (62.0 * ease) as u8),
    );
    draw_notif_category_icon(
        pixmap,
        fonts,
        notif,
        badge_x + badge_sz * 0.5,
        badge_y + badge_sz * 0.5,
        ease,
        (nr, ng, nb),
    );

    let text_left = badge_x + badge_sz + 12.0;
    let text_w = (inner_right - text_left).max(40.0);
    let top_cy = (badge_y + 7.0).max(y0 + 14.0);

    let prefix = if notif.urgency >= 2 { "PENTING • " } else { "" };
    let tag = format!("{}{} • {}", prefix, notif.app_name.to_uppercase(), notif.timestamp_str);
    let tag_col = (
        ((nr * 0.28 + 0.72).min(1.0) * 255.0) as u8,
        ((ng * 0.28 + 0.72).min(1.0) * 255.0) as u8,
        ((nb * 0.28 + 0.72).min(1.0) * 255.0) as u8,
        (245.0 * ease) as u8,
    );
    fonts.draw_text_centered_y(pixmap, text_left, top_cy, &tag, 10.6, true, tag_col, Some(text_w));

    let has_body = !notif.body.is_empty() && notif.body != notif.summary;
    if has_body {
        fonts.draw_text_centered_y(
            pixmap,
            text_left,
            top_cy + 16.0,
            &notif.summary,
            13.2,
            true,
            (250, 252, 255, (250.0 * ease) as u8),
            Some(text_w),
        );
        let bw = fonts.measure_width(&notif.body, 12.0, false);
        let scroll_x = if bw > text_w {
            let overflow = bw - text_w + 14.0;
            if ctx.notif_scroll_phase > 0.85 {
                text_left - (((ctx.notif_scroll_phase - 0.85) * 42.0) as f32).min(overflow)
            } else {
                text_left
            }
        } else {
            text_left
        };
        let body_clip = HitRect::new(text_left, top_cy + 22.0, text_w, 22.0);
        let by = fonts.centered_baseline(top_cy + 32.0, 12.0, false);
        fonts.draw_text_clipped(
            pixmap,
            scroll_x,
            by,
            &notif.body,
            12.0,
            false,
            (214, 220, 235, (235.0 * ease) as u8),
            Some(body_clip),
        );
    } else {
        fonts.draw_text_centered_y(
            pixmap,
            text_left,
            top_cy + 20.0,
            &notif.summary,
            13.6,
            true,
            (250, 252, 255, (250.0 * ease) as u8),
            Some(text_w),
        );
    }
}

// =============================================================================
// EXPANDED BODY
// =============================================================================

#[allow(clippy::too_many_arguments)]
fn draw_expanded_body(
    pixmap: &mut Pixmap,
    fonts: &mut FontEngine,
    ctx: &RuntimeContext,
    hitmap: &mut UiHitMap,
    x: f32,
    y: f32,
    w: f32,
    avail_h: f32,
    op: f32,
) {
    let hdr_h = 30.0;
    let tab_defs = [
        (ActiveTab::Media, "Media"),
        (ActiveTab::Notif, "Notif"),
        (ActiveTab::Focus, "Fokus"),
        (ActiveTab::Todo, "Todo"),
        (ActiveTab::Shelf, "Rak"),
        (ActiveTab::Prayer, "Sholat"),
    ];

    let mut tab_widths = [0.0f32; 6];
    let mut tab_labels: [String; 6] = Default::default();
    let mut nav_total_w = 4.0f32;
    for (i, (tid, lbl)) in tab_defs.iter().enumerate() {
        let txt = if *tid == ActiveTab::Notif && ctx.unread_notif_count > 0 {
            format!("Notif {}", ctx.unread_notif_count.min(99))
        } else {
            (*lbl).to_string()
        };
        let p = ctx.tab_spring_pos[i].clamp(0.0, 1.15) as f32;
        let lw = fonts.measure_width(&txt, 11.8, true);
        let tw = 27.0 + (lw + 10.0) * p;
        tab_widths[i] = tw;
        tab_labels[i] = txt;
        nav_total_w += tw + 2.0;
    }

    fill_round_rect(pixmap, x, y, nav_total_w, hdr_h, 15.0, alpha_scale((18, 18, 21, 255), op));
    let pal = resolve_accent_palette(&ctx.app_state.accent_color);

    let mut tx = x + 3.0;
    for (i, (tid, _)) in tab_defs.iter().enumerate() {
        let tw = tab_widths[i];
        let pill_r = HitRect::new(tx, y + 2.0, tw, hdr_h - 4.0);
        let active = ctx.active_tab == *tid;
        let has_unread = *tid == ActiveTab::Notif && ctx.unread_notif_count > 0;
        let hovered = pill_r.contains(ctx.hover_x, ctx.hover_y);

        if active {
            let bg = if has_unread {
                (pal.light.0, pal.light.1, pal.light.2, 255)
            } else {
                (244, 244, 245, 255)
            };
            fill_round_rect(pixmap, pill_r.x, pill_r.y, pill_r.w, pill_r.h, 13.0, alpha_scale(bg, op));
        } else if hovered {
            fill_round_rect(pixmap, pill_r.x, pill_r.y, pill_r.w, pill_r.h, 13.0, alpha_scale((255, 255, 255, 20), op));
        }

        let fg = if active {
            (9, 9, 11, 255)
        } else if has_unread {
            (pal.light.0, pal.light.1, pal.light.2, 255)
        } else if hovered {
            (244, 244, 245, 255)
        } else {
            (161, 161, 170, 240)
        };
        let fg_a = alpha_scale(fg, op);

        let icon_cx = pill_r.x + 13.5;
        let icon_cy = pill_r.y + pill_r.h * 0.5;
        draw_tab_icon(pixmap, i, icon_cx, icon_cy, fg_a);

        let p = ctx.tab_spring_pos[i].clamp(0.0, 1.0) as f32;
        if p > 0.08 {
            let lbl_clip = HitRect::new(pill_r.x + 22.0, pill_r.y, (pill_r.w - 25.0).max(0.0), pill_r.h);
            let by = fonts.centered_baseline(icon_cy, 11.8, true);
            fonts.draw_text_clipped(
                pixmap,
                pill_r.x + 22.0 + 3.0 * p,
                by,
                &tab_labels[i],
                11.8,
                true,
                alpha_scale(fg, op * p),
                Some(lbl_clip),
            );
        }

        hitmap.push(pill_r, UiAction::SelectTab(*tid));
        tx += tw + 2.0;
    }

    let btn_sz = 26.0;
    let btn_gap = 6.0;
    let actions = [
        (0u8, UiAction::ToggleStartup, ctx.app_state.run_on_startup),
        (1u8, UiAction::ToggleSettings, ctx.active_tab == ActiveTab::Settings),
        (2u8, UiAction::TogglePin, ctx.app_state.pinned),
        (3u8, UiAction::Quit, false),
    ];
    let total_act_w = actions.len() as f32 * btn_sz + (actions.len() as f32 - 1.0) * btn_gap;
    let mut ax = x + w - total_act_w;
    let ay = y + (hdr_h - btn_sz) * 0.5;

    for (icon_k, act, is_on) in actions {
        let r = HitRect::new(ax, ay, btn_sz, btn_sz);
        let hov = r.contains(ctx.hover_x, ctx.hover_y);
        let bg = if is_on {
            (pal.solid.0, pal.solid.1, pal.solid.2, 54)
        } else if hov {
            if icon_k == 3 { (244, 63, 94, 72) } else { (31, 31, 35, 255) }
        } else {
            (18, 18, 21, 255)
        };
        let fg = if is_on {
            (pal.light.0, pal.light.1, pal.light.2, 255)
        } else if hov {
            (244, 244, 245, 255)
        } else {
            (161, 161, 170, 240)
        };
        fill_circle(pixmap, r.x + btn_sz * 0.5, r.y + btn_sz * 0.5, btn_sz * 0.5, alpha_scale(bg, op));
        draw_header_action_icon(pixmap, icon_k, r.x + btn_sz * 0.5, r.y + btn_sz * 0.5, alpha_scale(fg, op));
        hitmap.push(r, act);
        ax += btn_sz + btn_gap;
    }

    let page_y = y + hdr_h + 8.0;
    let page_h = (avail_h - hdr_h - 8.0).max(40.0);
    let page_x = x + ctx.page_slide_x as f32;
    let page_op = (op * ctx.page_slide_fade.clamp(0.0, 1.0) as f32).clamp(0.0, 1.0);

    match ctx.active_tab {
        ActiveTab::Media => draw_page_media(pixmap, fonts, ctx, hitmap, page_x, page_y, w, page_h, page_op),
        ActiveTab::Notif => draw_page_notif(pixmap, fonts, ctx, hitmap, page_x, page_y, w, page_h, page_op),
        ActiveTab::Focus => draw_page_focus(pixmap, fonts, ctx, hitmap, page_x, page_y, w, page_h, page_op),
        ActiveTab::Todo => draw_page_todo(pixmap, fonts, ctx, hitmap, page_x, page_y, w, page_h, page_op),
        ActiveTab::Shelf => draw_page_shelf(pixmap, fonts, ctx, hitmap, page_x, page_y, w, page_h, page_op),
        ActiveTab::Prayer => draw_page_prayer(pixmap, fonts, ctx, hitmap, page_x, page_y, w, page_h, page_op),
        ActiveTab::Settings => draw_page_settings(pixmap, fonts, ctx, hitmap, page_x, page_y, w, page_h, page_op),
    }

    // Cleanly mask horizontal overflow during page bounce-slide so content glides inside the notch walls
    if ctx.page_slide_x.abs() > 0.1 {
        let style = ctx.app_state.island_style.as_str();
        let bg_rgb = match style {
            "notch" => (6u8, 6u8, 10u8),
            "dock" => (8u8, 8u8, 12u8),
            _ => (7u8, 7u8, 11u8),
        };
        let wall_pad = if style == "notch" { 6.0 } else { 20.0 };
        let clear_l = (x - wall_pad).max(0.0).ceil() as usize;
        let clear_r = ((x + w + wall_pad).floor() as usize).min(pixmap.width() as usize);
        let notch_left = (x - 2.0).max(0.0).floor() as usize;
        let notch_right = ((x + w + 2.0).ceil() as usize).min(pixmap.width() as usize);
        let y_start = (page_y.floor().max(0.0) as usize).min(pixmap.height() as usize);
        let y_end = ((page_y + page_h).ceil().max(0.0) as usize).min(pixmap.height() as usize);
        let pw_usize = pixmap.width() as usize;
        let data = pixmap.data_mut();
        for row in y_start..y_end {
            let row_off = row * pw_usize * 4;
            if clear_l > 0 {
                data[row_off..row_off + clear_l * 4].fill(0);
            }
            if clear_r < pw_usize {
                data[row_off + clear_r * 4..row_off + pw_usize * 4].fill(0);
            }
            // Tuck sliding edge cleanly behind inner notch margin (leaving anti-aliased corners intact)
            for col in clear_l..notch_left.min(pw_usize) {
                let idx = row_off + col * 4;
                if data[idx + 3] == 255 {
                    data[idx] = bg_rgb.0;
                    data[idx + 1] = bg_rgb.1;
                    data[idx + 2] = bg_rgb.2;
                }
            }
            for col in notch_right..clear_r {
                let idx = row_off + col * 4;
                if data[idx + 3] == 255 {
                    data[idx] = bg_rgb.0;
                    data[idx + 1] = bg_rgb.1;
                    data[idx + 2] = bg_rgb.2;
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_pill_btn_sized(
    pixmap: &mut Pixmap,
    fonts: &mut FontEngine,
    ctx: &RuntimeContext,
    hitmap: &mut UiHitMap,
    x: f32,
    y: f32,
    bh: f32,
    pad_x: f32,
    font_sz: f32,
    label: &str,
    primary: bool,
    op: f32,
    action: UiAction,
) -> f32 {
    let tw = fonts.measure_width(label, font_sz, true);
    let bw = (tw + pad_x * 2.0).max(28.0);
    let r = HitRect::new(x, y, bw, bh);
    let hov = r.contains(ctx.hover_x, ctx.hover_y);
    let bg = if primary {
        if hov { (228, 228, 231, 255) } else { (244, 244, 245, 255) }
    } else if hov {
        (39, 39, 42, 255)
    } else {
        (24, 24, 27, 255)
    };
    let fg = if primary {
        (9, 9, 11, 255)
    } else if hov {
        (244, 244, 245, 255)
    } else {
        (228, 228, 231, 240)
    };
    fill_round_rect(pixmap, x, y, bw, bh, bh * 0.5, alpha_scale(bg, op));
    fonts.draw_text_centered_y(pixmap, x + (bw - tw) * 0.5, y + bh * 0.5, label, font_sz, true, alpha_scale(fg, op), None);
    hitmap.push(r, action);
    bw
}

#[allow(clippy::too_many_arguments)]
fn draw_pill_btn(
    pixmap: &mut Pixmap,
    fonts: &mut FontEngine,
    ctx: &RuntimeContext,
    hitmap: &mut UiHitMap,
    x: f32,
    y: f32,
    label: &str,
    primary: bool,
    op: f32,
    action: UiAction,
) -> f32 {
    draw_pill_btn_sized(pixmap, fonts, ctx, hitmap, x, y, 25.0, 10.0, 11.4, label, primary, op, action)
}

// =============================================================================
// PAGE 1: MEDIA + 3-LINE ROLLING KARAOKE LYRICS
// =============================================================================

fn draw_album_art(pixmap: &mut Pixmap, art: &AlbumArt, x: f32, y: f32, op: f32) {
    let op = op.clamp(0.0, 1.0);
    if op <= 0.01 {
        return;
    }
    let pw = pixmap.width() as i32;
    let ph = pixmap.height() as i32;
    let sz = art.size as i32;
    let ox = x.round() as i32;
    let oy = y.round() as i32;
    let dst = pixmap.data_mut();
    let src = &art.premul_rgba;

    for row in 0..sz {
        let dy = oy + row;
        if dy < 0 || dy >= ph {
            continue;
        }
        let s_row = (row * sz) as usize * 4;
        let d_row = (dy * pw) as usize * 4;
        for col in 0..sz {
            let dx = ox + col;
            if dx < 0 || dx >= pw {
                continue;
            }
            let si = s_row + col as usize * 4;
            let sa_raw = src[si + 3];
            if sa_raw == 0 {
                continue;
            }
            let di = d_row + dx as usize * 4;
            if sa_raw == 255 && op >= 0.995 {
                dst[di] = src[si];
                dst[di + 1] = src[si + 1];
                dst[di + 2] = src[si + 2];
                dst[di + 3] = 255;
            } else {
                let sr = src[si] as f32 * op;
                let sg = src[si + 1] as f32 * op;
                let sb = src[si + 2] as f32 * op;
                let sa = (sa_raw as f32 / 255.0) * op;
                let inv = 1.0 - sa;
                dst[di] = (sr + dst[di] as f32 * inv + 0.5).min(255.0) as u8;
                dst[di + 1] = (sg + dst[di + 1] as f32 * inv + 0.5).min(255.0) as u8;
                dst[di + 2] = (sb + dst[di + 2] as f32 * inv + 0.5).min(255.0) as u8;
                dst[di + 3] = (sa * 255.0 + dst[di + 3] as f32 * inv + 0.5).min(255.0) as u8;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_page_media(
    pixmap: &mut Pixmap,
    fonts: &mut FontEngine,
    ctx: &RuntimeContext,
    hitmap: &mut UiHitMap,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    op: f32,
) {
    fill_round_rect(pixmap, x, y, w, h, 14.0, alpha_scale((17, 17, 20, 255), op));

    let px = x + 14.0;
    let py = y + 12.0;
    let pw = (w - 28.0).max(80.0);
    let pal = resolve_accent_palette(&ctx.app_state.accent_color);
    let acc = pal.light;

    // 58x58 Album Cover Squircle (real artwork when available, accent music-note fallback otherwise)
    let art_sz = 58.0;
    if let Some(ref art) = ctx.album_art {
        draw_album_art(pixmap, art, px, py, op);
    } else {
        if let Some(art_path) = rounded_rect_path(px, py, art_sz, art_sz, 14.0) {
            let stops = vec![
                GradientStop::new(0.0, Color::from_rgba8(pal.light.0, pal.light.1, pal.light.2, (255.0 * op) as u8)),
                GradientStop::new(1.0, Color::from_rgba8(pal.solid.0, pal.solid.1, pal.solid.2, (255.0 * op) as u8)),
            ];
            if let Some(grad) = LinearGradient::new(
                Point::from_xy(px, py),
                Point::from_xy(px + art_sz, py + art_sz),
                stops,
                SpreadMode::Pad,
                Transform::identity(),
            ) {
                let mut p = Paint::default();
                p.shader = grad;
                p.anti_alias = true;
                pixmap.fill_path(&art_path, &p, FillRule::Winding, Transform::identity(), None);
            }
        }
        draw_music_note_icon(pixmap, px + art_sz * 0.5, py + art_sz * 0.5, 1.85, alpha_scale((9, 9, 11, 240), op));
    }

    // Right Media Controls + 4-bar Visualizer
    let ctrl_w = 152.0;
    let ctrl_x = px + pw - ctrl_w;
    let ctrl_cy = py + art_sz * 0.5;

    let prev_r = HitRect::new(ctrl_x, ctrl_cy - 15.5, 31.0, 31.0);
    let play_r = HitRect::new(ctrl_x + 37.0, ctrl_cy - 17.5, 35.0, 35.0);
    let next_r = HitRect::new(ctrl_x + 78.0, ctrl_cy - 15.5, 31.0, 31.0);

    let prev_bg = if prev_r.contains(ctx.hover_x, ctx.hover_y) { (39, 39, 42, 255) } else { (24, 24, 27, 255) };
    let next_bg = if next_r.contains(ctx.hover_x, ctx.hover_y) { (39, 39, 42, 255) } else { (24, 24, 27, 255) };
    let play_bg = if play_r.contains(ctx.hover_x, ctx.hover_y) { (228, 228, 231, 255) } else { (244, 244, 245, 255) };

    fill_circle(pixmap, prev_r.x + 15.5, ctrl_cy, 15.5, alpha_scale(prev_bg, op));
    fill_circle(pixmap, play_r.x + 17.5, ctrl_cy, 17.5, alpha_scale(play_bg, op));
    fill_circle(pixmap, next_r.x + 15.5, ctrl_cy, 15.5, alpha_scale(next_bg, op));

    draw_prev_icon(pixmap, prev_r.x + 15.5, ctrl_cy, 10.5, alpha_scale((228, 228, 231, 245), op));
    if ctx.cached_media.playing {
        draw_pause_icon(pixmap, play_r.x + 17.5, ctrl_cy, 11.5, alpha_scale((9, 9, 11, 255), op));
    } else {
        draw_play_icon(pixmap, play_r.x + 18.5, ctrl_cy, 11.5, alpha_scale((9, 9, 11, 255), op));
    }
    draw_next_icon(pixmap, next_r.x + 15.5, ctrl_cy, 10.5, alpha_scale((228, 228, 231, 245), op));

    hitmap.push(prev_r, UiAction::MediaPrev);
    hitmap.push(play_r, UiAction::MediaPlayPause);
    hitmap.push(next_r, UiAction::MediaNext);

    let vis_x = ctrl_x + 122.0;
    let base_heights = [5.5f32, 9.5, 13.5, 17.0];
    for (i, &bh_idle) in base_heights.iter().enumerate() {
        let bh = if ctx.cached_media.has_player && ctx.cached_media.playing {
            let wave = ((ctx.wave_phase * 1.15 + i as f64 * 1.15).sin() * 0.5 + 0.5).clamp(0.15, 1.0) as f32;
            4.5 + wave * 14.5
        } else {
            bh_idle
        };
        fill_round_rect(
            pixmap,
            vis_x + i as f32 * 6.4,
            ctrl_cy - bh * 0.5,
            3.2,
            bh,
            1.6,
            alpha_scale((acc.0, acc.1, acc.2, 245), op),
        );
    }

    let tx = px + art_sz + 14.0;
    let tw = (ctrl_x - tx - 12.0).max(60.0);
    let state_word = if ctx.cached_media.playing {
        "DIPUTAR"
    } else if ctx.cached_media.has_player {
        "DIJEDA"
    } else {
        "SIAP"
    };
    let src_tag = format!(
        "{} • {} • {}",
        ctx.cached_media.player_name.to_uppercase(),
        state_word,
        ctx.lyric_provider_badge
    );
    fonts.draw_text_centered_y(pixmap, tx, py + 9.0, &src_tag, 10.4, true, alpha_scale((pal.light.0, pal.light.1, pal.light.2, 250), op), Some(tw));
    fonts.draw_text_centered_y(pixmap, tx, py + 29.0, &ctx.cached_media.title, 15.2, true, alpha_scale((244, 244, 245, 255), op), Some(tw));
    fonts.draw_text_centered_y(pixmap, tx, py + 47.0, &ctx.cached_media.artist, 12.2, false, alpha_scale((161, 161, 170, 240), op), Some(tw));

    // Seek Bar
    let seek_top = py + art_sz + 14.0;
    let pos_sec = ctx.interp_pos_sec.max(0.0);
    let len_sec = ctx.cached_media.length_secs as f64;
    let frac = if len_sec > 0.5 {
        (pos_sec / len_sec).clamp(0.0, 1.0) as f32
    } else if ctx.cached_media.playing {
        ctx.lyric_line_prog.clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    let track_h = if ctx.media_seek_dragging { 5.8 } else { 4.2 };
    let seek_hit = HitRect::new(px, seek_top - 8.0, pw, 22.0);
    hitmap.seek_bar = seek_hit;
    hitmap.push(seek_hit, UiAction::MediaSeekBar);

    fill_round_rect(
        pixmap,
        px,
        seek_top,
        pw,
        track_h,
        track_h * 0.5,
        alpha_scale((255, 255, 255, if ctx.media_seek_dragging { 56 } else { 36 }), op),
    );
    let fill_w = (pw * frac).max(if frac > 0.002 { track_h } else { 0.0 });
    if fill_w > 0.0 {
        fill_round_rect(pixmap, px, seek_top, fill_w, track_h, track_h * 0.5, alpha_scale((acc.0, acc.1, acc.2, 245), op));
    }
    let knob_r = if ctx.media_seek_dragging { 6.6 } else { 5.0 };
    let knob_x = (px + pw * frac).clamp(px + knob_r, px + pw - knob_r);
    fill_circle(pixmap, knob_x, seek_top + track_h * 0.5, knob_r, alpha_scale((250, 252, 255, 250), op));
    if ctx.media_seek_dragging {
        fill_circle(pixmap, knob_x, seek_top + track_h * 0.5, 2.4, alpha_scale((acc.0, acc.1, acc.2, 245), op));
    }

    let l_time = format_sec_short(pos_sec);
    let r_time = if len_sec > 0.5 { format_sec_short(len_sec) } else { "0:00".to_string() };
    fonts.draw_text_centered_y(pixmap, px, seek_top + 14.0, &l_time, 11.0, false, alpha_scale((143, 148, 161, 235), op), None);
    let rtw = fonts.measure_width(&r_time, 11.0, false);
    fonts.draw_text_centered_y(pixmap, px + pw - rtw, seek_top + 14.0, &r_time, 11.0, false, alpha_scale((143, 148, 161, 235), op), None);

    // 3-Line Bottom-to-Top Rolling Karaoke Lyrics (hidden cleanly when no lyrics are active)
    if ctx.cur_lyric.is_empty() {
        return;
    }

    let lyr_top = seek_top + 24.0;
    let lyr_h = (y + h - lyr_top - 6.0).max(52.0);
    let lyr_clip = HitRect::new(px, lyr_top, pw, lyr_h);

    let t = ctx.media_lyric_anim_prog.clamp(0.0, 1.0) as f32;
    // Quartic ease-out (fast responsive start -> long silky deceleration settle)
    let ease = 1.0 - (1.0 - t).powi(4);

    let prev_txt = ctx.prev_lyric.as_str();
    let main_txt = ctx.cur_lyric.as_str();
    let next_txt = ctx.next_lyric.as_str();

    let y_prev = lyr_top + 13.0;
    let y_main = lyr_top + 39.0;
    let y_next = lyr_top + 63.0;
    let sub_sz = 12.0f32;
    let active_max_sz = if main_txt.chars().count() > 48 { 14.5 } else { 17.5 };

    // A. Outgoing old previous line slides out of the top edge & fades out
    if t < 0.99 && !ctx.prev_prev_lyric.is_empty() && !ctx.cur_lyric.is_empty() {
        let y_out = y_prev - 18.0 * ease;
        let a = (112.0 * (1.0 - ease).powi(2) * op) as u8;
        let tr = fonts.truncate_to_width(&ctx.prev_prev_lyric, sub_sz, true, pw);
        fonts.draw_text_clipped(pixmap, px, y_out, &tr, sub_sz, true, (145, 150, 164, a), Some(lyr_clip));
    }

    // B. Previous line: LARGE -> SMALL (shrinks & dims smoothly while rolling up from main -> prev)
    if !prev_txt.is_empty() {
        let (yp, prev_sz, prev_col) = if !ctx.cur_lyric.is_empty() {
            let from_sz = if prev_txt.chars().count() > 48 { 14.5 } else { 17.5 };
            let sz = from_sz + (sub_sz - from_sz) * ease;
            let r = (248.0 + (140.0 - 248.0) * ease) as u8;
            let g = (250.0 + (145.0 - 250.0) * ease) as u8;
            let b = (255.0 + (158.0 - 255.0) * ease) as u8;
            let a = (242.0 + (116.0 - 242.0) * ease) as u8;
            (y_main + (y_prev - y_main) * ease, sz, (r, g, b, a))
        } else {
            (y_prev, sub_sz, (140, 145, 158, 116))
        };
        let tr = fonts.truncate_to_width(prev_txt, prev_sz, true, pw);
        fonts.draw_text_clipped(pixmap, px, yp, &tr, prev_sz, true, alpha_scale(prev_col, op), Some(lyr_clip));
    }

    // C. Main active line: SMALL -> LARGE (grows smoothly while rolling up from next -> main)
    let (ym, main_sz) = if !ctx.cur_lyric.is_empty() {
        let sz = sub_sz + (active_max_sz - sub_sz) * ease;
        (y_next + (y_main - y_next) * ease, sz)
    } else {
        (y_main, active_max_sz)
    };

    if main_txt == "●  ●  ●" {
        for i in 0..3 {
            let dx = px + 8.0 + i as f32 * 16.0;
            let active = i <= ctx.active_word_idx;
            fill_circle(
                pixmap,
                dx,
                ym - 5.0,
                if active { 4.2 } else { 3.3 },
                alpha_scale(if active { (250, 252, 255, 245) } else { (132, 140, 155, 110) }, op),
            );
        }
    } else if ctx.lyric_is_word_synced && !ctx.lyric_words.is_empty() && !ctx.cur_lyric.is_empty() {
        let sp_w = fonts.measure_width(" ", main_sz, true).max(4.5);
        let mut total_w = 0.0f32;
        let mut active_center = 0.0f32;
        let mut wpos = Vec::with_capacity(ctx.lyric_words.len());
        for (idx, item) in ctx.lyric_words.iter().enumerate() {
            let ww = fonts.measure_width(&item.text, main_sz, true);
            if idx == ctx.active_word_idx {
                active_center = total_w + ww * 0.5;
            }
            wpos.push((idx, ww, total_w));
            total_w += ww + if item.space_after { sp_w } else { 0.0 };
        }
        let base_x = if total_w > pw {
            px - (active_center - pw * 0.4).clamp(0.0, total_w - pw)
        } else {
            px
        };
        let lyric_style = ctx.app_state.lyric_style.as_str();
        for (idx, ww, rx) in wpos {
            let wx = base_x + rx;
            let txt = &ctx.lyric_words[idx].text;
            if idx < ctx.active_word_idx {
                let col = if lyric_style == "cinema_glow" {
                    (205, 212, 228, 160)
                } else {
                    (250, 252, 255, 250)
                };
                fonts.draw_text_clipped(pixmap, wx, ym, txt, main_sz, true, alpha_scale(col, op), Some(lyr_clip));
            } else if idx == ctx.active_word_idx {
                let bounce = if lyric_style == "word_bounce" {
                    -2.4 * ((ctx.word_prog as f32) * PI).sin()
                } else {
                    0.0
                };
                let wy = ym + bounce;
                if lyric_style == "cinema_glow" {
                    let gx0 = (wx - 5.0).max(lyr_clip.x);
                    let gx1 = (wx + ww + 5.0).min(lyr_clip.x + lyr_clip.w);
                    if gx1 > gx0 {
                        fill_round_rect(
                            pixmap,
                            gx0,
                            wy - 15.0,
                            gx1 - gx0,
                            21.0,
                            10.0,
                            alpha_scale((acc.0, acc.1, acc.2, 46), op),
                        );
                    }
                }
                fonts.draw_text_clipped(pixmap, wx, wy, txt, main_sz, true, alpha_scale((122, 128, 140, 175), op), Some(lyr_clip));
                let fw = if lyric_style == "cinema_glow" {
                    ww + 1.5
                } else {
                    (ww * (ctx.word_prog as f32).clamp(0.0, 1.0)) + 1.0
                };
                let wclip = HitRect::new(
                    wx.max(lyr_clip.x),
                    lyr_clip.y,
                    (wx + fw).min(lyr_clip.x + lyr_clip.w) - wx.max(lyr_clip.x),
                    lyr_clip.h,
                );
                fonts.draw_text_clipped(pixmap, wx, wy, txt, main_sz, true, alpha_scale((250, 252, 255, 255), op), Some(wclip));
            } else {
                let dim_a = if lyric_style == "cinema_glow" { 105 } else { 184 };
                fonts.draw_text_clipped(pixmap, wx, ym, txt, main_sz, true, alpha_scale((122, 128, 140, dim_a), op), Some(lyr_clip));
            }
        }
    } else {
        // Static / line-synced lyric (not word-by-word): render clean solid text without karaoke sweep
        let mw = fonts.measure_width(main_txt, main_sz, true);
        let base_x = if mw > pw && !ctx.cur_lyric.is_empty() {
            px - (mw - pw) * ctx.lyric_line_prog.clamp(0.0, 1.0) as f32
        } else {
            px
        };
        fonts.draw_text_clipped(
            pixmap,
            base_x,
            ym,
            main_txt,
            main_sz,
            true,
            alpha_scale((250, 252, 255, 252), op),
            Some(lyr_clip),
        );
    }

    if !next_txt.is_empty() {
        let yn = if !ctx.cur_lyric.is_empty() { (y_next + 22.0) + (y_next - (y_next + 22.0)) * ease } else { y_next };
        let tr = fonts.truncate_to_width(next_txt, sub_sz, true, pw);
        fonts.draw_text_clipped(pixmap, px, yn, &tr, sub_sz, true, alpha_scale((118, 122, 136, (108.0 * ease) as u8), op), Some(lyr_clip));
    }
}

// =============================================================================
// PAGE 2: NOTIFIKASI
// =============================================================================

#[allow(clippy::too_many_arguments)]
fn draw_page_notif(
    pixmap: &mut Pixmap,
    fonts: &mut FontEngine,
    ctx: &RuntimeContext,
    hitmap: &mut UiHitMap,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    op: f32,
) {
    fill_round_rect(pixmap, x, y, w, h, 14.0, alpha_scale((17, 17, 20, 255), op));
    let pal = resolve_accent_palette(&ctx.app_state.accent_color);
    let px = x + 14.0;
    let py = y + 10.0;
    let pw = (w - 28.0).max(80.0);
    let head_cy = py + 12.0;

    let title_w = fonts.draw_text_centered_y(pixmap, px, head_cy, "PUSAT NOTIFIKASI", 10.6, true, alpha_scale((pal.light.0, pal.light.1, pal.light.2, 255), op), None);
    let badge = format!("{} Baru", ctx.unread_notif_count);
    let bw = fonts.measure_width(&badge, 10.2, true) + 14.0;
    fill_round_rect(pixmap, px + title_w + 8.0, head_cy - 9.0, bw, 18.0, 9.0, alpha_scale((pal.solid.0, pal.solid.1, pal.solid.2, 42), op));
    fonts.draw_text_centered_y(pixmap, px + title_w + 15.0, head_cy, &badge, 10.2, true, alpha_scale((pal.light.0, pal.light.1, pal.light.2, 255), op), None);

    // Right buttons: Tes | Baca Semua | Bersihkan (clean spacing, never collides)
    let w_clear = fonts.measure_width("Bersihkan", 11.2, true) + 18.0;
    let w_read = fonts.measure_width("Baca Semua", 11.2, true) + 18.0;
    let w_test = fonts.measure_width("Tes Notch", 11.2, true) + 18.0;
    let mut bx = px + pw - w_clear;
    draw_pill_btn_sized(pixmap, fonts, ctx, hitmap, bx, py, 24.0, 9.0, 11.2, "Bersihkan", false, op, UiAction::NotifClear);
    bx -= w_read + 6.0;
    draw_pill_btn_sized(pixmap, fonts, ctx, hitmap, bx, py, 24.0, 9.0, 11.2, "Baca Semua", false, op, UiAction::NotifMarkRead);
    bx -= w_test + 6.0;
    draw_pill_btn_sized(pixmap, fonts, ctx, hitmap, bx, py, 24.0, 9.0, 11.2, "Tes Notch", true, op, UiAction::NotifTest);

    if ctx.notif_history.is_empty() {
        let msg = "Belum ada notifikasi masuk — klik 'Tes Notch' untuk mencoba.";
        let mw = fonts.measure_width(msg, 12.2, false);
        fonts.draw_text_centered_y(pixmap, px + ((pw - mw) * 0.5).max(0.0), py + 62.0, msg, 12.2, false, alpha_scale((161, 161, 170, 200), op), Some(pw));
        return;
    }

    let mut cy = py + 32.0;
    let start = ctx.notif_scroll_idx.min(ctx.notif_history.len().saturating_sub(1));
    for item in ctx.notif_history.iter().skip(start).take(3) {
        let card_h = 44.0;
        if cy + card_h > y + h - 4.0 {
            break;
        }
        let card_bg = if item.urgency >= 2 {
            (244, 63, 94, 26)
        } else if !item.read {
            (pal.solid.0, pal.solid.1, pal.solid.2, 22)
        } else {
            (22, 22, 26, 255)
        };
        fill_round_rect(pixmap, px, cy, pw, card_h, 11.0, alpha_scale(card_bg, op));

        let (nr, ng, nb) = item.accent_rgb();
        fill_round_rect(
            pixmap,
            px + 8.0,
            cy + 7.0,
            30.0,
            30.0,
            9.0,
            alpha_scale(((nr * 255.0) as u8, (ng * 255.0) as u8, (nb * 255.0) as u8, 46), op),
        );
        draw_notif_category_icon(pixmap, fonts, item, px + 23.0, cy + 22.0, op, (nr, ng, nb));

        let tx = px + 46.0;
        let tw = (pw - 84.0).max(60.0);
        let top = format!("{} • {}", item.app_name, item.summary);
        if item.body.is_empty() {
            fonts.draw_text_centered_y(pixmap, tx, cy + card_h * 0.5, &top, 12.2, true, alpha_scale((244, 244, 245, 250), op), Some(tw));
        } else {
            fonts.draw_text_centered_y(pixmap, tx, cy + 14.5, &top, 12.2, true, alpha_scale((244, 244, 245, 250), op), Some(tw));
            fonts.draw_text_centered_y(pixmap, tx, cy + 31.0, &item.body, 11.4, false, alpha_scale((161, 161, 170, 220), op), Some(tw));
        }

        let del_r = HitRect::new(px + pw - 27.0, cy + 12.0, 20.0, 20.0);
        if del_r.contains(ctx.hover_x, ctx.hover_y) {
            fill_circle(pixmap, del_r.x + 10.0, del_r.y + 10.0, 10.0, alpha_scale((244, 63, 94, 65), op));
        }
        draw_header_action_icon(pixmap, 3, del_r.x + 10.0, del_r.y + 10.0, alpha_scale((161, 161, 170, 210), op));
        hitmap.push(del_r, UiAction::NotifRemove(item.id));

        cy += card_h + 6.0;
    }
}

// =============================================================================
// PAGE 3: FOKUS (POMODORO + ALARM PENGINGAT — Clean Uncrowded 2-Card Layout)
// =============================================================================

#[allow(clippy::too_many_arguments)]
fn draw_page_focus(
    pixmap: &mut Pixmap,
    fonts: &mut FontEngine,
    ctx: &RuntimeContext,
    hitmap: &mut UiHitMap,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    op: f32,
) {
    let gap = 10.0;
    let cw = (w - gap) * 0.5;
    let pal = resolve_accent_palette(&ctx.app_state.accent_color);

    // LEFT CARD: POMODORO FOKUS
    fill_round_rect(pixmap, x, y, cw, h, 14.0, alpha_scale((17, 17, 20, 255), op));
    let lx = x + 13.0;
    let ly = y + 12.0;
    let lw = cw - 26.0;

    let ptag = if ctx.pomo_break_mode { "REHAT" } else { "FOKUS" };
    fonts.draw_text_centered_y(pixmap, lx, ly + 6.0, ptag, 10.4, true, alpha_scale((pal.light.0, pal.light.1, pal.light.2, 255), op), None);
    let sess = format!("{} Sesi", ctx.app_state.pomo_completed_sessions);
    let sw = fonts.measure_width(&sess, 10.6, false);
    fonts.draw_text_centered_y(pixmap, lx + lw - sw, ly + 6.0, &sess, 10.6, false, alpha_scale((161, 161, 170, 220), op), None);

    let pm = ctx.pomo_remaining_secs / 60;
    let ps = ctx.pomo_remaining_secs % 60;
    fonts.draw_text_centered_y(
        pixmap,
        lx,
        ly + 35.0,
        &format!("{:02}:{:02}", pm, ps),
        27.0,
        true,
        alpha_scale((244, 244, 245, 255), op),
        None,
    );

    // Compact right-aligned quick adjust pills (never overlap digits!)
    let mut rx_cur = lx + lw;
    for (lbl, delta) in [("+5", 5), ("+1", 1), ("-1", -1), ("-5", -5)] {
        let bw = (fonts.measure_width(lbl, 10.6, true) + 12.0).max(28.0);
        rx_cur -= bw;
        draw_pill_btn_sized(pixmap, fonts, ctx, hitmap, rx_cur, ly + 23.0, 23.0, 6.0, 10.6, lbl, false, op, UiAction::PomoAdjust(delta));
        rx_cur -= 4.0;
    }

    let p_frac = if ctx.pomo_total_secs > 0 {
        (ctx.pomo_remaining_secs as f32 / ctx.pomo_total_secs as f32).clamp(0.0, 1.0)
    } else {
        1.0
    };
    fill_round_rect(pixmap, lx, ly + 60.0, lw, 5.0, 2.5, alpha_scale((39, 39, 42, 255), op));
    fill_round_rect(pixmap, lx, ly + 60.0, (lw * p_frac).max(5.0), 5.0, 2.5, alpha_scale((pal.solid.0, pal.solid.1, pal.solid.2, 255), op));

    let btn_y = ly + 76.0;
    let start_lbl = if ctx.pomo_running { "Jeda" } else { "Mulai" };
    let w1 = draw_pill_btn(pixmap, fonts, ctx, hitmap, lx, btn_y, start_lbl, true, op, UiAction::PomoStartPause);
    let w2 = draw_pill_btn(pixmap, fonts, ctx, hitmap, lx + w1 + 6.0, btn_y, "Reset", false, op, UiAction::PomoReset);
    let mode_lbl = if ctx.pomo_break_mode {
        format!("Fokus {}m", ctx.app_state.pomo_focus_mins)
    } else {
        format!("Rehat {}m", ctx.app_state.pomo_break_mins)
    };
    draw_pill_btn(pixmap, fonts, ctx, hitmap, lx + w1 + w2 + 12.0, btn_y, &mode_lbl, false, op, UiAction::PomoToggleMode);

    // RIGHT CARD: ALARM PENGINGAT
    let rx = x + cw + gap;
    fill_round_rect(pixmap, rx, y, cw, h, 14.0, alpha_scale((17, 17, 20, 255), op));
    let rlx = rx + 13.0;

    fonts.draw_text_centered_y(pixmap, rlx, ly + 6.0, "ALARM PENGINGAT", 10.4, true, alpha_scale((pal.light.0, pal.light.1, pal.light.2, 255), op), None);
    let astat = if ctx.alarm_ringing {
        "BERBUNYI!"
    } else if ctx.app_state.alarm_enabled {
        "AKTIF"
    } else {
        "NONAKTIF"
    };
    let asw = fonts.measure_width(astat, 10.4, true);
    let acol = if ctx.alarm_ringing {
        (251, 113, 133, 255)
    } else if ctx.app_state.alarm_enabled {
        (pal.light.0, pal.light.1, pal.light.2, 255)
    } else {
        (161, 161, 170, 220)
    };
    fonts.draw_text_centered_y(pixmap, rlx + lw - asw, ly + 6.0, astat, 10.4, true, alpha_scale(acol, op), None);

    fonts.draw_text_centered_y(
        pixmap,
        rlx,
        ly + 35.0,
        &format!("{:02}:{:02}", ctx.app_state.alarm_hour.min(23), ctx.app_state.alarm_min.min(59)),
        27.0,
        true,
        alpha_scale((244, 244, 245, 255), op),
        None,
    );

    let mut arx = rlx + lw;
    let alarm_pills = [
        ("+5", UiAction::AlarmAdjustMin(5)),
        ("+1", UiAction::AlarmAdjustMin(1)),
        ("-5", UiAction::AlarmAdjustMin(-5)),
        ("+1j", UiAction::AlarmAdjustHour(1)),
    ];
    for (lbl, act) in alarm_pills {
        let bw = (fonts.measure_width(lbl, 10.6, true) + 12.0).max(28.0);
        arx -= bw;
        draw_pill_btn_sized(pixmap, fonts, ctx, hitmap, arx, ly + 23.0, 23.0, 6.0, 10.6, lbl, false, op, act);
        arx -= 4.0;
    }

    fill_round_rect(pixmap, rlx, ly + 60.0, lw, 5.0, 2.5, alpha_scale((39, 39, 42, 255), op));
    if ctx.app_state.alarm_enabled || ctx.alarm_ringing {
        let bar_col = if ctx.alarm_ringing { (244, 63, 94, 255) } else { (pal.solid.0, pal.solid.1, pal.solid.2, 255) };
        fill_round_rect(pixmap, rlx, ly + 60.0, lw, 5.0, 2.5, alpha_scale(bar_col, op));
    }

    let tog_lbl = if ctx.alarm_ringing {
        "Stop"
    } else if ctx.app_state.alarm_enabled {
        "Matikan"
    } else {
        "Aktifkan"
    };
    let aw1 = draw_pill_btn(pixmap, fonts, ctx, hitmap, rlx, btn_y, tog_lbl, true, op, UiAction::AlarmToggle);
    let aw2 = draw_pill_btn(pixmap, fonts, ctx, hitmap, rlx + aw1 + 6.0, btn_y, "Tunda 5m", false, op, UiAction::AlarmSnooze5);
    draw_pill_btn(pixmap, fonts, ctx, hitmap, rlx + aw1 + aw2 + 12.0, btn_y, "-1j", false, op, UiAction::AlarmAdjustHour(-1));
}

// =============================================================================
// PAGE 4: TODO
// =============================================================================

#[allow(clippy::too_many_arguments)]
fn draw_page_todo(
    pixmap: &mut Pixmap,
    fonts: &mut FontEngine,
    ctx: &RuntimeContext,
    hitmap: &mut UiHitMap,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    op: f32,
) {
    fill_round_rect(pixmap, x, y, w, h, 14.0, alpha_scale((17, 17, 20, 255), op));
    let pal = resolve_accent_palette(&ctx.app_state.accent_color);
    let px = x + 14.0;
    let py = y + 10.0;
    let pw = (w - 28.0).max(80.0);

    let add_w = fonts.measure_width("+ Tambah", 11.4, true) + 20.0;
    let entry_w = (pw - add_w - 8.0).max(80.0);
    let entry_r = HitRect::new(px, py, entry_w, 27.0);
    let focused = ctx.focused_input == Some(InputField::Todo);
    fill_round_rect(
        pixmap,
        entry_r.x,
        entry_r.y,
        entry_r.w,
        entry_r.h,
        8.5,
        alpha_scale(if focused { (30, 30, 36, 255) } else { (24, 24, 27, 255) }, op),
    );
    hitmap.push(entry_r, UiAction::FocusInput(InputField::Todo));

    let display = if ctx.todo_draft.is_empty() && !focused {
        ("Tulis tugas atau catatan baru…", (120, 120, 132, 220))
    } else {
        (ctx.todo_draft.as_str(), (244, 244, 245, 255))
    };
    let text_w = fonts.draw_text_centered_y(
        pixmap,
        entry_r.x + 11.0,
        entry_r.y + entry_r.h * 0.5,
        display.0,
        12.0,
        false,
        alpha_scale(display.1, op),
        Some(entry_w - 22.0),
    );
    if focused && (ctx.cursor_blink.sin() >= 0.0) {
        let cx = (entry_r.x + 11.0 + text_w + 1.0).min(entry_r.x + entry_w - 8.0);
        draw_line(pixmap, cx, entry_r.y + 6.0, cx, entry_r.y + 21.0, 1.5, alpha_scale((pal.light.0, pal.light.1, pal.light.2, 255), op));
    }

    draw_pill_btn(pixmap, fonts, ctx, hitmap, px + entry_w + 8.0, py + 1.0, "+ Tambah", true, op, UiAction::TodoAdd);

    if ctx.app_state.todos.is_empty() {
        let msg = "Belum ada tugas. Klik kotak di atas, ketik, lalu tekan Enter.";
        let mw = fonts.measure_width(msg, 12.0, false);
        fonts.draw_text_centered_y(pixmap, px + ((pw - mw) * 0.5).max(0.0), py + 68.0, msg, 12.0, false, alpha_scale((161, 161, 170, 200), op), Some(pw));
        return;
    }

    let mut cy = py + 35.0;
    let start = ctx.todo_scroll_idx.min(ctx.app_state.todos.len().saturating_sub(1));
    for (offset, item) in ctx.app_state.todos.iter().skip(start).take(4).enumerate() {
        let idx = start + offset;
        let row_h = 29.0;
        if cy + row_h > y + h - 4.0 {
            break;
        }
        let row_r = HitRect::new(px, cy, pw, row_h);
        let hov = row_r.contains(ctx.hover_x, ctx.hover_y);
        fill_round_rect(pixmap, px, cy, pw, row_h, 9.0, alpha_scale(if hov { (28, 28, 34, 255) } else { (22, 22, 26, 255) }, op));

        let cb_r = HitRect::new(px + 9.0, cy + 7.0, 15.0, 15.0);
        fill_round_rect(
            pixmap,
            cb_r.x,
            cb_r.y,
            15.0,
            15.0,
            4.5,
            alpha_scale(if item.completed { (pal.solid.0, pal.solid.1, pal.solid.2, 255) } else { (39, 39, 42, 255) }, op),
        );
        if item.completed {
            draw_line(pixmap, cb_r.x + 3.5, cb_r.y + 7.8, cb_r.x + 6.5, cb_r.y + 10.8, 1.8, alpha_scale((9, 9, 11, 255), op));
            draw_line(pixmap, cb_r.x + 6.5, cb_r.y + 10.8, cb_r.x + 11.5, cb_r.y + 4.5, 1.8, alpha_scale((9, 9, 11, 255), op));
        }
        hitmap.push(HitRect::new(px, cy, pw - 30.0, row_h), UiAction::TodoToggle(idx));

        let txt_col = if item.completed { (120, 120, 132, 220) } else { (244, 244, 245, 250) };
        fonts.draw_text_centered_y(pixmap, px + 32.0, cy + row_h * 0.5, &item.text, 12.2, false, alpha_scale(txt_col, op), Some(pw - 64.0));

        let del_r = HitRect::new(px + pw - 25.0, cy + 4.5, 20.0, 20.0);
        if del_r.contains(ctx.hover_x, ctx.hover_y) {
            fill_circle(pixmap, del_r.x + 10.0, del_r.y + 10.0, 9.0, alpha_scale((244, 63, 94, 65), op));
        }
        draw_header_action_icon(pixmap, 3, del_r.x + 10.0, del_r.y + 10.0, alpha_scale((161, 161, 170, 200), op));
        hitmap.push(del_r, UiAction::TodoDelete(idx));

        cy += row_h + 5.0;
    }
}

// =============================================================================
// PAGE 5: RAK / FILE SHELF
// =============================================================================

#[allow(clippy::too_many_arguments)]
fn draw_page_shelf(
    pixmap: &mut Pixmap,
    fonts: &mut FontEngine,
    ctx: &RuntimeContext,
    hitmap: &mut UiHitMap,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    op: f32,
) {
    fill_round_rect(pixmap, x, y, w, h, 14.0, alpha_scale((17, 17, 20, 255), op));
    let px = x + 14.0;
    let py = y + 10.0;
    let pw = (w - 28.0).max(80.0);

    fill_round_rect(pixmap, px, py, pw, 34.0, 10.0, alpha_scale((22, 22, 26, 255), op));
    let pal = resolve_accent_palette(&ctx.app_state.accent_color);
    draw_tab_icon(pixmap, 4, px + 16.0, py + 17.0, alpha_scale((pal.light.0, pal.light.1, pal.light.2, 240), op));
    fonts.draw_text_centered_y(
        pixmap,
        px + 30.0,
        py + 17.0,
        "Simpan pintasan file cepat ke Rak Arwiland",
        12.0,
        false,
        alpha_scale((161, 161, 170, 235), op),
        Some(pw - 170.0),
    );
    let w_clr = fonts.measure_width("Hapus", 11.4, true) + 20.0;
    let w_pick = fonts.measure_width("Pilih File", 11.4, true) + 20.0;
    draw_pill_btn(pixmap, fonts, ctx, hitmap, px + pw - w_clr - 6.0, py + 4.5, "Hapus", false, op, UiAction::ShelfClear);
    draw_pill_btn(pixmap, fonts, ctx, hitmap, px + pw - w_clr - w_pick - 12.0, py + 4.5, "Pilih File", true, op, UiAction::ShelfPickFile);

    if ctx.app_state.shelf_files.is_empty() {
        let msg = "Rak file kosong — klik 'Pilih File' untuk menambahkan file.";
        let mw = fonts.measure_width(msg, 12.0, false);
        fonts.draw_text_centered_y(pixmap, px + ((pw - mw) * 0.5).max(0.0), py + 72.0, msg, 12.0, false, alpha_scale((161, 161, 170, 200), op), Some(pw));
        return;
    }

    let mut cy = py + 40.0;
    let start = ctx.shelf_scroll_idx.min(ctx.app_state.shelf_files.len().saturating_sub(1));
    for (offset, f) in ctx.app_state.shelf_files.iter().skip(start).take(4).enumerate() {
        let idx = start + offset;
        let row_h = 29.0;
        if cy + row_h > y + h - 4.0 {
            break;
        }
        let row_r = HitRect::new(px, cy, pw, row_h);
        let hov = row_r.contains(ctx.hover_x, ctx.hover_y);
        fill_round_rect(pixmap, px, cy, pw, row_h, 9.0, alpha_scale(if hov { (28, 28, 34, 255) } else { (22, 22, 26, 255) }, op));

        let sz_str = &f.size_label;
        let sz_w = fonts.measure_width(sz_str, 10.8, false);
        fonts.draw_text_centered_y(pixmap, px + 11.0, cy + row_h * 0.5, &f.name, 12.0, true, alpha_scale((244, 244, 245, 250), op), Some(pw - sz_w - 52.0));
        fonts.draw_text_centered_y(pixmap, px + pw - sz_w - 30.0, cy + row_h * 0.5, sz_str, 10.8, false, alpha_scale((161, 161, 170, 210), op), None);
        hitmap.push(HitRect::new(px, cy, pw - 28.0, row_h), UiAction::ShelfOpen(idx));

        let del_r = HitRect::new(px + pw - 25.0, cy + 4.5, 20.0, 20.0);
        if del_r.contains(ctx.hover_x, ctx.hover_y) {
            fill_circle(pixmap, del_r.x + 10.0, del_r.y + 10.0, 9.0, alpha_scale((244, 63, 94, 65), op));
        }
        draw_header_action_icon(pixmap, 3, del_r.x + 10.0, del_r.y + 10.0, alpha_scale((161, 161, 170, 200), op));
        hitmap.push(del_r, UiAction::ShelfRemove(idx));

        cy += row_h + 5.0;
    }
}

// =============================================================================
// PAGE 6: SHOLAT
// =============================================================================

#[allow(clippy::too_many_arguments)]
fn draw_page_prayer(
    pixmap: &mut Pixmap,
    fonts: &mut FontEngine,
    ctx: &RuntimeContext,
    hitmap: &mut UiHitMap,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    op: f32,
) {
    fill_round_rect(pixmap, x, y, w, h, 14.0, alpha_scale((17, 17, 20, 255), op));
    let px = x + 14.0;
    let py = y + 11.0;
    let pw = (w - 28.0).max(80.0);
    let ps = &ctx.prayer_schedule;

    let city_btn_lbl = format!("{} ({})", ps.city, ps.tz_label);
    let city_bw = (fonts.measure_width(&city_btn_lbl, 11.4, true) + 22.0).min(210.0);
    let city_tr = fonts.truncate_to_width(&city_btn_lbl, 11.4, true, city_bw - 18.0);
    draw_pill_btn(pixmap, fonts, ctx, hitmap, px + pw - city_bw, py + 4.0, &city_tr, false, op, UiAction::PrayerCycleCity);

    let left_w = (pw - city_bw - 10.0).max(100.0);
    let pal = resolve_accent_palette(&ctx.app_state.accent_color);
    fonts.draw_text_centered_y(pixmap, px, py + 7.0, "JADWAL SHOLAT", 10.4, true, alpha_scale((pal.light.0, pal.light.1, pal.light.2, 255), op), None);
    let next_str = format!("{} pukul {} ({})", ps.next_prayer_name, ps.next_prayer_time, ps.countdown_str);
    fonts.draw_text_centered_y(pixmap, px, py + 24.0, &next_str, 14.2, true, alpha_scale((244, 244, 245, 255), op), Some(left_w));

    let sy = py + 39.0;
    let cari_w = fonts.measure_width("Cari", 11.4, true) + 20.0;
    let entry_w = (pw - cari_w - 8.0).max(80.0);
    let entry_r = HitRect::new(px, sy, entry_w, 26.0);
    let focused = ctx.focused_input == Some(InputField::PrayerSearch);
    fill_round_rect(
        pixmap,
        entry_r.x,
        entry_r.y,
        entry_r.w,
        entry_r.h,
        8.5,
        alpha_scale(if focused { (30, 30, 36, 255) } else { (24, 24, 27, 255) }, op),
    );
    hitmap.push(entry_r, UiAction::FocusInput(InputField::PrayerSearch));

    let placeholder = if !ctx.prayer_search_status.is_empty() && ctx.prayer_search_draft.is_empty() && !focused {
        ctx.prayer_search_status.as_str()
    } else {
        "Cari kota atau kecamatan (mis: Bandung, Coblong, Gubeng)…"
    };
    let (disp, col) = if ctx.prayer_search_draft.is_empty() && !focused {
        (placeholder, (120, 120, 132, 220))
    } else {
        (ctx.prayer_search_draft.as_str(), (244, 244, 245, 255))
    };
    let tw = fonts.draw_text_centered_y(pixmap, entry_r.x + 11.0, entry_r.y + entry_r.h * 0.5, disp, 11.8, false, alpha_scale(col, op), Some(entry_w - 22.0));
    if focused && ctx.cursor_blink.sin() >= 0.0 {
        let cx = (entry_r.x + 11.0 + tw + 1.0).min(entry_r.x + entry_w - 8.0);
        draw_line(pixmap, cx, entry_r.y + 5.5, cx, entry_r.y + 20.5, 1.5, alpha_scale((pal.light.0, pal.light.1, pal.light.2, 255), op));
    }
    draw_pill_btn(pixmap, fonts, ctx, hitmap, px + entry_w + 8.0, sy + 0.5, "Cari", true, op, UiAction::PrayerSearchSubmit);

    let count = ps.prayers.len().max(1);
    let gap = 6.0;
    let card_w = (pw - gap * (count as f32 - 1.0)) / count as f32;
    let card_y = sy + 35.0;
    let card_h = (y + h - card_y - 10.0).clamp(48.0, 68.0);

    for (i, (name, time_str, _)) in ps.prayers.iter().enumerate() {
        let cx = px + i as f32 * (card_w + gap);
        let is_next = name.eq_ignore_ascii_case(&ps.next_prayer_name);
        let bg = if is_next { (pal.solid.0, pal.solid.1, pal.solid.2, 46) } else { (24, 24, 27, 255) };
        fill_round_rect(pixmap, cx, card_y, card_w, card_h, 11.0, alpha_scale(bg, op));

        let nw = fonts.measure_width(name, 11.0, true);
        let ncol = if is_next { (pal.light.0, pal.light.1, pal.light.2, 255) } else { (161, 161, 170, 230) };
        fonts.draw_text_centered_y(pixmap, cx + ((card_w - nw) * 0.5).max(3.0), card_y + card_h * 0.34, name, 11.0, true, alpha_scale(ncol, op), Some(card_w - 6.0));

        let t_w = fonts.measure_width(time_str, 13.8, true);
        fonts.draw_text_centered_y(pixmap, cx + ((card_w - t_w) * 0.5).max(3.0), card_y + card_h * 0.70, time_str, 13.8, true, alpha_scale((244, 244, 245, 255), op), None);
    }
}

// =============================================================================
// INLINE PENGATURAN (SETTINGS + SYSTEM METRICS)
// =============================================================================

#[allow(clippy::too_many_arguments)]
fn draw_page_settings(
    pixmap: &mut Pixmap,
    fonts: &mut FontEngine,
    ctx: &RuntimeContext,
    hitmap: &mut UiHitMap,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    op: f32,
) {
    fill_round_rect(pixmap, x, y, w, h, 14.0, alpha_scale((17, 17, 20, 255), op));
    let px = x + 14.0;
    let py = y + 9.0;
    let pw = (w - 28.0).max(80.0);
    let pal = resolve_accent_palette(&ctx.app_state.accent_color);

    fonts.draw_text_centered_y(pixmap, px, py + 7.0, "PENGATURAN ARWILAND", 10.8, true, alpha_scale((pal.light.0, pal.light.1, pal.light.2, 255), op), None);
    let m = &ctx.cached_metrics;
    let sys_txt = format!(
        "CPU {:.0}%  •  RAM {:.1}/{:.0}GB  •  Vol {}%",
        m.cpu_usage, m.ram_used_gb, m.ram_total_gb, ctx.cur_volume_pct
    );
    let sw = fonts.measure_width(&sys_txt, 10.8, false);
    fonts.draw_text_centered_y(pixmap, px + pw - sw, py + 7.0, &sys_txt, 10.8, false, alpha_scale((161, 161, 170, 220), op), None);

    // Full-width Aksen Warna swatch picker (9 curated colors, 1-click live preview)
    let acc_y = py + 19.0;
    let acc_h = 31.0;
    let acc_cy = acc_y + acc_h * 0.5;
    fill_round_rect(pixmap, px, acc_y, pw, acc_h, 9.5, alpha_scale((22, 22, 26, 255), op));
    let lbl_w = fonts.draw_text_centered_y(
        pixmap,
        px + 11.0,
        acc_cy,
        "Aksen Warna",
        11.6,
        false,
        alpha_scale((228, 228, 231, 240), op),
        None,
    );
    let badge_w = fonts.measure_width(pal.label, 10.0, true) + 14.0;
    fill_round_rect(
        pixmap,
        px + 18.0 + lbl_w,
        acc_cy - 9.0,
        badge_w,
        18.0,
        9.0,
        alpha_scale((pal.solid.0, pal.solid.1, pal.solid.2, 46), op),
    );
    fonts.draw_text_centered_y(
        pixmap,
        px + 25.0 + lbl_w,
        acc_cy,
        pal.label,
        10.0,
        true,
        alpha_scale((pal.light.0, pal.light.1, pal.light.2, 255), op),
        None,
    );

    let swatch_stride = 23.0f32;
    let swatch_r = 7.2f32;
    let total_sw_w = (ACCENT_PRESETS.len() as f32 - 1.0) * swatch_stride + swatch_r * 2.0;
    let mut sx = px + pw - total_sw_w - 11.0 + swatch_r;
    for (idx, preset) in ACCENT_PRESETS.iter().enumerate() {
        let hit = HitRect::new(sx - 10.5, acc_cy - 11.0, 21.0, 22.0);
        let selected = preset.id.eq_ignore_ascii_case(&ctx.app_state.accent_color);
        let hovered = hit.contains(ctx.hover_x, ctx.hover_y);
        if selected {
            stroke_circle(pixmap, sx, acc_cy, swatch_r + 2.6, 1.7, alpha_scale((248, 250, 252, 245), op));
        } else if hovered {
            stroke_circle(pixmap, sx, acc_cy, swatch_r + 2.2, 1.3, alpha_scale((200, 204, 214, 175), op));
        }
        let r_draw = if selected { swatch_r - 0.5 } else { swatch_r };
        fill_circle(
            pixmap,
            sx,
            acc_cy,
            r_draw,
            alpha_scale((preset.light.0, preset.light.1, preset.light.2, 255), op),
        );
        hitmap.push(hit, UiAction::SettingSelectAccent(idx));
        sx += swatch_stride;
    }

    let col_gap = 12.0;
    let col_w = (pw - col_gap) * 0.5;
    let st = &ctx.app_state;
    let pos_lbl = if st.island_position == "bottom" {
        "BAWAH".to_string()
    } else {
        "ATAS".to_string()
    };

    let row_h = 31.0;
    let row_stride = 35.0;
    let btn_h = 23.0;
    let btn_y_off = 4.0;

    let left_rows: [(&str, String, UiAction); 5] = [
        ("Gaya Island", st.island_style.to_uppercase(), UiAction::SettingCycleIslandStyle),
        ("Posisi", pos_lbl, UiAction::SettingTogglePosition),
        ("Animasi", st.anim_preset.to_uppercase(), UiAction::SettingCycleAnimPreset),
        ("Hover Expand", if st.hover_expand { "AKTIF".into() } else { "KLIK".into() }, UiAction::SettingToggleHoverExpand),
        ("Notifikasi", if st.notif_enabled { "AKTIF".into() } else { "MATI".into() }, UiAction::SettingToggleNotif),
    ];

    let mut ry = acc_y + acc_h + 6.0;
    for (label, val, act) in left_rows {
        fill_round_rect(pixmap, px, ry, col_w, row_h, 9.5, alpha_scale((22, 22, 26, 255), op));
        fonts.draw_text_centered_y(pixmap, px + 11.0, ry + row_h * 0.5, label, 11.6, false, alpha_scale((228, 228, 231, 240), op), None);
        let bw = (fonts.measure_width(&val, 10.8, true) + 18.0).max(48.0);
        draw_pill_btn_sized(pixmap, fonts, ctx, hitmap, px + col_w - bw - 6.0, ry + btn_y_off, btn_h, 9.0, 10.8, &val, false, op, act);
        ry += row_stride;
    }

    let rx = px + col_w + col_gap;
    let mut rry = acc_y + acc_h + 6.0;

    // Provider Lirik row: `<` [PROVIDER] `>` so user can cycle all 12 providers forward or backward
    fill_round_rect(pixmap, rx, rry, col_w, row_h, 9.5, alpha_scale((22, 22, 26, 255), op));
    fonts.draw_text_centered_y(pixmap, rx + 11.0, rry + row_h * 0.5, "Provider Lirik", 11.4, false, alpha_scale((228, 228, 231, 240), op), None);
    let prov_lbl = lyric_provider_label(&st.lyric_provider);
    let w_next = (fonts.measure_width(">", 10.8, true) + 12.0).max(28.0);
    let w_prev = (fonts.measure_width("<", 10.8, true) + 12.0).max(28.0);
    let prov_tr = fonts.truncate_to_width(prov_lbl, 10.0, true, 96.0);
    let w_prov = (fonts.measure_width(&prov_tr, 10.0, true) + 14.0).max(28.0);
    let next_x = rx + col_w - w_next - 6.0;
    let prov_x = next_x - w_prov - 4.0;
    let prev_x = prov_x - w_prev - 4.0;
    draw_pill_btn_sized(pixmap, fonts, ctx, hitmap, prev_x, rry + btn_y_off, btn_h, 6.0, 10.8, "<", false, op, UiAction::SettingCycleLyricProvider(-1));
    draw_pill_btn_sized(pixmap, fonts, ctx, hitmap, prov_x, rry + btn_y_off, btn_h, 7.0, 10.0, &prov_tr, false, op, UiAction::SettingCycleLyricProvider(1));
    draw_pill_btn_sized(pixmap, fonts, ctx, hitmap, next_x, rry + btn_y_off, btn_h, 6.0, 10.8, ">", false, op, UiAction::SettingCycleLyricProvider(1));
    rry += row_stride;

    // Gaya Lirik row
    let style_val = st.lyric_style.replace('_', " ").to_uppercase();
    fill_round_rect(pixmap, rx, rry, col_w, row_h, 9.5, alpha_scale((22, 22, 26, 255), op));
    fonts.draw_text_centered_y(pixmap, rx + 11.0, rry + row_h * 0.5, "Gaya Lirik", 11.6, false, alpha_scale((228, 228, 231, 240), op), None);
    let bw = (fonts.measure_width(&style_val, 10.4, true) + 16.0).max(52.0);
    draw_pill_btn_sized(pixmap, fonts, ctx, hitmap, rx + col_w - bw - 6.0, rry + btn_y_off, btn_h, 8.0, 10.4, &style_val, false, op, UiAction::SettingCycleLyricStyle);
    rry += row_stride;

    fill_round_rect(pixmap, rx, rry, col_w, row_h, 9.5, alpha_scale((22, 22, 26, 255), op));
    fonts.draw_text_centered_y(pixmap, rx + 11.0, rry + row_h * 0.5, &format!("Lebar ({}px)", st.compact_width), 11.6, false, alpha_scale((228, 228, 231, 240), op), None);
    let w_plus = draw_pill_btn_sized(pixmap, fonts, ctx, hitmap, rx + col_w - 36.0, rry + btn_y_off, btn_h, 8.0, 11.8, "+", false, op, UiAction::SettingCompactWidth(20));
    draw_pill_btn_sized(pixmap, fonts, ctx, hitmap, rx + col_w - 36.0 - w_plus - 4.0, rry + btn_y_off, btn_h, 8.0, 11.8, "-", false, op, UiAction::SettingCompactWidth(-20));
    rry += row_stride;

    fill_round_rect(pixmap, rx, rry, col_w, row_h, 9.5, alpha_scale((22, 22, 26, 255), op));
    fonts.draw_text_centered_y(pixmap, rx + 11.0, rry + row_h * 0.5, &format!("Jarak Y ({}px)", st.y_offset), 11.6, false, alpha_scale((228, 228, 231, 240), op), None);
    let w_yp = draw_pill_btn_sized(pixmap, fonts, ctx, hitmap, rx + col_w - 36.0, rry + btn_y_off, btn_h, 8.0, 11.8, "+", false, op, UiAction::SettingYOffset(4));
    draw_pill_btn_sized(pixmap, fonts, ctx, hitmap, rx + col_w - 36.0 - w_yp - 4.0, rry + btn_y_off, btn_h, 8.0, 11.8, "-", false, op, UiAction::SettingYOffset(-4));
    rry += row_stride;

    fill_round_rect(pixmap, rx, rry, col_w, row_h, 9.5, alpha_scale((22, 22, 26, 255), op));
    fonts.draw_text_centered_y(pixmap, rx + 11.0, rry + row_h * 0.5, &format!("Offset ({}ms)", st.lyric_offset_ms), 11.6, false, alpha_scale((228, 228, 231, 240), op), None);
    let w_p2 = draw_pill_btn_sized(pixmap, fonts, ctx, hitmap, rx + col_w - 36.0, rry + btn_y_off, btn_h, 8.0, 11.8, "+", false, op, UiAction::SettingLyricOffset(100));
    draw_pill_btn_sized(pixmap, fonts, ctx, hitmap, rx + col_w - 36.0 - w_p2 - 4.0, rry + btn_y_off, btn_h, 8.0, 11.8, "-", false, op, UiAction::SettingLyricOffset(-100));
}
