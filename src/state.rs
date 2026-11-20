use crate::notifications::IslandNotification;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: u64,
    pub text: String,
    pub completed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShelfFile {
    pub path: String,
    pub name: String,
    pub size_label: String,
}

fn default_island_style() -> String {
    "notch".to_string()
}

fn default_island_position() -> String {
    "top".to_string()
}

fn default_anim_preset() -> String {
    "smooth".to_string()
}

fn default_compact_width() -> i32 {
    300
}

fn default_true() -> bool {
    true
}

fn default_lyric_provider() -> String {
    "auto".to_string()
}

fn default_lyric_style() -> String {
    "word_bounce".to_string()
}

fn default_max_lyric_width() -> i32 {
    780
}

fn default_notif_duration() -> i32 {
    5
}

fn default_pomo_focus_mins() -> u32 {
    25
}

fn default_pomo_break_mins() -> u32 {
    5
}

fn default_alarm_hour() -> u32 {
    7
}

fn default_alarm_min() -> u32 {
    0
}

fn default_accent_color() -> String {
    "emerald".to_string()
}

#[derive(Clone, Copy, Debug)]
pub struct AccentPalette {
    pub id: &'static str,
    pub label: &'static str,
    /// Bright accent for headings, icons, highlights, active bars
    pub light: (u8, u8, u8),
    /// Deeper saturated accent for tinted pills, progress fills, gradients
    pub solid: (u8, u8, u8),
    /// Extra bright highlight for small badge numbers
    pub glow: (u8, u8, u8),
}

impl AccentPalette {
    pub fn rgb_f64(&self) -> (f64, f64, f64) {
        (
            self.light.0 as f64 / 255.0,
            self.light.1 as f64 / 255.0,
            self.light.2 as f64 / 255.0,
        )
    }
}

pub const ACCENT_PRESETS: &[AccentPalette] = &[
    AccentPalette {
        id: "emerald",
        label: "HIJAU",
        light: (52, 211, 153),
        solid: (16, 185, 129),
        glow: (64, 235, 173),
    },
    AccentPalette {
        id: "sky",
        label: "BIRU",
        light: (56, 189, 248),
        solid: (14, 165, 233),
        glow: (103, 212, 255),
    },
    AccentPalette {
        id: "indigo",
        label: "INDIGO",
        light: (129, 140, 248),
        solid: (99, 102, 241),
        glow: (165, 175, 255),
    },
    AccentPalette {
        id: "violet",
        label: "UNGU",
        light: (167, 139, 250),
        solid: (139, 92, 246),
        glow: (196, 173, 255),
    },
    AccentPalette {
        id: "pink",
        label: "PINK",
        light: (244, 114, 182),
        solid: (236, 72, 153),
        glow: (250, 156, 206),
    },
    AccentPalette {
        id: "rose",
        label: "MERAH",
        light: (251, 113, 133),
        solid: (244, 63, 94),
        glow: (255, 148, 165),
    },
    AccentPalette {
        id: "amber",
        label: "ORANYE",
        light: (251, 191, 36),
        solid: (245, 158, 11),
        glow: (253, 214, 96),
    },
    AccentPalette {
        id: "teal",
        label: "CYAN",
        light: (45, 212, 191),
        solid: (20, 184, 166),
        glow: (94, 234, 212),
    },
    AccentPalette {
        id: "mono",
        label: "PUTIH",
        light: (244, 244, 245),
        solid: (212, 212, 216),
        glow: (255, 255, 255),
    },
];

