//! write_atomic: the destination is never left half-written, and no temp survives.

use prayertray::util::write_atomic;
use std::fs;
use std::path::PathBuf;

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("prayertray-test-{tag}"));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn strays(dir: &PathBuf) -> Vec<String> {
    fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".tmp"))
        .collect()
}

#[test]
fn creates_missing_parent_directories() {
    let d = scratch("mkdir");
    let target = d.join("nested/deeper/config.json");
    write_atomic(&target, b"{}").unwrap();
    assert_eq!(fs::read_to_string(&target).unwrap(), "{}");
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn replaces_an_existing_file_and_leaves_no_temp() {
    let d = scratch("replace");
    let target = d.join("config.json");
    write_atomic(&target, b"first").unwrap();
    write_atomic(&target, b"second").unwrap();
    assert_eq!(fs::read_to_string(&target).unwrap(), "second");
    assert!(strays(&d).is_empty(), "temp file survived: {:?}", strays(&d));
    let _ = fs::remove_dir_all(&d);
}

/// Shrinking content must not leave the tail of the previous write behind — the whole point
/// over an in-place rewrite.
#[test]
fn shorter_content_fully_replaces_longer() {
    let d = scratch("shrink");
    let target = d.join("usage.json");
    write_atomic(&target, b"{\"2026-01-01\":{\"Rx\":1,\"Tx\":2}}").unwrap();
    write_atomic(&target, b"{}").unwrap();
    assert_eq!(fs::read_to_string(&target).unwrap(), "{}");
    let _ = fs::remove_dir_all(&d);
}

/// A write that cannot even start must report the error rather than destroy what is there.
/// Here the parent path is an ordinary file, so create_dir_all fails.
#[test]
fn unwritable_target_errors_without_touching_anything() {
    let d = scratch("blocked");
    let blocker = d.join("blocker");
    fs::write(&blocker, b"i am a file, not a directory").unwrap();
    assert!(write_atomic(&blocker.join("child.json"), b"nope").is_err());
    assert_eq!(fs::read_to_string(&blocker).unwrap(), "i am a file, not a directory");
    assert!(strays(&d).is_empty());
    let _ = fs::remove_dir_all(&d);
}
