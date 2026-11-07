//! Physically renaming pre-rebrand data folders into the current one.
//!
//! The first migration pass *adopted* `%APPDATA%\PecoFence` in place — cheap
//! and data-safe, but it kept the old name alive forever as the live config
//! root. [`migrate_legacy_dirs`] instead moves each legacy directory into
//! `base\CoreDeskEngine` so every resolver ends up on the new path, and the
//! old folder disappears (or is left empty) once the move succeeds.

use std::fs;
use std::path::{Path, PathBuf};

use crate::brand::{ID, LEGACY_DATA_DIRS};

/// What [`move_tree`] did with one legacy directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveOutcome {
    /// `src` was absent (or not a directory) — nothing to do.
    Absent,
    /// `dst` did not exist: the whole directory renamed over in one call.
    Renamed,
    /// `dst` already existed (or the rename was refused): contents merged in.
    /// `conflicts` counts entries preserved under a `.legacy` name because the
    /// destination already had a same-named entry; `leftover` counts entries
    /// that could not move at all (locked, permissions) and remain in `src`.
    Merged { conflicts: usize, leftover: usize },
    /// A filesystem error stopped the move entirely; `src` is untouched.
    Failed(String),
}

/// Move `base\<legacy>` into `base\<ID>` for every known pre-rebrand name,
/// newest first. Returns one entry per attempted source for logging.
pub fn migrate_legacy_dirs(base: &Path) -> Vec<(PathBuf, MoveOutcome)> {
    LEGACY_DATA_DIRS
        .iter()
        .map(|name| {
            let src = base.join(name);
            let outcome = move_tree(&src, &base.join(ID));
            (src, outcome)
        })
        .collect()
}

/// Move `src`'s whole tree to `dst`: a plain rename when `dst` does not exist,
/// an entry-wise merge otherwise. Never deletes or overwrites user data — a
/// name collision keeps the destination's entry and preserves the legacy one
/// under a `.legacy` suffix.
pub fn move_tree(src: &Path, dst: &Path) -> MoveOutcome {
    if src == dst || !src.is_dir() {
        return MoveOutcome::Absent;
    }
    if !dst.exists() && fs::rename(src, dst).is_ok() {
        return MoveOutcome::Renamed;
    }
    merge_children(src, dst)
}

fn merge_children(src: &Path, dst: &Path) -> MoveOutcome {
    if let Err(e) = fs::create_dir_all(dst) {
        return MoveOutcome::Failed(format!("create {}: {e}", dst.display()));
    }
    let mut conflicts = 0;
    let mut leftover = 0;
    let entries = match fs::read_dir(src) {
        Ok(entries) => entries,
        Err(e) => return MoveOutcome::Failed(format!("read {}: {e}", src.display())),
    };
    for entry in entries.flatten() {
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if !to.exists() {
            if fs::rename(&from, &to).is_err() {
                leftover += 1;
            }
        } else if from.is_dir() && to.is_dir() {
            match merge_children(&from, &to) {
                MoveOutcome::Merged {
                    conflicts: c,
                    leftover: l,
                } => {
                    conflicts += c;
                    leftover += l;
                }
                MoveOutcome::Failed(_) => leftover += 1,
                _ => {}
            }
        } else {
            // Same-named entry on both sides: the destination's is newer —
            // keep it, and park the legacy copy beside it instead of dropping.
            match fs::rename(&from, free_legacy_name(&to)) {
                Ok(()) => conflicts += 1,
                Err(_) => leftover += 1,
            }
        }
    }
    if leftover == 0 {
        // Emptied source: remove the now-bare folder; a lingering file
        // junction or stray handle leaves it behind, harmlessly.
        let _ = fs::remove_dir(src);
    }
    MoveOutcome::Merged {
        conflicts,
        leftover,
    }
}

