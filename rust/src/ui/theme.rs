//! Theme palettes + global font style, port of UI/Theme.cs. Colors are 0xAARRGGBB.

use std::sync::Mutex;
use windows::core::w;
use windows::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ, REG_DWORD,
    REG_VALUE_TYPE,
};

pub const fn argb(r: u8, g: u8, b: u8) -> u32 {
    0xFF00_0000 | ((r as u32) << 16) | ((g as u32) << 8) | b as u32
}

/// Blend two 0xAARRGGBB colours. `t` is clamped, so callers need not.
pub fn mix(a: u32, b: u32, t: f32) -> u32 {
    let t = t.clamp(0.0, 1.0);
    let channel = |shift: u32| {
        let (from, to) = (((a >> shift) & 0xFF) as f32, ((b >> shift) & 0xFF) as f32);
        ((from + (to - from) * t).round() as u32) << shift
    };
    0xFF00_0000 | channel(16) | channel(8) | channel(0)
}

/// good -> warn -> urgent as `t` runs 0..=1, for the closing prayer window.
pub fn urgency(pal: &Palette, t: f32) -> u32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        mix(pal.good, pal.warn, t * 2.0)
    } else {
        mix(pal.warn, pal.urgent, (t - 0.5) * 2.0)
    }
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: u32,
    pub bg_hover: u32,
    pub panel: u32,
    pub text: u32,
    pub text_dim: u32,
    pub accent: u32,
    pub accent_soft: u32,
    pub good: u32,
    pub warn: u32,
    pub urgent: u32,
    pub is_dark: bool,
}

pub const DARK: Palette = Palette {
    bg: argb(32, 32, 32),
    bg_hover: argb(48, 48, 50),
    panel: argb(43, 43, 46),
    text: argb(240, 240, 240),
    text_dim: argb(165, 170, 178),
    accent: argb(96, 205, 255),
    accent_soft: argb(38, 64, 78),
    good: argb(126, 224, 158),
    warn: argb(240, 200, 110),
    urgent: argb(240, 130, 120),
    is_dark: true,
};

pub const LIGHT: Palette = Palette {
    bg: argb(243, 243, 243),
    bg_hover: argb(225, 225, 228),
    panel: argb(250, 250, 252),
    text: argb(24, 24, 24),
    text_dim: argb(92, 98, 108),
    accent: argb(0, 120, 200),
    accent_soft: argb(205, 230, 245),
    good: argb(30, 150, 80),
    warn: argb(176, 124, 0),
    urgent: argb(190, 45, 40),
    is_dark: false,
};

pub const MIDNIGHT: Palette = Palette {
    bg: argb(16, 22, 40),
    bg_hover: argb(28, 36, 60),
    panel: argb(22, 30, 52),
    text: argb(232, 236, 245),
    text_dim: argb(150, 160, 185),
    accent: argb(120, 160, 255),
    accent_soft: argb(36, 48, 90),
    good: argb(120, 220, 170),
    warn: argb(235, 200, 130),
    urgent: argb(240, 130, 145),
    is_dark: true,
};

pub const SLATE: Palette = Palette {
    bg: argb(30, 34, 42),
    bg_hover: argb(44, 50, 60),
    panel: argb(38, 43, 52),
    text: argb(232, 236, 242),
    text_dim: argb(150, 158, 170),
    accent: argb(130, 180, 210),
    accent_soft: argb(48, 62, 76),
    good: argb(130, 210, 165),
    warn: argb(226, 196, 128),
    urgent: argb(232, 130, 125),
    is_dark: true,
};

pub const WARM: Palette = Palette {
    bg: argb(34, 30, 26),
    bg_hover: argb(50, 44, 38),
    panel: argb(44, 39, 34),
    text: argb(244, 238, 230),
    text_dim: argb(180, 168, 150),
    accent: argb(255, 180, 90),
    accent_soft: argb(78, 60, 38),
    good: argb(200, 210, 120),
    warn: argb(245, 195, 105),
    urgent: argb(240, 135, 105),
    is_dark: true,
};

pub const NAMES: [&str; 6] = ["Auto", "Dark", "Light", "Midnight", "Slate", "Warm"];

struct Style {
    palette: Palette,
    family: String,
    font_scale: f32,
}

static STYLE: Mutex<Style> = Mutex::new(Style {
    palette: DARK,
    family: String::new(), // empty -> "Segoe UI" at use
    font_scale: 1.0,
});

pub fn apply(name: &str, family: &str, font_scale: f32) {
    let mut s = STYLE.lock().unwrap();
    s.palette = resolve(name);
    s.family = family.to_string();
    s.font_scale = font_scale;
}

pub fn current() -> Palette {
    STYLE.lock().unwrap().palette
}

pub fn family() -> String {
    let s = STYLE.lock().unwrap();
    if s.family.is_empty() {
        "Segoe UI".into()
    } else {
        s.family.clone()
    }
}

pub fn font_scale() -> f32 {
    STYLE.lock().unwrap().font_scale
}

fn resolve(name: &str) -> Palette {
    match name {
        "Dark" => DARK,
        "Light" => LIGHT,
        "Midnight" => MIDNIGHT,
        "Slate" => SLATE,
        "Warm" => WARM,
        _ => {
            if windows_uses_light_theme() {
                LIGHT
            } else {
                DARK
            }
        }
    }
}

fn windows_uses_light_theme() -> bool {
    unsafe {
        let mut key = HKEY::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            None,
            KEY_READ,
            &mut key,
        )
        .is_err()
        {
            return false;
        }
        let mut ty = REG_VALUE_TYPE::default();
        let mut val = 0u32;
        let mut len = 4u32;
        let ok = RegQueryValueExW(
            key,
            w!("AppsUseLightTheme"),
            None,
            Some(&mut ty),
            Some(&mut val as *mut u32 as *mut u8),
            Some(&mut len),
        )
        .is_ok();
        let _ = RegCloseKey(key);
        ok && ty == REG_DWORD && val != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mix_hits_both_ends_and_the_middle() {
        let (a, b) = (argb(0, 0, 0), argb(255, 255, 255));
        assert_eq!(mix(a, b, 0.0), a);
        assert_eq!(mix(a, b, 1.0), b);
        assert_eq!(mix(a, b, 0.5), argb(128, 128, 128)); // .5 rounds away from zero
        assert_eq!(mix(a, b, -3.0), a, "t is clamped for the caller");
        assert_eq!(mix(a, b, 9.0), b);
    }

    #[test]
    fn urgency_runs_good_to_warn_to_urgent() {
        let p = DARK;
        assert_eq!(urgency(&p, 0.0), p.good, "a wide-open window stays the normal colour");
        assert_eq!(urgency(&p, 0.5), p.warn);
        assert_eq!(urgency(&p, 1.0), p.urgent);
        // Monotonic through the first half: the red channel only climbs toward warn.
        let (q, h) = (urgency(&p, 0.25), urgency(&p, 0.4));
        assert!((q >> 16) & 0xFF <= (h >> 16) & 0xFF);
    }

    #[test]
    fn every_palette_defines_the_urgency_stops() {
        for p in [DARK, LIGHT, MIDNIGHT, SLATE, WARM] {
            assert_ne!(p.good, p.warn, "warn must be distinguishable from good");
            assert_ne!(p.warn, p.urgent);
        }
    }
}
