#[cfg(target_os = "linux")]
use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static LIVE_HW_VOL: AtomicU32 = AtomicU32::new(75);
static LIVE_HW_MUTED: AtomicBool = AtomicBool::new(false);
static LIVE_HW_INITIALIZED: AtomicBool = AtomicBool::new(false);
static LAST_USER_VOL_SET_MS: AtomicU64 = AtomicU64::new(0);

struct VolWorkerSignal {
    pending_vol: AtomicI32, // -1 = none, 0..=100 = target %
    pending_mute_toggle: AtomicBool,
    lock: Mutex<()>,
    cvar: Condvar,
}

static VOL_WORKER: OnceLock<VolWorkerSignal> = OnceLock::new();

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn ensure_volume_threads() -> &'static VolWorkerSignal {
    VOL_WORKER.get_or_init(|| {
        let sig = VolWorkerSignal {
            pending_vol: AtomicI32::new(-1),
            pending_mute_toggle: AtomicBool::new(false),
            lock: Mutex::new(()),
            cvar: Condvar::new(),
        };

        // 1. Dedicated Coalescing Volume Setter Thread (zero UI thread blocking, zero process spam)
        thread::spawn(|| {
            let worker = VOL_WORKER.get().unwrap();
            let mut last_applied_vol: i32 = -1;
            loop {
                let mut v = worker.pending_vol.swap(-1, Ordering::AcqRel);
                let mute_tog = worker.pending_mute_toggle.swap(false, Ordering::AcqRel);

                if v < 0 && !mute_tog {
                    let guard = worker.lock.lock().unwrap();
                    drop(worker.cvar.wait(guard));
                    continue;
                }

                if mute_tog {
                    apply_platform_mute_toggle();
                }

                // Drain any newer volume value that arrived while mute was toggling
                let newer = worker.pending_vol.swap(-1, Ordering::AcqRel);
                if newer >= 0 {
                    v = newer;
                }

                if v >= 0 && v != last_applied_vol {
                    last_applied_vol = v;
                    apply_platform_volume(v.clamp(0, 100) as u32);
                }
            }
        });

        // 2. Fast Dedicated Volume Monitor Thread
        thread::spawn(|| {
            #[cfg(target_os = "linux")]
            let poll_ms = 45;
            #[cfg(not(target_os = "linux"))]
            let poll_ms = 500;

            loop {
                let since_user_ms =
                    now_epoch_ms().saturating_sub(LAST_USER_VOL_SET_MS.load(Ordering::Acquire));
                if since_user_ms >= 220 {
                    if let Some((pct, muted)) = query_platform_volume() {
                        let since_after = now_epoch_ms()
                            .saturating_sub(LAST_USER_VOL_SET_MS.load(Ordering::Acquire));
                        if since_after >= 220 {
                            LIVE_HW_VOL.store(pct, Ordering::Release);
                            LIVE_HW_MUTED.store(muted, Ordering::Release);
                            LIVE_HW_INITIALIZED.store(true, Ordering::Release);
                        }
                    }
                }
                thread::sleep(Duration::from_millis(poll_ms));
            }
        });

        sig
    })
}

#[cfg(target_os = "linux")]
fn apply_platform_mute_toggle() {
    let _ = Command::new("wpctl")
        .args(["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"])
        .status();
}

#[cfg(target_os = "macos")]
fn apply_platform_mute_toggle() {
    let muted = LIVE_HW_MUTED.load(Ordering::Acquire);
    let script = if muted {
        "set volume with output muted"
    } else {
        "set volume without output muted"
    };
    let _ = Command::new("osascript").args(["-e", script]).status();
}