/// `path` with `.legacy` appended — the first free spelling wins
/// (`config.json.legacy`, then `.legacy2`, …).
fn free_legacy_name(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_default();
    for n in 0..100 {
        let suffix = if n == 0 {
            ".legacy".into()
        } else {
            format!(".legacy{n}")
        };
        let candidate = path.with_file_name(format!("{name}{suffix}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    path.with_file_name(format!("{name}.legacy-overflow"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn tmpdir(tag: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("coredesk-migrate-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(path: &Path, body: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }

    #[test]
    fn absent_source_is_a_noop() {
        let base = tmpdir("absent");
        let out = move_tree(&base.join("PecoFence"), &base.join("CoreDeskEngine"));
        assert_eq!(out, MoveOutcome::Absent);
        assert!(!base.join("CoreDeskEngine").exists());
    }

    #[test]
    fn whole_tree_renames_when_destination_is_absent() {
        let base = tmpdir("rename");
        write(&base.join("PecoFence/config.json"), "{}");
        write(&base.join("PecoFence/themes/a.cde"), "x");
        let out = move_tree(&base.join("PecoFence"), &base.join("CoreDeskEngine"));
        assert_eq!(out, MoveOutcome::Renamed);
        assert!(!base.join("PecoFence").exists());
        assert_eq!(
            fs::read_to_string(base.join("CoreDeskEngine/config.json")).unwrap(),
            "{}"
        );
        assert_eq!(
            fs::read_to_string(base.join("CoreDeskEngine/themes/a.cde")).unwrap(),
            "x"
        );
    }

    #[test]
    fn disjoint_contents_merge_and_empty_the_source() {
        let base = tmpdir("merge");
        write(&base.join("PecoFence/config.json"), "{}");
        write(&base.join("PecoFence/themes/a.cde"), "a");
        write(&base.join("CoreDeskEngine/themes/b.cde"), "b");
        write(&base.join("CoreDeskEngine/imported_wallpapers/w.png"), "w");
        let out = move_tree(&base.join("PecoFence"), &base.join("CoreDeskEngine"));
        assert_eq!(
            out,
            MoveOutcome::Merged {
                conflicts: 0,
                leftover: 0
            }
        );
        assert!(!base.join("PecoFence").exists());
        let dst = base.join("CoreDeskEngine");
        for file in [
            "config.json",
            "themes/a.cde",
            "themes/b.cde",
            "imported_wallpapers/w.png",
        ] {
            assert!(dst.join(file).exists(), "{file} missing");
        }
    }

    #[test]
    fn file_conflicts_keep_the_destination_and_park_the_legacy_copy() {
        let base = tmpdir("conflict");
        write(&base.join("PecoFence/config.json"), "old");
        write(&base.join("PecoFence/themes/same.cde"), "old-theme");
        write(&base.join("CoreDeskEngine/config.json"), "new");
        write(&base.join("CoreDeskEngine/themes/same.cde"), "new-theme");
        let out = move_tree(&base.join("PecoFence"), &base.join("CoreDeskEngine"));
        assert_eq!(
            out,
            MoveOutcome::Merged {
                conflicts: 2,
                leftover: 0
            }
        );
        let dst = base.join("CoreDeskEngine");
        assert_eq!(fs::read_to_string(dst.join("config.json")).unwrap(), "new");
        assert_eq!(
            fs::read_to_string(dst.join("config.json.legacy")).unwrap(),
            "old"
        );
        assert_eq!(
            fs::read_to_string(dst.join("themes/same.cde")).unwrap(),
            "new-theme"
        );
        assert_eq!(
            fs::read_to_string(dst.join("themes/same.cde.legacy")).unwrap(),
            "old-theme"
        );
        assert!(!base.join("PecoFence").exists());
    }

    #[test]
    fn migrate_legacy_dirs_picks_every_known_name() {
        let base = tmpdir("all");
        write(&base.join("PecoFence/config.json"), "{}");
        write(&base.join("OpenFence/config.json"), "{}");
        let results = migrate_legacy_dirs(&base);
        assert_eq!(results.len(), 2);
        // PecoFence renames outright; OpenFence then merges into the
        // just-created destination — its config.json parks as `.legacy`.
        assert_eq!(results[0].1, MoveOutcome::Renamed);
        assert_eq!(
            results[1].1,
            MoveOutcome::Merged {
                conflicts: 1,
                leftover: 0
            }
        );
        assert!(!base.join("PecoFence").exists());
        assert!(!base.join("OpenFence").exists());
        // Second pass is a no-op, not a merge into itself.
        let again = migrate_legacy_dirs(&base);
        assert!(again.iter().all(|(_, o)| *o == MoveOutcome::Absent));
        let dst = base.join("CoreDeskEngine");
        assert_eq!(fs::read_to_string(dst.join("config.json")).unwrap(), "{}");
        assert_eq!(
            fs::read_to_string(dst.join("config.json.legacy")).unwrap(),
            "{}"
        );
    }

    #[test]
    fn legacy_data_dirs_order_is_preserved() {
        assert_eq!(LEGACY_DATA_DIRS.first().copied(), Some("PecoFence"));
    }
}
