//! Resolving a user-supplied relative path inside a folder, safely.
//!
//! Every address the app takes — a list, a widget folder, a note — arrives
//! from somewhere the user controls: a config file, a text field, a template
//! someone downloaded. The rule is always the same, so it lives here once
//! instead of being re-derived (slightly differently) in each module.
//!
//! What is refused, and why:
//!
//! - `..` in any form — the whole point;
//! - an absolute path, which would ignore the base entirely;
//! - a component starting with `.` — hidden files are the app's business
//!   (`.memo`, `.workspace.json`), never content;
//! - `\` and NUL, which mean different things on different platforms and
//!   nothing good on any of them;
//! - an empty component (`a//b`), which hides intent.

use std::path::{Path, PathBuf};

/// True when one path component is safe to walk into.
pub fn is_safe_component(part: &str) -> bool {
    !part.trim().is_empty() && !part.starts_with('.') && !part.contains(['\\', '\0'])
}

/// True when `relative` is a safe path inside some base folder.
pub fn is_safe_relative(relative: &str) -> bool {
    !relative.trim().is_empty()
        && !relative.starts_with('/')
        && !relative.contains("..")
        && !relative.contains(['\\', '\0'])
        && relative.split('/').all(is_safe_component)
}

/// Joins `relative` onto `base`, or `None` when it is not safe.
pub fn safe_join(base: &Path, relative: &str) -> Option<PathBuf> {
    is_safe_relative(relative).then(|| base.join(relative))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_ordinary_relative_paths() {
        for good in ["Inbox", "Ideias/2026", "Project A/Backlog", "a b/c d"] {
            assert!(is_safe_relative(good), "{good:?} should be accepted");
        }
    }

    #[test]
    fn refuses_anything_that_climbs_hides_or_escapes() {
        for bad in [
            "..",
            "../fora",
            "a/../b",
            "/etc",
            ".memo",
            "a/.oculto",
            "a\\b",
            "a\0b",
            "",
            "   ",
            "a//b",
        ] {
            assert!(!is_safe_relative(bad), "{bad:?} should be refused");
        }
    }

    #[test]
    fn safe_join_stays_under_the_base() {
        let base = Path::new("/notebook/Notes");
        assert_eq!(
            safe_join(base, "Ideias/nota.md"),
            Some(PathBuf::from("/notebook/Notes/Ideias/nota.md"))
        );
        assert_eq!(safe_join(base, "../../etc/passwd"), None);
    }
}
