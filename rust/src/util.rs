//! Filesystem helpers shared by the persisted stores.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// Unique per process and per call: a fixed name would let a second instance (a dev build beside
/// a release, or another session) truncate the scratch file mid-write and publish the remains.
fn temp_sibling(path: &Path) -> PathBuf {
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{}-{}.tmp", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed)));
    path.with_file_name(name)
}

/// Replace `path` without ever leaving it half-written.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = temp_sibling(path);
    let written = (|| {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()
    })();
    if let Err(e) = written {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    // Windows rename replaces an existing destination (MoveFileEx REPLACE_EXISTING), but fails
    // outright while a scanner or sync agent holds it open — where the old in-place write would
    // have succeeded. One retry covers the brief holds; the target is never left missing either way.
    if let Err(e) = fs::rename(&tmp, path) {
        std::thread::sleep(std::time::Duration::from_millis(15));
        if let Err(e2) = fs::rename(&tmp, path) {
            let _ = fs::remove_file(&tmp);
            return Err(if e2.kind() == io::ErrorKind::NotFound { e } else { e2 });
        }
    }
    Ok(())
}
