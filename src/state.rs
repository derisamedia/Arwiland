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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppState {
    pub todos: Vec<TodoItem>,
    pub shelf_files: Vec<ShelfFile>,
    pub water_glasses: u32,
    pub water_goal: u32,
    pub last_date: String,
    pub selected_city: String,
    pub pinned: bool,
    pub auto_collapse: bool,
    pub y_offset: i32,
    pub pomo_completed_sessions: u32,
    #[serde(default = "default_island_style")]
    pub island_style: String,
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
            pinned: false,
            auto_collapse: true,
            y_offset: 0,
            pomo_completed_sessions: 0,
            island_style: "notch".to_string(),
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
        }
    }
}

impl AppState {
    pub fn config_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let dir = PathBuf::from(home).join(".config").join("arwiland");
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
                if state.max_lyric_width <= 620 {
                    state.max_lyric_width = 780;
                }
                if state.notif_duration_secs < 2 || state.notif_duration_secs > 15 {
                    state.notif_duration_secs = 5;
                }
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
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("autostart")
        .join("arwiland.desktop")
}

pub fn is_autostart_installed() -> bool {
    autostart_desktop_path().exists()
}

pub fn resolve_startup_exec_command() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let local_bin = PathBuf::from(home)
        .join(".local")
        .join("bin")
        .join("arwiland");
    if local_bin.exists() {
        return format!(
            "env GDK_BACKEND=x11 GSK_RENDERER=cairo \"{}\"",
            local_bin.to_string_lossy()
        );
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(project_root) = exe
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
        {
            let run_script = project_root.join("run.sh");
            if run_script.exists() {
                return format!("\"{}\"", run_script.to_string_lossy());
            }
        }
        return format!(
            "env GDK_BACKEND=x11 GSK_RENDERER=cairo \"{}\"",
            exe.to_string_lossy()
        );
    }
    "arwiland".to_string()
}

pub fn set_autostart_enabled(enabled: bool) -> bool {
    let desktop_path = autostart_desktop_path();
    if enabled {
        if let Some(parent) = desktop_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let exec_cmd = resolve_startup_exec_command();
        let desktop_content = format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Version=1.0\n\
             Name=Arwiland\n\
             GenericName=Dynamic Island & Living Notch\n\
             Comment=Dynamic Island & Living Notch untuk Linux\n\
             Exec={}\n\
             Icon=arwiland\n\
             Terminal=false\n\
             Categories=Utility;GTK;\n\
             StartupNotify=false\n\
             X-GNOME-Autostart-enabled=true\n\
             X-GNOME-Autostart-Delay=2\n",
            exec_cmd
        );
        fs::write(&desktop_path, desktop_content).is_ok()
    } else if desktop_path.exists() {
        fs::remove_file(&desktop_path).is_ok()
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
