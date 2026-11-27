//! Prepare all writes before modifying originals and roll back ordinary I/O failures.
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};
use tempfile::{NamedTempFile, TempPath};

pub(crate) struct Change {
    pub(crate) from: PathBuf,
    pub(crate) to: PathBuf,
    pub(crate) before: String,
    pub(crate) after: String,
}
pub(crate) struct ChangePlan {
    root: PathBuf,
    pub(crate) changes: Vec<Change>,
}
struct Prepared {
    next: TempPath,
    backup: TempPath,
}
#[derive(Default)]
struct Directories(Vec<PathBuf>);
impl Drop for Directories {
    fn drop(&mut self) {
        for path in self.0.iter().rev() {
            let _ = fs::remove_dir(path);
        }
    }
}
impl ChangePlan {
    pub(crate) fn new(root: &Path) -> Self {
        Self {
            root: root.canonicalize().unwrap_or_else(|_| root.to_path_buf()),
            changes: Vec::new(),
        }
    }
    pub(crate) fn apply(&self) -> Result<(), String> {
        let moved = self
            .changes
            .iter()
            .filter(|change| change.from != change.to)
            .map(|change| change.to.as_path())
            .collect::<std::collections::HashSet<_>>();
        self.apply_with(|from, to| {
            if moved.contains(to) {
                // Staging is in the destination directory, so this is an atomic no-clobber publish.
                fs::hard_link(from, to)
            } else {
                fs::rename(from, to)
            }
        })
    }
    fn apply_with(
        &self,
        mut replace: impl FnMut(&Path, &Path) -> io::Result<()>,
    ) -> Result<(), String> {
        let mut directories = Directories::default();
        let mut prepared = self
            .changes
            .iter()
            .map(|change| self.prepare(change, &mut directories))
            .collect::<Result<Vec<_>, _>>()?;
        let mut committed = Vec::new();
        for (index, (change, staged)) in self.changes.iter().zip(&prepared).enumerate() {
            let result = (|| {
                validate_snapshot(change)?;
                replace(&staged.next, &change.to)
                    .map_err(|e| format!("{}: {e}", change.to.display()))?;
                committed.push(index);
                if change.from != change.to {
                    fs::remove_file(&change.from).map_err(|e| e.to_string())?;
                }
                Ok::<_, String>(())
            })();
            if let Err(error) = result {
                return Err(self.rollback(&mut prepared, &committed, error));
            }
        }
        Ok(())
    }
    fn prepare(&self, change: &Change, directories: &mut Directories) -> Result<Prepared, String> {
        validate_snapshot(change)?;
        let metadata = fs::symlink_metadata(&change.from).map_err(|e| e.to_string())?;
        if !metadata.is_file() || metadata.permissions().readonly() {
            return Err(format!(
                "{}: source is not a writable regular file",
                change.from.display()
            ));
        }
        let parent = change.to.parent().ok_or("destination has no parent")?;
        if parent
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            return Err("destination must stay inside wiki root".into());
        }
        let mut ancestor = parent;
        while !ancestor.exists() {
            ancestor = ancestor.parent().ok_or("destination has no ancestor")?;
        }
        if !fs::canonicalize(ancestor)
            .map_err(|e| e.to_string())?
            .starts_with(&self.root)
        {
            return Err("destination directory escapes wiki root".into());
        }
        make_directories(parent, directories)?;
        let backup = staged_file(
            change.from.parent().ok_or("source has no parent")?,
            &change.before,
            &metadata,
        )?;
        let next = staged_file(parent, &change.after, &metadata)?;
        Ok(Prepared { next, backup })
    }
    fn rollback(&self, prepared: &mut [Prepared], committed: &[usize], error: String) -> String {
        let mut errors = vec![error];
        for &index in committed.iter().rev() {
            let change = &self.changes[index];
            let backup = &prepared[index].backup;
            let restore = (|| {
                if fs::read_to_string(&change.to).map_err(|e| e.to_string())? != change.after {
                    return Err("destination changed during rollback".to_string());
                }
                if change.from != change.to
                    && change.from.exists()
                    && fs::read_to_string(&change.from).map_err(|e| e.to_string())? != change.before
                {
                    return Err("source changed during rollback".into());
                }
                fs::rename(backup, &change.from).map_err(|e| e.to_string())?;
                if change.from != change.to {
                    fs::remove_file(&change.to).map_err(|e| e.to_string())?;
                }
                Ok::<_, String>(())
            })();
            if let Err(reason) = restore {
                if prepared[index].backup.exists() {
                    let path = prepared[index].backup.to_path_buf();
                    prepared[index].backup.disable_cleanup(true);
                    errors.push(format!(
                        "rollback {}: {reason}; recovery: {}",
                        change.from.display(),
                        path.display()
                    ));
                } else {
                    errors.push(format!(
                        "rollback cleanup {}: {reason}; original restored at {}",
                        change.to.display(),
                        change.from.display()
                    ));
                }
            }
        }
        errors.join("; ")
    }
}
fn validate_snapshot(change: &Change) -> Result<(), String> {
    let current =
        fs::read_to_string(&change.from).map_err(|e| format!("{}: {e}", change.from.display()))?;
    if current != change.before {
        return Err(format!(
            "{}: changed since planning; no overwrite",
            change.from.display()
        ));
    }
    if change.from != change.to && change.to.exists() {
        return Err(format!(
            "{}: destination already exists",
            change.to.display()
        ));
    }
    Ok(())
}
fn make_directories(parent: &Path, directories: &mut Directories) -> Result<(), String> {
    if parent.exists() {
        return Ok(());
    }
    make_directories(
        parent.parent().ok_or("destination has no ancestor")?,
        directories,
    )?;
    fs::create_dir(parent).map_err(|e| e.to_string())?;
    directories.0.push(parent.to_path_buf());
    Ok(())
}
fn staged_file(parent: &Path, text: &str, metadata: &fs::Metadata) -> Result<TempPath, String> {
    let mut file = NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    file.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
    file.as_file()
        .set_permissions(metadata.permissions())
        .map_err(|e| e.to_string())?;
    file.as_file().sync_all().map_err(|e| e.to_string())?;
    Ok(file.into_temp_path())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn plan(root: &Path) -> ChangePlan {
        let mut plan = ChangePlan::new(root);
        for name in ["A", "B"] {
            let from = root.join(format!("{name}.md"));
            fs::write(&from, name).unwrap();
            plan.changes.push(Change {
                from: from.clone(),
                to: from,
                before: name.into(),
                after: format!("new {name}"),
            });
        }
        plan
    }
    #[test]
    fn commit_failure_restores_all_files_and_cleans_staging() {
        let dir = tempfile::tempdir().unwrap();
        let plan = plan(dir.path());
        let result = plan.apply_with(|from, to| {
            if to.ends_with("B.md") {
                Err(io::Error::other("disk full"))
            } else {
                fs::rename(from, to)
            }
        });
        assert!(result.unwrap_err().contains("disk full"));
        assert_eq!(fs::read_to_string(dir.path().join("A.md")).unwrap(), "A");
        assert_eq!(fs::read_to_string(dir.path().join("B.md")).unwrap(), "B");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }
    #[test]
    fn stale_plans_and_existing_move_destinations_do_not_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let mut plan = plan(dir.path());
        fs::write(&plan.changes[1].from, "external edit").unwrap();
        assert!(plan.apply().is_err());
        assert_eq!(fs::read_to_string(&plan.changes[0].from).unwrap(), "A");
        fs::write(&plan.changes[1].from, "B").unwrap();
        plan.changes[0].to = plan.changes[1].from.clone();
        assert!(plan.apply().is_err());
    }
    #[test]
    fn failed_move_restores_names_and_removes_created_directories() {
        let dir = tempfile::tempdir().unwrap();
        let mut plan = plan(dir.path());
        plan.changes[0].to = dir.path().join("new/A.md");
        assert!(plan
            .apply_with(|from, to| if to.ends_with("B.md") {
                Err(io::Error::other("failure"))
            } else {
                fs::rename(from, to)
            })
            .is_err());
        assert_eq!(fs::read_to_string(dir.path().join("A.md")).unwrap(), "A");
        assert!(!dir.path().join("new").exists());
    }
}