pub fn resolve_accent_palette(id: &str) -> &'static AccentPalette {
    ACCENT_PRESETS
        .iter()
        .find(|p| p.id.eq_ignore_ascii_case(id))
        .unwrap_or(&ACCENT_PRESETS[0])
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppState {
    pub todos: Vec<TodoItem>,
    pub shelf_files: Vec<ShelfFile>,
    pub water_glasses: u32,
    pub water_goal: u32,
    pub last_date: String,
    pub selected_city: String,
    #[serde(default)]
    pub prayer_lat: Option<f64>,
    #[serde(default)]
    pub prayer_lon: Option<f64>,
    #[serde(default)]
    pub prayer_tz_offset: Option<f64>,
    #[serde(default)]
    pub prayer_tz_label: Option<String>,
    pub pinned: bool,
    pub auto_collapse: bool,
    pub y_offset: i32,
    pub pomo_completed_sessions: u32,
    #[serde(default = "default_pomo_focus_mins")]
    pub pomo_focus_mins: u32,
    #[serde(default = "default_pomo_break_mins")]
    pub pomo_break_mins: u32,
    #[serde(default = "default_alarm_hour")]
    pub alarm_hour: u32,
    #[serde(default = "default_alarm_min")]
    pub alarm_min: u32,
    #[serde(default)]
    pub alarm_enabled: bool,
    #[serde(default = "default_island_style")]
    pub island_style: String,
    #[serde(default = "default_island_position")]
    pub island_position: String,
    #[serde(default = "default_anim_preset")]
    pub anim_preset: String,
    #[serde(default = "default_compact_width")]
    pub compact_width: i32,
    #[serde(default = "default_true")]
    pub hover_expand: bool,
    #[serde(default = "default_lyric_provider")]
    pub lyric_provider: String,
    #[serde(default = "default_lyric_style")]
    pub lyric_style: String,
    #[serde(default)]
    pub lyric_offset_ms: i32,
    #[serde(default = "default_max_lyric_width")]
    pub max_lyric_width: i32,
    #[serde(default)]
    pub notifications: Vec<IslandNotification>,
    #[serde(default = "default_true")]
    pub notif_enabled: bool,
    #[serde(default = "default_true")]
    pub suppress_system_banners: bool,
    #[serde(default = "default_true")]
    pub system_event_notifs: bool,
    #[serde(default = "default_notif_duration")]
    pub notif_duration_secs: i32,
    #[serde(default)]
    pub run_on_startup: bool,
    #[serde(default = "default_accent_color")]
    pub accent_color: String,
}

impl Default for AppState {
    fn default() -> Self {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        Self {
            todos: vec![
                TodoItem {
                    id: 1,
                    text: "Fokus kerja 2 sesi Pomodoro".to_string(),
                    completed: false,
                },
                TodoItem {
                    id: 2,
                    text: "Minum 8 gelas air putih hari ini".to_string(),
                    completed: false,
                },
                TodoItem {
                    id: 3,
                    text: "Coba fitur Notifikasi & Media di Arwiland".to_string(),
                    completed: true,
                },
            ],
            shelf_files: Vec::new(),
            water_glasses: 2,
            water_goal: 8,
            last_date: today,
            selected_city: "Jakarta".to_string(),
            prayer_lat: None,
            prayer_lon: None,
            prayer_tz_offset: None,
            prayer_tz_label: None,
            pinned: false,
            auto_collapse: true,
            y_offset: 0,
            pomo_completed_sessions: 0,
            pomo_focus_mins: 25,
            pomo_break_mins: 5,
            alarm_hour: 7,
            alarm_min: 0,
            alarm_enabled: false,
            island_style: "notch".to_string(),
            island_position: "top".to_string(),
            anim_preset: "smooth".to_string(),
            compact_width: 300,
            hover_expand: true,
            lyric_provider: "auto".to_string(),
            lyric_style: "word_bounce".to_string(),
            lyric_offset_ms: 0,
            max_lyric_width: 780,
            notifications: Vec::new(),
            notif_enabled: true,
            suppress_system_banners: true,
            system_event_notifs: true,
            notif_duration_secs: 5,
            run_on_startup: is_autostart_installed(),
            accent_color: default_accent_color(),
        }
    }
}

impl AppState {
    pub fn config_path() -> PathBuf {
        #[cfg(target_os = "windows")]
        let base_dir = std::env::var("APPDATA")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        #[cfg(not(target_os = "windows"))]
        let base_dir = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());

        #[cfg(target_os = "windows")]
        let dir = PathBuf::from(base_dir).join("arwiland");
        #[cfg(not(target_os = "windows"))]
        let dir = PathBuf::from(base_dir).join(".config").join("arwiland");