#[cfg(target_os = "windows")]
fn apply_platform_mute_toggle() {
    #[link(name = "user32")]
    extern "system" {
        fn keybd_event(bVk: u8, bScan: u8, dwFlags: u32, dwExtraInfo: usize);
    }
    const VK_VOLUME_MUTE: u8 = 0xAD;
    const KEYEVENTF_KEYUP: u32 = 0x0002;
    unsafe {
        keybd_event(VK_VOLUME_MUTE, 0, 0, 0);
        keybd_event(VK_VOLUME_MUTE, 0, KEYEVENTF_KEYUP, 0);
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn apply_platform_mute_toggle() {}

#[cfg(target_os = "linux")]
fn apply_platform_volume(vol: u32) {
    let arg = format!("{}%", vol);
    let _ = Command::new("wpctl")
        .args(["set-volume", "@DEFAULT_AUDIO_SINK@", &arg])
        .status();
}

#[cfg(target_os = "macos")]
fn apply_platform_volume(vol: u32) {
    let script = format!("set volume output volume {}", vol);
    let _ = Command::new("osascript").args(["-e", &script]).status();
}

#[cfg(target_os = "windows")]
fn apply_platform_volume(_vol: u32) {}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn apply_platform_volume(_vol: u32) {}

#[cfg(target_os = "linux")]
fn query_platform_volume() -> Option<(u32, bool)> {
    let out = Command::new("wpctl")
        .args(["get-volume", "@DEFAULT_AUDIO_SINK@"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let muted = text.contains("MUTED");
    for token in text.split_whitespace() {
        if let Ok(val) = token.parse::<f32>() {
            let pct = (val * 100.0).round() as u32;
            return Some((pct.min(150), muted));
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn query_platform_volume() -> Option<(u32, bool)> {
    let out = Command::new("osascript")
        .args([
            "-e",
            "set v to get volume settings\nreturn (output volume of v as string) & \" \" & (output muted of v as string)",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut parts = text.split_whitespace();
    let pct = parts.next()?.parse::<u32>().ok()?;
    let muted = parts.next().unwrap_or("false") == "true";
    Some((pct.min(100), muted))
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn query_platform_volume() -> Option<(u32, bool)> {
    Some((
        LIVE_HW_VOL.load(Ordering::Acquire),
        LIVE_HW_MUTED.load(Ordering::Acquire),
    ))
}

/// Instant, lock-free volume read for the 60FPS UI tick loop.
pub fn get_live_volume_fast() -> (u32, bool, bool) {
    let _ = ensure_volume_threads();
    (
        LIVE_HW_VOL.load(Ordering::Acquire),
        LIVE_HW_MUTED.load(Ordering::Acquire),
        LIVE_HW_INITIALIZED.load(Ordering::Acquire),
    )
}

/// Non-blocking coalesced volume setter (safe to call at 120Hz+ from UI drag handlers).
pub fn set_system_volume(percent: u32) {
    let worker = ensure_volume_threads();
    let clamped = percent.min(100);
    LAST_USER_VOL_SET_MS.store(now_epoch_ms(), Ordering::Release);
    LIVE_HW_VOL.store(clamped, Ordering::Release);
    LIVE_HW_MUTED.store(false, Ordering::Release);
    LIVE_HW_INITIALIZED.store(true, Ordering::Release);
    worker.pending_vol.store(clamped as i32, Ordering::Release);
    worker.cvar.notify_one();
}

/// Non-blocking mute toggle.
pub fn toggle_system_mute() {
    let worker = ensure_volume_threads();
    LAST_USER_VOL_SET_MS.store(now_epoch_ms(), Ordering::Release);
    let new_muted = !LIVE_HW_MUTED.load(Ordering::Acquire);
    LIVE_HW_MUTED.store(new_muted, Ordering::Release);
    worker.pending_mute_toggle.store(true, Ordering::Release);
    worker.cvar.notify_one();
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default)]
pub struct SystemMetrics {
    pub cpu_usage: f32,
    pub ram_usage: f32,
    pub ram_used_gb: f32,
    pub ram_total_gb: f32,
    pub battery_percent: Option<u32>,
    pub battery_charging: bool,
    pub uptime_str: String,
    pub volume_percent: u32,
    pub volume_muted: bool,
}

pub struct CpuTracker {
    prev_idle: u64,
    prev_total: u64,
    last_sample: Instant,
}

impl Default for CpuTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl CpuTracker {
    pub fn new() -> Self {
        let _ = ensure_volume_threads();
        let (idle, total) = Self::read_cpu_times().unwrap_or((0, 0));
        Self {
            prev_idle: idle,
            prev_total: total,
            last_sample: Instant::now(),
        }
    }

    pub fn sample_metrics(&mut self) -> SystemMetrics {
        let mut cpu_usage = 0.0;
        if let Some((idle, total)) = Self::read_cpu_times() {
            let diff_idle = idle.saturating_sub(self.prev_idle);
            let diff_total = total.saturating_sub(self.prev_total);
            if diff_total > 0 {
                cpu_usage =
                    ((diff_total.saturating_sub(diff_idle)) as f32 / diff_total as f32) * 100.0;
            }
            self.prev_idle = idle;
            self.prev_total = total;
            self.last_sample = Instant::now();
        }

        let (ram_usage, ram_used_gb, ram_total_gb) = Self::read_meminfo();
        let (battery_percent, battery_charging) = Self::read_battery();
        let uptime_str = Self::read_uptime();
        let (volume_percent, volume_muted, _) = get_live_volume_fast();

        SystemMetrics {
            cpu_usage: cpu_usage.clamp(0.0, 100.0),
            ram_usage,
            ram_used_gb,
            ram_total_gb,
            battery_percent,
            battery_charging,
            uptime_str,
            volume_percent,
            volume_muted,
        }
    }

    #[cfg(target_os = "linux")]
    fn read_cpu_times() -> Option<(u64, u64)> {
        let content = fs::read_to_string("/proc/stat").ok()?;
        let first_line = content.lines().next()?;
        let parts: Vec<u64> = first_line
            .split_whitespace()
            .skip(1)
            .filter_map(|s| s.parse::<u64>().ok())
            .collect();
        if parts.len() < 4 {
            return None;
        }
        let idle = parts[3] + parts.get(4).copied().unwrap_or(0);
        let total: u64 = parts.iter().sum();
        Some((idle, total))
    }

    #[cfg(target_os = "windows")]
    fn read_cpu_times() -> Option<(u64, u64)> {
        #[repr(C)]
        #[derive(Default, Copy, Clone)]
        struct FILETIME {
            dw_low_date_time: u32,
            dw_high_date_time: u32,
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn GetSystemTimes(
                lpIdleTime: *mut FILETIME,
                lpKernelTime: *mut FILETIME,
                lpUserTime: *mut FILETIME,
            ) -> i32;
        }
        let mut idle = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        let ok = unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) };
        if ok == 0 {
            return None;
        }
        let to_u64 =
            |ft: FILETIME| ((ft.dw_high_date_time as u64) << 32) | (ft.dw_low_date_time as u64);
        let idle_u = to_u64(idle);
        let total_u = to_u64(kernel) + to_u64(user);
        Some((idle_u, total_u))
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    fn read_cpu_times() -> Option<(u64, u64)> {
        None
    }

    #[cfg(target_os = "linux")]
    fn read_meminfo() -> (f32, f32, f32) {
        let content = match fs::read_to_string("/proc/meminfo") {
            Ok(c) => c,
            Err(_) => return (0.0, 0.0, 0.0),
        };
        let mut total_kb = 0u64;
        let mut avail_kb = 0u64;
        for line in content.lines() {
            if line.starts_with("MemTotal:") {
                total_kb = line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
            } else if line.starts_with("MemAvailable:") {
                avail_kb = line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
            }
        }
        if total_kb == 0 {
            return (0.0, 0.0, 0.0);
        }
        let used_kb = total_kb.saturating_sub(avail_kb);
        let pct = (used_kb as f32 / total_kb as f32) * 100.0;
        let used_gb = used_kb as f32 / (1024.0 * 1024.0);
        let total_gb = total_kb as f32 / (1024.0 * 1024.0);
        (pct, used_gb, total_gb)
    }

    #[cfg(target_os = "windows")]
    fn read_meminfo() -> (f32, f32, f32) {
        #[repr(C)]
        struct MEMORYSTATUSEX {
            dw_length: u32,
            dw_memory_load: u32,
            ull_total_phys: u64,
            ull_avail_phys: u64,
            ull_total_page_file: u64,
            ull_avail_page_file: u64,
            ull_total_virtual: u64,
            ull_avail_virtual: u64,
            ull_avail_extended_virtual: u64,
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn GlobalMemoryStatusEx(lpBuffer: *mut MEMORYSTATUSEX) -> i32;
        }
        unsafe {
            let mut mem: MEMORYSTATUSEX = std::mem::zeroed();
            mem.dw_length = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
            if GlobalMemoryStatusEx(&mut mem) != 0 && mem.ull_total_phys > 0 {
                let used = mem.ull_total_phys.saturating_sub(mem.ull_avail_phys);
                let gb = 1024.0 * 1024.0 * 1024.0;
                return (
                    mem.dw_memory_load as f32,
                    used as f32 / gb,
                    mem.ull_total_phys as f32 / gb,
                );
            }
        }
        (0.0, 0.0, 0.0)
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    fn read_meminfo() -> (f32, f32, f32) {
        (0.0, 0.0, 0.0)
    }

    #[cfg(target_os = "linux")]
    fn read_battery() -> (Option<u32>, bool) {
        let base = std::path::Path::new("/sys/class/power_supply");
        if let Ok(entries) = fs::read_dir(base) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("BAT") {
                    let cap_path = entry.path().join("capacity");
                    let status_path = entry.path().join("status");
                    let cap = fs::read_to_string(cap_path)
                        .ok()
                        .and_then(|s| s.trim().parse::<u32>().ok());
                    let status = fs::read_to_string(status_path)
                        .unwrap_or_default()
                        .trim()
                        .to_lowercase();
                    let charging = status == "charging" || status == "full";
                    if cap.is_some() {
                        return (cap, charging);
                    }
                }
            }
        }
        (None, false)
    }

    #[cfg(target_os = "windows")]
    fn read_battery() -> (Option<u32>, bool) {
        #[repr(C)]
        struct SYSTEM_POWER_STATUS {
            ac_line_status: u8,
            battery_flag: u8,
            battery_life_percent: u8,
            system_status_flag: u8,
            battery_life_time: u32,
            battery_full_life_time: u32,
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn GetSystemPowerStatus(lpSystemPowerStatus: *mut SYSTEM_POWER_STATUS) -> i32;
        }
        unsafe {
            let mut sps: SYSTEM_POWER_STATUS = std::mem::zeroed();
            if GetSystemPowerStatus(&mut sps) != 0
                && sps.battery_flag != 128
                && sps.battery_life_percent <= 100
            {
                let charging = sps.ac_line_status == 1 || (sps.battery_flag & 8) != 0;
                return (Some(sps.battery_life_percent as u32), charging);
            }
        }
        (None, false)
    }

    #[cfg(target_os = "macos")]
    fn read_battery() -> (Option<u32>, bool) {
        if let Ok(out) = Command::new("pmset").args(["-g", "batt"]).output() {
            let text = String::from_utf8_lossy(&out.stdout);
            for tok in text.split_whitespace() {
                if let Some(pct_str) = tok.strip_suffix("%;") {
                    if let Ok(pct) = pct_str.parse::<u32>() {
                        let charging = text.contains("AC Power") || text.contains("charging");
                        return (Some(pct.min(100)), charging);
                    }
                }
            }
        }
        (None, false)
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    fn read_battery() -> (Option<u32>, bool) {
        (None, false)
    }

    #[cfg(target_os = "linux")]
    fn read_uptime() -> String {
        if let Ok(content) = fs::read_to_string("/proc/uptime") {
            if let Some(first) = content.split_whitespace().next() {
                if let Ok(secs_f) = first.parse::<f64>() {
                    let secs = secs_f as u64;
                    let hours = secs / 3600;
                    let mins = (secs % 3600) / 60;
                    return format!("{}j {}m", hours, mins);
                }
            }
        }
        "0j 0m".to_string()
    }

    #[cfg(target_os = "windows")]
    fn read_uptime() -> String {
        #[link(name = "kernel32")]
        extern "system" {
            fn GetTickCount64() -> u64;
        }
        let secs = unsafe { GetTickCount64() } / 1000;
        let hours = secs / 3600;
        let mins = (secs % 3600) / 60;
        format!("{}j {}m", hours, mins)
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    fn read_uptime() -> String {
        "0j 0m".to_string()
    }
}
