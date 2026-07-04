//! Prayer-times computation, port of Calc/PrayerTimes.cs (PrayTimes.org algorithm).
//! Times are minutes since local midnight; NaN (polar) collapses to 0 like the C# TimeSpan.Zero.

use crate::datetime::Date;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AsrJuristic {
    Standard = 1,
    Hanafi = 2,
}

impl AsrJuristic {
    pub fn from_int(v: i32) -> Self {
        if v == 2 { Self::Hanafi } else { Self::Standard }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HighLatRule {
    None = 0,
    MidNight = 1,
    OneSeventh = 2,
    AngleBased = 3,
}

impl HighLatRule {
    pub fn from_name(name: &str) -> Self {
        match name {
            "None" => Self::None,
            "MidNight" => Self::MidNight,
            "OneSeventh" => Self::OneSeventh,
            _ => Self::AngleBased,
        }
    }
}

pub struct CalcMethod {
    pub key: &'static str,
    pub name: &'static str,
    pub fajr_angle: f64,
    pub isha_angle: f64,
    pub isha_minutes: f64,
}

pub static METHODS: [CalcMethod; 5] = [
    CalcMethod { key: "MWL", name: "Muslim World League", fajr_angle: 18.0, isha_angle: 17.0, isha_minutes: 0.0 },
    CalcMethod { key: "ISNA", name: "Islamic Society of North America", fajr_angle: 15.0, isha_angle: 15.0, isha_minutes: 0.0 },
    CalcMethod { key: "Egypt", name: "Egyptian General Authority", fajr_angle: 19.5, isha_angle: 17.5, isha_minutes: 0.0 },
    CalcMethod { key: "Makkah", name: "Umm al-Qura, Makkah", fajr_angle: 18.5, isha_angle: 0.0, isha_minutes: 90.0 },
    CalcMethod { key: "Karachi", name: "Univ. of Islamic Sciences, Karachi", fajr_angle: 18.0, isha_angle: 18.0, isha_minutes: 0.0 },
];

pub fn method_by_key(key: &str) -> &'static CalcMethod {
    METHODS.iter().find(|m| m.key == key).unwrap_or(&METHODS[0])
}

pub const KEYS: [&str; 6] = ["fajr", "sunrise", "dhuhr", "asr", "maghrib", "isha"];

/// Per-prayer minute offsets, indexed like KEYS (sunrise slot unused).
#[derive(Clone, Copy, Default)]
pub struct Offsets {
    pub fajr: i32,
    pub dhuhr: i32,
    pub asr: i32,
    pub maghrib: i32,
    pub isha: i32,
}

impl Offsets {
    fn of(&self, key: &str) -> i32 {
        match key {
            "fajr" => self.fajr,
            "dhuhr" => self.dhuhr,
            "asr" => self.asr,
            "maghrib" => self.maghrib,
            "isha" => self.isha,
            _ => 0,
        }
    }
}

/// Minutes since midnight for each of KEYS, in KEYS order.
pub fn compute(
    date: Date,
    lat: f64,
    lng: f64,
    tz_hours: f64,
    method: &CalcMethod,
    asr: AsrJuristic,
    offsets: Option<&Offsets>,
    high_lat: HighLatRule,
) -> [u16; 6] {
    const RISE_SET_ANGLE: f64 = 0.833;

    let j_date = julian(date.year, date.month as i32, date.day as i32) - lng / (15.0 * 24.0);

    let (mut fajr, mut sunrise, mut dhuhr, mut asr_t, mut maghrib, mut isha) =
        (5.0f64, 6.0f64, 12.0f64, 13.0f64, 18.0f64, 18.0f64);

    for _ in 0..2 {
        fajr = sun_angle_time(j_date, lat, method.fajr_angle, fajr / 24.0, true);
        sunrise = sun_angle_time(j_date, lat, RISE_SET_ANGLE, sunrise / 24.0, true);
        dhuhr = mid_day(j_date, dhuhr / 24.0);
        asr_t = asr_time(j_date, lat, asr as i32 as f64, asr_t / 24.0);
        maghrib = sun_angle_time(j_date, lat, RISE_SET_ANGLE, maghrib / 24.0, false);
        isha = if method.isha_minutes > 0.0 {
            maghrib + method.isha_minutes / 60.0
        } else {
            sun_angle_time(j_date, lat, method.isha_angle, isha / 24.0, false)
        };
    }

    // High-latitude safety: near the poles the sun may never reach the fajr/isha angle (NaN).
    if high_lat != HighLatRule::None {
        let night = fix_hour(sunrise - maghrib);
        fajr = adjust_high_lat(fajr, sunrise, method.fajr_angle, night, high_lat, true);
        if method.isha_minutes <= 0.0 {
            isha = adjust_high_lat(isha, maghrib, method.isha_angle, night, high_lat, false);
        }
    }

    let adjust = tz_hours - lng / 15.0;
    let raw = [fajr, sunrise, dhuhr, asr_t, maghrib, isha];
    let mut out = [0u16; 6];
    for (i, key) in KEYS.iter().enumerate() {
        let mut val = raw[i] + adjust;
        if let Some(o) = offsets {
            val += o.of(key) as f64 / 60.0;
        }
        out[i] = to_minutes(val);
    }
    out
}

fn adjust_high_lat(time: f64, base: f64, angle: f64, night: f64, rule: HighLatRule, ccw: bool) -> f64 {
    let frac = match rule {
        HighLatRule::MidNight => 1.0 / 2.0,
        HighLatRule::OneSeventh => 1.0 / 7.0,
        _ => angle / 60.0, // AngleBased
    };
    let portion = frac * night;
    let diff = if ccw { fix_hour(base - time) } else { fix_hour(time - base) };
    if time.is_nan() || diff > portion {
        base + if ccw { -portion } else { portion }
    } else {
        time
    }
}

fn to_minutes(hours: f64) -> u16 {
    if hours.is_nan() {
        return 0;
    }
    let hours = fix_hour(hours);
    let h = hours.floor() as i32;
    // C# Math.Round is banker's rounding.
    let mut m = ((hours - h as f64) * 60.0).round_ties_even() as i32;
    let mut h = h;
    if m == 60 {
        m = 0;
        h = (h + 1) % 24;
    }
    (h * 60 + m) as u16
}

// --- astronomy ---

fn julian(year: i32, month: i32, day: i32) -> f64 {
    let (year, month) = if month <= 2 { (year - 1, month + 12) } else { (year, month) };
    let a = (year as f64 / 100.0).floor();
    let b = 2.0 - a + (a / 4.0).floor();
    (365.25 * (year as f64 + 4716.0)).floor() + (30.6001 * (month as f64 + 1.0)).floor()
        + day as f64 + b - 1524.5
}

fn sun_position(jd: f64) -> (f64, f64) {
    let d = jd - 2451545.0;
    let g = fix_angle(357.529 + 0.98560028 * d);
    let q = fix_angle(280.459 + 0.98564736 * d);
    let l = fix_angle(q + 1.915 * dsin(g) + 0.020 * dsin(2.0 * g));
    let e = 23.439 - 0.00000036 * d;
    let decl = darcsin(dsin(e) * dsin(l));
    let ra = fix_hour(darctan2(dcos(e) * dsin(l), dcos(l)) / 15.0);
    let eqt = q / 15.0 - ra;
    (decl, eqt)
}

fn mid_day(j_date: f64, t: f64) -> f64 {
    let (_, eqt) = sun_position(j_date + t);
    fix_hour(12.0 - eqt)
}

fn sun_angle_time(j_date: f64, lat: f64, angle: f64, t: f64, ccw: bool) -> f64 {
    let (decl, _) = sun_position(j_date + t);
    let noon = mid_day(j_date, t);
    let inner = (-dsin(angle) - dsin(decl) * dsin(lat)) / (dcos(decl) * dcos(lat));
    let v = (1.0 / 15.0) * darccos(inner);
    noon + if ccw { -v } else { v }
}

fn asr_time(j_date: f64, lat: f64, factor: f64, t: f64) -> f64 {
    let (decl, _) = sun_position(j_date + t);
    let angle = -darccot(factor + dtan((lat - decl).abs()));
    sun_angle_time(j_date, lat, angle, t, false)
}

// --- degree-based trig helpers ---

fn dtr(d: f64) -> f64 {
    d * std::f64::consts::PI / 180.0
}
fn rtd(r: f64) -> f64 {
    r * 180.0 / std::f64::consts::PI
}
fn dsin(d: f64) -> f64 {
    dtr(d).sin()
}
fn dcos(d: f64) -> f64 {
    dtr(d).cos()
}
fn dtan(d: f64) -> f64 {
    dtr(d).tan()
}
fn darcsin(x: f64) -> f64 {
    rtd(x.asin())
}
fn darccos(x: f64) -> f64 {
    rtd(x.acos())
}
fn darccot(x: f64) -> f64 {
    rtd((1.0 / x).atan())
}
fn darctan2(y: f64, x: f64) -> f64 {
    rtd(y.atan2(x))
}
fn fix_angle(a: f64) -> f64 {
    fix(a, 360.0)
}
fn fix_hour(a: f64) -> f64 {
    fix(a, 24.0)
}
fn fix(a: f64, b: f64) -> f64 {
    let a = a - b * (a / b).floor();
    if a < 0.0 {
        a + b
    } else {
        a
    }
}
