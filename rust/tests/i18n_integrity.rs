//! Catalog integrity: English never empty, translations keep the same {n} placeholders.

use prayertray::i18n;

fn placeholders(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            if chars.peek() == Some(&'{') {
                chars.next();
                continue;
            }
            let mut idx = String::new();
            for d in chars.by_ref() {
                if d == '}' {
                    break;
                }
                idx.push(d);
            }
            if idx.chars().all(|c| c.is_ascii_digit()) && !idx.is_empty() {
                out.push(idx);
            }
        }
    }
    out.sort();
    out
}

#[test]
fn catalog_is_consistent() {
    // Exercise the public API across languages for a few known keys.
    for lang in ["en", "ar", "fr", "tr", "ur", "id"] {
        i18n::set(lang);
        assert!(!i18n::t("app.name").is_empty());
        assert!(!i18n::prayer("fajr").is_empty());
        assert!(!i18n::event("eidFitr").is_empty());
        // Composite templates keep placeholders through translation.
        let body = i18n::f("balloon.reminderBody", &["Fajr", "10", "04:00"]);
        assert!(body.contains("Fajr") && body.contains("10"), "{lang}: {body}");
        let tmpl = i18n::t("balloon.reminderBody");
        assert_eq!(placeholders(tmpl), vec!["0", "1", "2"], "{lang}: {tmpl}");
    }
    i18n::set("en");
}

#[test]
fn rtl_flags_and_fallback() {
    i18n::set("ar");
    assert!(i18n::is_rtl());
    i18n::set("ur");
    assert!(i18n::is_rtl());
    i18n::set("tr");
    assert!(!i18n::is_rtl());
    // Unknown key falls back to the key itself.
    i18n::set("en");
    assert_eq!(i18n::t("no.such.key"), "no.such.key");
}