        let _ = fs::create_dir_all(&dir);
        dir.join("state.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        if let Ok(data) = fs::read_to_string(&path) {
            if let Ok(mut state) = serde_json::from_str::<AppState>(&data) {
                if state.last_date != today {
                    state.water_glasses = 0;
                    state.last_date = today;
                }
                if state.island_style == "notch" && state.y_offset == 6 {
                    state.y_offset = 0;
                }
                if state.island_position != "top" && state.island_position != "bottom" {
                    state.island_position = "top".to_string();
                }
                state.y_offset = state.y_offset.clamp(0, 64);
                if state.max_lyric_width <= 620 {
                    state.max_lyric_width = 780;
                }
                if state.notif_duration_secs < 2 || state.notif_duration_secs > 15 {
                    state.notif_duration_secs = 5;
                }
                state.pomo_focus_mins = state.pomo_focus_mins.clamp(1, 180);
                state.pomo_break_mins = state.pomo_break_mins.clamp(1, 60);
                state.alarm_hour = state.alarm_hour.min(23);
                state.alarm_min = state.alarm_min.min(59);
                if is_autostart_installed() != state.run_on_startup {
                    let _ = set_autostart_enabled(state.run_on_startup);
                }
                return state;
            }
        }
        let state = Self::default();
        state.save();
        state
    }

    pub fn save(&self) {
        let path = Self::config_path();
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, json);
        }
    }
}

pub fn autostart_desktop_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(appdata)
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs")
            .join("Startup")
            .join("Arwiland.cmd")
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home)
            .join("Library")
            .join("LaunchAgents")
            .join("id.arwiland.DynamicIsland.plist")
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home)
            .join(".config")
            .join("autostart")
            .join("arwiland.desktop")
    }
}

pub fn is_autostart_installed() -> bool {
    autostart_desktop_path().exists()
}

pub fn resolve_startup_exec_command() -> String {
    #[cfg(target_os = "linux")]
    {
        let format_path = |p: &std::path::Path| -> String {
            let s = p.to_string_lossy();
            if s.contains(' ') {
                format!("env MALLOC_ARENA_MAX=2 \"{}\"", s)
            } else {
                format!("env MALLOC_ARENA_MAX=2 {}", s)
            }
        };
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let local_bin = PathBuf::from(home)
            .join(".local")
            .join("bin")
            .join("arwiland");
        if local_bin.exists() {
            return format_path(&local_bin);
        }
        if let Ok(exe) = std::env::current_exe() {
            return format_path(&exe);
        }
        "env MALLOC_ARENA_MAX=2 arwiland".to_string()
    }
    #[cfg(not(target_os = "linux"))]
    {
        std::env::current_exe()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "arwiland".to_string())
    }
}

pub fn set_autostart_enabled(enabled: bool) -> bool {
    let autostart_path = autostart_desktop_path();
    if enabled {
        if let Some(parent) = autostart_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let exec_cmd = resolve_startup_exec_command();

        #[cfg(target_os = "windows")]
        let content = format!("@echo off\r\nstart \"\" \"{}\"\r\n", exec_cmd);

        #[cfg(target_os = "macos")]
        let content = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
             <plist version=\"1.0\">\n\
             <dict>\n\
                 <key>Label</key>\n\
                 <string>id.arwiland.DynamicIsland</string>\n\
                 <key>ProgramArguments</key>\n\
                 <array>\n\
                     <string>{}</string>\n\
                 </array>\n\
                 <key>RunAtLoad</key>\n\
                 <true/>\n\
             </dict>\n\
             </plist>\n",
            exec_cmd
        );

        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        let content = format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Version=1.0\n\
             Name=Arwiland\n\
             GenericName=Dynamic Island & Living Notch\n\
             Comment=Dynamic Island & Living Notch\n\
             Exec={}\n\
             Icon=arwiland\n\
             Terminal=false\n\
             Categories=Utility;GTK;\n\
             StartupNotify=false\n\
             X-GNOME-Autostart-enabled=true\n\
             X-GNOME-Autostart-Delay=3\n",
            exec_cmd
        );

        fs::write(&autostart_path, content).is_ok()
    } else if autostart_path.exists() {
        fs::remove_file(&autostart_path).is_ok()
    } else {
        true
    }
}

pub fn format_file_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.1} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{} B", bytes)
    }
}
