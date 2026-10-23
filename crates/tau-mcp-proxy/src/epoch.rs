use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

static WRITE_LOCK: Mutex<()> = Mutex::new(());
static TEMP_ID: AtomicU64 = AtomicU64::new(0);

fn path(dir: &Path, key: &str) -> PathBuf {
    let mut name: String = key
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if matches!(name.as_str(), "" | "." | "..") {
        name = "_".into();
    }
    dir.join("codegraph-epochs").join(name)
}

pub fn read_epoch(dir: &Path, key: &str) -> u64 {
    fs::read_to_string(path(dir, key))
        .ok()
        .and_then(|text| text.trim().parse().ok())
        .unwrap_or(0)
}

pub fn bump_epoch(dir: &Path, key: &str) -> u64 {
    let _lock = WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let previous = read_epoch(dir, key);
    let result = (|| -> io::Result<u64> {
        let next = previous
            .checked_add(1)
            .ok_or_else(|| io::Error::other("epoch overflow"))?;
        let destination = path(dir, key);
        let parent = destination.parent().expect("epoch directory");
        fs::create_dir_all(parent)?;
        let temporary = parent.join(format!(
            ".epoch-{}-{}.tmp",
            std::process::id(),
            TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        let written = (|| {
            write!(file, "{next}")?;
            file.sync_all()?;
            fs::rename(&temporary, &destination)
        })();
        if written.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        written?;
        Ok(next)
    })();
    result.unwrap_or_else(|error| {
        eprintln!("codegraph: could not bump epoch for {key:?}: {error}");
        previous
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_and_incrementing() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_epoch(dir.path(), "A"), 0);
        assert_eq!(bump_epoch(dir.path(), "A"), 1);
        assert_eq!(bump_epoch(dir.path(), "A"), 2);
        assert_eq!(read_epoch(dir.path(), "A"), 2);
        assert_eq!(read_epoch(dir.path(), "B"), 0);
    }

    #[test]
    fn sanitizes_file_names() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(bump_epoch(dir.path(), "../A/β: b"), 1);
        assert!(dir.path().join("codegraph-epochs/.._A____b").is_file());
        for key in ["", ".", "..", "A-z_0.9"] {
            bump_epoch(dir.path(), key);
            assert_eq!(
                path(dir.path(), key).parent(),
                Some(dir.path().join("codegraph-epochs").as_path())
            );
            assert!(path(dir.path(), key).is_file());
        }
    }

    #[test]
    fn concurrent_bumps_do_not_lose_updates() {
        let dir = tempfile::tempdir().unwrap();
        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| {
                    bump_epoch(dir.path(), "A");
                });
            }
        });
        assert_eq!(read_epoch(dir.path(), "A"), 8);
    }
}
