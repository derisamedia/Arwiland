use chrono::{Datelike, Local, Timelike};

#[derive(Clone, Debug)]
pub struct CityCoord {
    pub name: &'static str,
    pub lat: f64,
    pub lon: f64,
    pub tz_offset: f64,
    pub tz_label: &'static str,
}

pub const INDONESIAN_CITIES: &[CityCoord] = &[
    CityCoord {
        name: "Jakarta",
        lat: -6.2088,
        lon: 106.8456,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Bandung",
        lat: -6.9175,
        lon: 107.6191,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Surabaya",
        lat: -7.2575,
        lon: 112.7521,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Yogyakarta",
        lat: -7.7956,
        lon: 110.3695,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Semarang",
        lat: -6.9667,
        lon: 110.4167,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Medan",
        lat: 3.5952,
        lon: 98.6722,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Palembang",
        lat: -2.9761,
        lon: 104.7754,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Makassar",
        lat: -5.1477,
        lon: 119.4327,
        tz_offset: 8.0,
        tz_label: "WITA",
    },
    CityCoord {
        name: "Kendari",
        lat: -3.9985,
        lon: 122.5130,
        tz_offset: 8.0,
        tz_label: "WITA",
    },
    CityCoord {
        name: "Denpasar",
        lat: -8.6705,
        lon: 115.2126,
        tz_offset: 8.0,
        tz_label: "WITA",
    },
    CityCoord {
        name: "Balikpapan",
        lat: -1.2379,
        lon: 116.8529,
        tz_offset: 8.0,
        tz_label: "WITA",
    },
    CityCoord {
        name: "Jayapura",
        lat: -2.5916,
        lon: 140.6690,
        tz_offset: 9.0,
        tz_label: "WIT",
    },
];

#[derive(Clone, Debug)]
pub struct PrayerSchedule {
    pub city: String,
    pub tz_label: String,
    pub prayers: Vec<(String, String, u32)>, // (Name, HH:MM, minutes_from_midnight)
    pub next_prayer_name: String,
    pub next_prayer_time: String,
    pub countdown_str: String,
}

fn deg2rad(d: f64) -> f64 {
    d * std::f64::consts::PI / 180.0
}

fn rad2deg(r: f64) -> f64 {
    r * 180.0 / std::f64::consts::PI
}

fn hour_angle(lat: f64, decl: f64, altitude_deg: f64) -> f64 {
    let lat_r = deg2rad(lat);
    let decl_r = deg2rad(decl);
    let alt_r = deg2rad(altitude_deg);
    let cos_ha = (alt_r.sin() - lat_r.sin() * decl_r.sin()) / (lat_r.cos() * decl_r.cos());
    let clamped = cos_ha.clamp(-1.0, 1.0);
    rad2deg(clamped.acos()) / 15.0
}

fn format_hours_to_hm(hours: f64) -> (String, u32) {
    let mut total_mins = (hours * 60.0).round() as i32;
    total_mins = total_mins.rem_euclid(24 * 60);
    let h = (total_mins / 60) as u32;
    let m = (total_mins % 60) as u32;
    (format!("{:02}:{:02}", h, m), (h * 60 + m))
}

pub fn calculate_prayer_schedule(city_name: &str) -> PrayerSchedule {
    let city = INDONESIAN_CITIES
        .iter()
        .find(|c| c.name.eq_ignore_ascii_case(city_name))
        .unwrap_or(&INDONESIAN_CITIES[0]);

    let now = Local::now();
    let day_of_year = now.ordinal() as f64;

    // Solar approximation (accurate within ~1-2 minutes for Kemenag RI standard)
    let gamma = 2.0 * std::f64::consts::PI / 365.0 * (day_of_year - 1.0);
    let eq_time_min = 229.18
        * (0.000075 + 0.001868 * gamma.cos()
            - 0.032077 * gamma.sin()
            - 0.014615 * (2.0 * gamma).cos()
            - 0.040849 * (2.0 * gamma).sin());
    let decl_rad = 0.006918 - 0.399912 * gamma.cos() + 0.070257 * gamma.sin()
        - 0.006758 * (2.0 * gamma).cos()
        + 0.000907 * (2.0 * gamma).sin()
        - 0.002697 * (3.0 * gamma).cos()
        + 0.00148 * (3.0 * gamma).sin();
    let decl_deg = rad2deg(decl_rad);

    // Solar noon + 2 minute ihtiyath (Kemenag standard safety margin)
    let ihtiyath_hours = 2.0 / 60.0;
    let noon = 12.0 + city.tz_offset - (city.lon / 15.0) - (eq_time_min / 60.0);

    let dzuhur_h = noon + ihtiyath_hours;
    let fajr_ha = hour_angle(city.lat, decl_deg, -20.0);
    let subuh_h = noon - fajr_ha + ihtiyath_hours;
    let imsak_h = subuh_h - (10.0 / 60.0);

    // Ashar (Shafi'i shadow factor = 1 + tan(|lat - decl|))
    let shadow_angle_rad = (1.0 / (1.0 + deg2rad((city.lat - decl_deg).abs()).tan())).atan();
    let ashar_ha = hour_angle(city.lat, decl_deg, rad2deg(shadow_angle_rad));
    let ashar_h = noon + ashar_ha + ihtiyath_hours;

    // Maghrib (-0.833 deg sunset)
    let maghrib_ha = hour_angle(city.lat, decl_deg, -0.833);
    let maghrib_h = noon + maghrib_ha + ihtiyath_hours;

    // Isya (-18 deg)
    let isya_ha = hour_angle(city.lat, decl_deg, -18.0);
    let isya_h = noon + isya_ha + ihtiyath_hours;

    let raw_list = [
        ("Imsak", imsak_h),
        ("Subuh", subuh_h),
        ("Dzuhur", dzuhur_h),
        ("Ashar", ashar_h),
        ("Maghrib", maghrib_h),
        ("Isya", isya_h),
    ];

    let mut prayers = Vec::new();
    for (label, h) in raw_list {
        let (hm, mins) = format_hours_to_hm(h);
        prayers.push((label.to_string(), hm, mins));
    }

    let now_secs = now.hour() * 3600 + now.minute() * 60 + now.second();
    let mut next_idx = 0;
    let mut found_today = false;
    for (i, (_, _, mins)) in prayers.iter().enumerate() {
        if (*mins * 60) > now_secs {
            next_idx = i;
            found_today = true;
            break;
        }
    }

    let (next_name, next_time, next_mins) = &prayers[next_idx];
    let target_secs = if found_today {
        next_mins * 60
    } else {
        (next_mins + 24 * 60) * 60
    };
    let rem_secs = target_secs.saturating_sub(now_secs);
    let rh = rem_secs / 3600;
    let rm = (rem_secs % 3600) / 60;
    let rs = rem_secs % 60;

    PrayerSchedule {
        city: city.name.to_string(),
        tz_label: city.tz_label.to_string(),
        next_prayer_name: next_name.clone(),
        next_prayer_time: next_time.clone(),
        countdown_str: format!("-{:02}:{:02}:{:02}", rh, rm, rs),
        prayers,
    }
}
