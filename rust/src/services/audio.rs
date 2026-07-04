//! Fire-and-forget MP3/WAV playback via winmm MCI, port of Services/AudioPlayer.cs.
//! Single track at a time. Embedded adhans extract once to %TEMP%\PrayerTray
//! (MCI can't play from memory — same behavior the C# build ships).

use crate::config::AppConfig;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use windows::core::PCWSTR;
use windows::Win32::Media::Multimedia::mciSendStringW;

const MAKKAH_MP3: &[u8] = include_bytes!("../../../assets/azan-makkah.mp3");
const MADINAH_MP3: &[u8] = include_bytes!("../../../assets/azan-madinah.mp3");

pub const BUILTIN_ADHANS: [(&str, &str); 2] = [("makkah", "Makkah"), ("madinah", "Madinah")];

pub const REMINDER_SOUNDS: [(&str, &str); 5] = [
    ("chime", "Chime"),
    ("bell", "Bell"),
    ("ding", "Ding"),
    ("beep", "Beep"),
    ("double", "Double beep"),
];

static OPEN: Mutex<bool> = Mutex::new(false);

fn mci(cmd: &str) -> bool {
    let wide: Vec<u16> = cmd.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe { mciSendStringW(PCWSTR(wide.as_ptr()), None, None) == 0 }
}

fn temp_dir() -> PathBuf {
    let d = std::env::temp_dir().join("PrayerTray");
    let _ = std::fs::create_dir_all(&d);
    d
}

/// Start playing a file (async). Replaces any current track.
pub fn play(path: &Path) {
    if !path.exists() {
        return;
    }
    let mut open = OPEN.lock().unwrap();
    if *open {
        mci("close ptaudio");
        *open = false;
    }
    let kind = if path.extension().map(|e| e.eq_ignore_ascii_case("wav")).unwrap_or(false) {
        "waveaudio"
    } else {
        "mpegvideo"
    };
    if !mci(&format!("open \"{}\" type {} alias ptaudio", path.display(), kind)) {
        return;
    }
    *open = true;
    mci("play ptaudio");
}

pub fn stop() {
    let mut open = OPEN.lock().unwrap();
    if *open {
        mci("close ptaudio");
        *open = false;
    }
}

/// Extract a bundled adhan to temp (cached), or None if that id isn't embedded.
pub fn builtin_adhan_path(id: &str) -> Option<PathBuf> {
    let bytes = match id {
        "makkah" => MAKKAH_MP3,
        "madinah" => MADINAH_MP3,
        _ => return None,
    };
    let out = temp_dir().join(format!("azan-{id}.mp3"));
    if !out.exists() {
        std::fs::write(&out, bytes).ok()?;
    }
    Some(out)
}

/// Play the reminder sound chosen in config (custom file, else a synth-bank tone).
pub fn play_reminder(cfg: &AppConfig) {
    if cfg.reminder_sound_id == "custom" {
        if let Some(p) = &cfg.reminder_sound_path {
            if !p.trim().is_empty() {
                play(Path::new(p));
                return;
            }
        }
    }
    play(&synth_path(&cfg.reminder_sound_id));
}

/// A short synthesized WAV for the given bank id, generated once into temp.
pub fn synth_path(id: &str) -> PathBuf {
    let out = temp_dir().join(format!("rem-{id}.wav"));
    if !out.exists() {
        let _ = std::fs::write(&out, build_sound(id));
    }
    out
}

/// (freq Hz, length ms, decay) blips in sequence; decay>0 = exponential ring-out (bell).
fn build_sound(id: &str) -> Vec<u8> {
    let blips: &[(f64, i32, f64)] = match id {
        "bell" => &[(659.3, 900, 4.5)],
        "ding" => &[(1046.5, 350, 3.0)],
        "beep" => &[(880.0, 180, 0.0)],
        "double" => &[(988.0, 110, 0.0), (0.0, 70, 0.0), (988.0, 110, 0.0)],
        _ => &[(880.0, 160, 0.0), (1174.7, 240, 1.5)], // chime
    };
    const RATE: i32 = 44100;
    const BITS: i32 = 16;
    let total: i32 = blips.iter().map(|(_, ms, _)| RATE * ms / 1000).sum();
    let data_bytes = total * (BITS / 8);

    let mut w = Vec::with_capacity(44 + data_bytes as usize);
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + data_bytes).to_le_bytes());
    w.extend_from_slice(b"WAVE");
    w.extend_from_slice(b"fmt ");
    w.extend_from_slice(&16i32.to_le_bytes());
    w.extend_from_slice(&1i16.to_le_bytes()); // PCM
    w.extend_from_slice(&1i16.to_le_bytes()); // mono
    w.extend_from_slice(&RATE.to_le_bytes());
    w.extend_from_slice(&(RATE * (BITS / 8)).to_le_bytes());
    w.extend_from_slice(&((BITS / 8) as i16).to_le_bytes());
    w.extend_from_slice(&(BITS as i16).to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&data_bytes.to_le_bytes());

    for &(freq, ms_len, decay) in blips {
        let n = RATE * ms_len / 1000;
        let fade = RATE * 8 / 1000; // 8 ms ramps to avoid clicks
        for i in 0..n {
            if freq <= 0.0 {
                w.extend_from_slice(&0i16.to_le_bytes()); // silence gap
                continue;
            }
            let env = if decay > 0.0 {
                (-decay * i as f64 / n as f64).exp() * (i as f64 / fade as f64).min(1.0)
            } else {
                (i.min(n - i) as f64 / fade as f64).min(1.0)
            };
            let sample = (2.0 * std::f64::consts::PI * freq * i as f64 / RATE as f64).sin() * env * 0.35;
            w.extend_from_slice(&((sample * i16::MAX as f64) as i16).to_le_bytes());
        }
    }
    w
}
