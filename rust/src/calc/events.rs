//! Islamic special days from the Hijri date, port of Calc/IslamicEvents.cs.
//! Returns stable keys; display names come from the string catalog.

use super::hijri;
use crate::datetime::Date;

const MAJOR: [&str; 7] = ["newYear", "ashura", "mawlid", "ramadanStart", "eidFitr", "arafah", "eidAdha"];

/// Today's event key (a fixed day wins over the monthly white days), or None.
pub fn for_date(date: Date, adjust: i32) -> Option<&'static str> {
    let (_, m, d) = hijri::convert(date, adjust);
    fixed(m, d).or(if matches!(d, 13..=15) { Some("whiteDays") } else { None })
}

/// The next major event and how many days away (scans ~13 months forward), or None.
pub fn next_major(date: Date, adjust: i32) -> Option<(&'static str, i32)> {
    for i in 1..=400 {
        let (_, m, d) = hijri::convert(date.add_days(i as i64), adjust);
        if let Some(k) = fixed(m, d) {
            if MAJOR.contains(&k) {
                return Some((k, i));
            }
        }
    }
    None
}

fn fixed(m: u32, d: u32) -> Option<&'static str> {
    match (m, d) {
        (1, 1) => Some("newYear"),
        (1, 10) => Some("ashura"),
        (3, 12) => Some("mawlid"),
        (7, 27) => Some("isra"),
        (8, 15) => Some("midShaban"),
        (9, 1) => Some("ramadanStart"),
        (9, 27) => Some("laylatQadr"),
        (10, 1) => Some("eidFitr"),
        (12, 9) => Some("arafah"),
        (12, 10) => Some("eidAdha"),
        (12, 11..=13) => Some("tashreeq"),
        _ => None,
    }
}
