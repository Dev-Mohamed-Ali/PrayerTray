//! String catalog runtime, port of I18n/Strings.cs. Missing key -> English -> key itself.

mod data;

use crate::datetime::Date;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    En = 0,
    Ar = 1,
    Fr = 2,
    Tr = 3,
    Ur = 4,
    Id = 5,
}

static CURRENT: AtomicUsize = AtomicUsize::new(0);

pub fn set(cfg: &str) {
    CURRENT.store(resolve(cfg) as usize, Ordering::Relaxed);
}

pub fn lang() -> Lang {
    match CURRENT.load(Ordering::Relaxed) {
        1 => Lang::Ar,
        2 => Lang::Fr,
        3 => Lang::Tr,
        4 => Lang::Ur,
        5 => Lang::Id,
        _ => Lang::En,
    }
}

pub fn is_rtl() -> bool {
    matches!(lang(), Lang::Ar | Lang::Ur)
}

fn resolve(cfg: &str) -> Lang {
    match cfg {
        "ar" => Lang::Ar,
        "en" => Lang::En,
        "fr" => Lang::Fr,
        "tr" => Lang::Tr,
        "ur" => Lang::Ur,
        "id" => Lang::Id,
        _ => match os_primary_lang() {
            0x01 => Lang::Ar,
            0x0C => Lang::Fr,
            0x1F => Lang::Tr,
            0x20 => Lang::Ur,
            0x21 => Lang::Id,
            _ => Lang::En,
        },
    }
}

#[cfg(windows)]
fn os_primary_lang() -> u32 {
    extern "system" {
        fn GetUserDefaultUILanguage() -> u16;
    }
    (unsafe { GetUserDefaultUILanguage() } as u32) & 0x3FF
}

#[cfg(not(windows))]
fn os_primary_lang() -> u32 {
    0x09 // en
}

fn lookup(table: &'static [(&'static str, [&'static str; data::LANGS])], key: &str) -> &'static str {
    match table.binary_search_by(|(k, _)| (*k).cmp(key)) {
        Ok(i) => {
            let vals = &table[i].1;
            let v = vals[lang() as usize];
            if v.is_empty() {
                vals[0]
            } else {
                v
            }
        }
        // Missing key: leak so the signature stays &'static — happens only on a coding error.
        Err(_) => Box::leak(key.to_owned().into_boxed_str()),
    }
}

pub fn t(key: &str) -> &'static str {
    lookup(&data::UI, key)
}

/// Composite-format lookup: t(key) used as a {0}/{1} template (C# String.Format semantics
/// for plain positional placeholders, which is all the catalog uses).
pub fn f(key: &str, args: &[&str]) -> String {
    let mut out = String::new();
    let tmpl = t(key);
    let mut chars = tmpl.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            if chars.peek() == Some(&'{') {
                chars.next();
                out.push('{');
                continue;
            }
            let mut idx = String::new();
            for d in chars.by_ref() {
                if d == '}' {
                    break;
                }
                idx.push(d);
            }
            if let Ok(i) = idx.parse::<usize>() {
                if let Some(a) = args.get(i) {
                    out.push_str(a);
                }
            }
        } else if c == '}' && chars.peek() == Some(&'}') {
            chars.next();
            out.push('}');
        } else {
            out.push(c);
        }
    }
    out
}

pub fn prayer(key: &str) -> &'static str {
    lookup(&data::PRAYERS, key)
}

pub fn event(key: &str) -> &'static str {
    lookup(&data::EVENTS, key)
}

pub fn am_pm(hour: u32) -> &'static str {
    if hour < 12 {
        t("time.am")
    } else {
        t("time.pm")
    }
}

const EN_WEEKDAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const EN_MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// "Weekday, dd Month" with Western digits (C# invariant "dddd, dd MMM" for languages
/// without their own tables).
pub fn format_popup_date(d: Date) -> String {
    let l = lang() as usize;
    let wd = d.weekday() as usize;
    match (&data::WEEKDAYS[l], &data::MONTHS[l]) {
        (Some(w), Some(m)) => {
            let sep = if is_rtl() { "، " } else { ", " };
            format!("{}{}{:02} {}", w[wd], sep, d.day, m[d.month as usize - 1])
        }
        _ => format!("{}, {:02} {}", EN_WEEKDAYS[wd], d.day, EN_MONTHS[d.month as usize - 1]),
    }
}

/// "7 Ramadan 1448" — Umm al-Qura, Western digits, ±adjust days, localized month name.
pub fn format_hijri(d: Date, adjust: i32) -> String {
    let (y, m, day) = crate::calc::hijri::convert(d, adjust);
    format!("{} {} {}", day, data::HIJRI_MONTHS[lang() as usize][m as usize - 1], y)
}
