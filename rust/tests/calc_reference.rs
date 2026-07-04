//! Cross-checks the Rust engine against fixtures dumped from the C# implementation
//! (see rust/tools — regenerate after any calc change on the C# side).

use prayertray::calc::praytimes::{self, AsrJuristic, HighLatRule, Offsets, KEYS};
use prayertray::calc::hijri;
use prayertray::datetime::Date;
use serde::Deserialize;
use std::collections::HashMap;

fn parse_date(s: &str) -> Date {
    let mut it = s.split('-').map(|p| p.parse::<i32>().unwrap());
    Date::new(it.next().unwrap(), it.next().unwrap() as u32, it.next().unwrap() as u32)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TimesCase {
    city: String,
    lat: f64,
    lng: f64,
    tz: f64,
    date: String,
    method: String,
    asr: i32,
    high_lat: i32,
    offsets: bool,
    times: HashMap<String, i32>,
}

#[test]
fn prayer_times_match_csharp() {
    let json = include_str!("data/reference_times.json");
    let cases: Vec<TimesCase> = serde_json::from_str(json).unwrap();
    assert!(cases.len() >= 1000);

    let offsets = Offsets { fajr: 2, dhuhr: -3, asr: 0, maghrib: 1, isha: -2 };
    let mut failures = Vec::new();
    for c in &cases {
        let rule = match c.high_lat {
            0 => HighLatRule::None,
            1 => HighLatRule::MidNight,
            2 => HighLatRule::OneSeventh,
            _ => HighLatRule::AngleBased,
        };
        let got = praytimes::compute(
            parse_date(&c.date),
            c.lat,
            c.lng,
            c.tz,
            praytimes::method_by_key(&c.method),
            AsrJuristic::from_int(c.asr),
            if c.offsets { Some(&offsets) } else { None },
            rule,
        );
        for (i, key) in KEYS.iter().enumerate() {
            let want = c.times[*key];
            if got[i] as i32 != want {
                failures.push(format!(
                    "{} {} {} asr{} hl{} {}: got {} want {}",
                    c.city, c.date, c.method, c.asr, c.high_lat, key, got[i], want
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} mismatches (of {} cases):\n{}",
        failures.len(),
        cases.len(),
        failures[..failures.len().min(25)].join("\n")
    );
}

#[derive(Deserialize)]
struct HijriCase {
    date: String,
    adjust: i32,
    hy: i32,
    hm: u32,
    hd: u32,
}

#[test]
fn hijri_matches_dotnet_umalqura() {
    let json = include_str!("data/hijri_fixture.json");
    let cases: Vec<HijriCase> = serde_json::from_str(json).unwrap();
    assert!(cases.len() >= 400);
    for c in &cases {
        let got = hijri::convert(parse_date(&c.date), c.adjust);
        assert_eq!(
            got,
            (c.hy, c.hm, c.hd),
            "hijri {} adjust {}: got {:?}",
            c.date,
            c.adjust,
            got
        );
    }
}
