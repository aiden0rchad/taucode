//! Source synchronization shared by resource staging and app-data installation.
use std::{ffi::OsStr, fs, io, path::Path};

pub fn excluded(name: &OsStr) -> bool {
    matches!(
        name.to_str(),
        Some(
            "node_modules"
                | ".venv"
                | "tests"
                | "__pycache__"
                | "evals"
                | ".pytest_cache"
                | ".installed-lock"
        )
    )
}

fn remove(path: &Path) -> io::Result<()> {
    if fs::symlink_metadata(path)?.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

pub fn copy_tree(source: &Path, target: &Path) -> io::Result<()> {
    // Read the source before changing the destination; never follow destination symlinks.
    let entries = fs::read_dir(source)?.collect::<io::Result<Vec<_>>>()?;
    if let Ok(meta) = fs::symlink_metadata(target) {
        if !meta.is_dir() {
            remove(target)?;
        }
    }
    fs::create_dir_all(target)?;
    let mut names = std::collections::HashSet::new();
    for entry in entries {
        let name = entry.file_name();
        if excluded(&name) {
            continue;
        }
        let kind = entry.file_type()?;
        let path = target.join(&name);
        if kind.is_dir() {
            copy_tree(&entry.path(), &path)?;
        } else if kind.is_file() {
            if let Ok(meta) = fs::symlink_metadata(&path) {
                if !meta.is_file() {
                    remove(&path)?;
                }
            }
            fs::copy(entry.path(), &path)?;
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Unsupported engine resource: {}", entry.path().display()),
            ));
        }
        names.insert(name);
    }
    for entry in fs::read_dir(target)? {
        let entry = entry?;
        let name = entry.file_name();
        // Dependencies and their installation marker are local runtime state.
        if !names.contains(&name)
            && !matches!(
                name.to_str(),
                Some("node_modules" | ".venv" | ".installed-lock")
            )
        {
            remove(&entry.path())?;
        }
    }
    Ok(())
}
