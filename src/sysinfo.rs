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
                    let _ = Command::new("wpctl")
                        .args(["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"])
                        .status();
                }

                // Drain any newer volume value that arrived while mute was toggling
                let newer = worker.pending_vol.swap(-1, Ordering::AcqRel);
                if newer >= 0 {
                    v = newer;
                }

                if v >= 0 && v != last_applied_vol {
                    last_applied_vol = v;
                    let arg = format!("{}%", v.clamp(0, 100));
                    let _ = Command::new("wpctl")
                        .args(["set-volume", "@DEFAULT_AUDIO_SINK@", &arg])
                        .status();
                }
            }
        });

        // 2. Fast Dedicated Volume Monitor Thread (45ms poll, independent of slow MPRIS/D-Bus)
        thread::spawn(|| loop {
            let since_user_ms =
                now_epoch_ms().saturating_sub(LAST_USER_VOL_SET_MS.load(Ordering::Acquire));
            if since_user_ms >= 220 {
                if let Some((pct, muted)) = query_wpctl_volume() {
                    let since_after =
                        now_epoch_ms().saturating_sub(LAST_USER_VOL_SET_MS.load(Ordering::Acquire));
                    if since_after >= 220 {
                        LIVE_HW_VOL.store(pct, Ordering::Release);
                        LIVE_HW_MUTED.store(muted, Ordering::Release);
                        LIVE_HW_INITIALIZED.store(true, Ordering::Release);
                    }
                }
            }
            thread::sleep(Duration::from_millis(45));
        });

        sig
    })
}

fn query_wpctl_volume() -> Option<(u32, bool)> {
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
        let (idle, total) = Self::read_proc_stat().unwrap_or((0, 0));
        Self {
            prev_idle: idle,
            prev_total: total,
            last_sample: Instant::now(),
        }
    }

    fn read_proc_stat() -> Option<(u64, u64)> {
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

    pub fn sample_metrics(&mut self) -> SystemMetrics {
        let mut cpu_usage = 0.0;
        if let Some((idle, total)) = Self::read_proc_stat() {
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
}
