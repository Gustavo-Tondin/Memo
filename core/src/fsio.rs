//! The one way anything in Memo writes a file.
//!
//! Every write is atomic — tmp file, then rename — because a sync tool may
//! read the file at any instant, and a half-written list or config is a
//! corrupted notebook. This used to live in three near-identical copies
//! (lists, config, machine prefs); one helper means the next kind of file
//! (notes, workspace configs) cannot accidentally skip the dance.
//!
//! The invariant test in `core/tests/invariants.rs` enforces that no other
//! module calls `std::fs::write` directly.

use std::path::Path;

use crate::error::{IoContext, Result};

/// Writes `bytes` to `path` atomically, creating parent folders as needed.
///
/// The temporary lands next to the target as `<name>.tmp`, which is what the
/// notebook watcher knows to ignore — using another suffix would make every
/// save look like an external change.
pub fn write_atomically(path: impl AsRef<Path>, bytes: &[u8]) -> Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ctx(parent)?;
    }

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let tmp = path.with_file_name(format!("{name}.tmp"));

    std::fs::write(&tmp, bytes).ctx(&tmp)?;
    std::fs::rename(&tmp, path).ctx(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_and_creates_parents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a/b/list.md");

        write_atomically(&path, b"- [ ] task\n").unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "- [ ] task\n");
        assert!(
            !path.with_file_name("list.md.tmp").exists(),
            "the temporary must be gone after the rename"
        );
    }

    #[test]
    fn the_temporary_has_the_extension_the_watcher_ignores() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Inbox.md");
        let tmp = path.with_file_name("Inbox.md.tmp");
        assert_eq!(tmp.extension().unwrap(), "tmp");
    }
}
