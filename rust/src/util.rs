//! Filesystem helpers shared by the persisted stores.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

fn temp_sibling(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

/// Replace `path` without ever leaving it half-written: write a sibling temp, flush it, then
/// rename over the target. A torn plain `fs::write` reads back as corrupt, which for config.json
/// means a silent reset to defaults and for a cached sound means a non-empty file that
/// `Path::exists` keeps approving forever.
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
    // Windows rename replaces an existing destination (MoveFileEx REPLACE_EXISTING).
    fs::rename(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}
