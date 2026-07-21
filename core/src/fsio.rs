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

use std::path::{Path, PathBuf};

use crate::error::{Error, IoContext, Result};

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

/// Overrides where deleted files go, instead of the OS trash.
///
/// Two uses, same as [`crate::prefs`]'s `MEMO_CONFIG_DIR`: the tests, which
/// must not fill the user's real trash with fixtures, and anyone who wants
/// the notebook's deletions kept next to the notebook — a portable install,
/// a synced folder, a machine with no desktop trash at all.
const TRASH_DIR_ENV: &str = "MEMO_TRASH_DIR";

/// Sends a file to the trash instead of destroying it.
///
/// Deleting is a filing decision, not a decision to burn the file — the same
/// rule that makes a deleted list rescue its tasks (principle 2, the data is
/// the user's). Obsidian does exactly this, and for the same reason: the user
/// can change their mind, and the app is not the only thing that knows what
/// was in there.
///
/// Refuses rather than falling back to a permanent delete. A "delete" that
/// silently destroys the file when the trash is unavailable would be the one
/// outcome this function exists to prevent.
pub fn move_to_trash(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();

    if let Some(dir) = std::env::var_os(TRASH_DIR_ENV) {
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir).ctx(&dir)?;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "deleted".to_string());
        // Two files of the same name deleted in a row must both survive.
        let target = free_name(&dir, &name);
        return std::fs::rename(path, &target).ctx(&target);
    }

    trash::delete(path).map_err(|source| Error::Trash {
        path: path.to_path_buf(),
        message: source.to_string(),
    })
}

/// A free path in `dir` for `name`, suffixed until nothing is overwritten.
fn free_name(dir: &Path, name: &str) -> PathBuf {
    let candidate = dir.join(name);
    if !candidate.exists() {
        return candidate;
    }
    let (stem, extension) = match name.rsplit_once('.') {
        Some((stem, ext)) => (stem.to_string(), format!(".{ext}")),
        None => (name.to_string(), String::new()),
    };
    for attempt in 2.. {
        let candidate = dir.join(format!("{stem} {attempt}{extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!("the loop returns as soon as a name is free")
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