#[cfg(test)]
mod recovery_tests {
    use super::*;
    #[test]
    fn rollback_conflicts_keep_external_edits_and_recovery_material() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("A.md");
        let b = dir.path().join("B.md");
        fs::write(&a, "A").unwrap();
        fs::write(&b, "B").unwrap();
        let plan = ChangePlan {
            root: dir.path().canonicalize().unwrap(),
            changes: vec![
                Change {
                    from: a.clone(),
                    to: a.clone(),
                    before: "A".into(),
                    after: "new A".into(),
                },
                Change {
                    from: b.clone(),
                    to: b.clone(),
                    before: "B".into(),
                    after: "new B".into(),
                },
            ],
        };
        let error = plan
            .apply_with(|from, to| {
                if to == b {
                    fs::write(&a, "external edit")?;
                    Err(io::Error::other("commit failed"))
                } else {
                    fs::rename(from, to)
                }
            })
            .unwrap_err();
        assert!(error.contains("recovery:"));
        assert_eq!(fs::read_to_string(&a).unwrap(), "external edit");
        let backup = error.rsplit("recovery: ").next().unwrap();
        assert_eq!(fs::read_to_string(backup).unwrap(), "A");
        assert_eq!(fs::read_to_string(&b).unwrap(), "B");
    }
    #[cfg(unix)]
    #[test]
    fn destination_symlinks_cannot_create_files_outside_the_wiki() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let from = dir.path().join("A.md");
        fs::write(&from, "A").unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("escape")).unwrap();
        let mut plan = ChangePlan::new(dir.path());
        plan.changes.push(Change {
            from: from.clone(),
            to: dir.path().join("escape/new/A.md"),
            before: "A".into(),
            after: "new A".into(),
        });
        assert!(plan.apply().unwrap_err().contains("escapes"));
        assert_eq!(fs::read_to_string(&from).unwrap(), "A");
        assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
    }
}
