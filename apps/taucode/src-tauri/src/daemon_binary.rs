use std::path::PathBuf;

pub(crate) fn daemon_binary_candidates() -> &'static [&'static str] {
    if cfg!(windows) {
        &["codex_monitor_daemon.exe", "codex-monitor-daemon.exe"]
    } else {
        &["codex_monitor_daemon", "codex-monitor-daemon"]
    }
}

fn daemon_search_dirs(executable_dir: &std::path::Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    let mut push_unique = |path: PathBuf| {
        if !dirs.iter().any(|entry| entry == &path) {
            dirs.push(path);
        }
    };

    push_unique(executable_dir.to_path_buf());

    #[cfg(target_os = "macos")]
    {
        if let Some(contents_dir) = executable_dir.parent() {
            push_unique(contents_dir.join("Resources"));
        }
        push_unique(PathBuf::from("/opt/homebrew/bin"));
        push_unique(PathBuf::from("/usr/local/bin"));
    }

    #[cfg(target_os = "linux")]
    {
        push_unique(PathBuf::from("/usr/local/bin"));
        push_unique(PathBuf::from("/usr/bin"));
        push_unique(PathBuf::from("/usr/sbin"));
    }

    dirs
}

pub(crate) fn resolve_daemon_binary_path() -> Result<PathBuf, String> {
    let mut attempted_paths: Vec<PathBuf> = Vec::new();
    let current_exe = std::env::current_exe().map_err(|err| err.to_string())?;
    let parent = current_exe
        .parent()
        .ok_or_else(|| "Unable to resolve executable directory".to_string())?;
    let candidate_names = daemon_binary_candidates();

    if let Ok(explicit_raw) = std::env::var("CODEX_MONITOR_DAEMON_PATH") {
        let explicit = explicit_raw.trim();
        if !explicit.is_empty() {
            let explicit_path = PathBuf::from(explicit);
            if explicit_path.is_file() {
                return Ok(explicit_path);
            }
            if explicit_path.is_dir() {
                for name in candidate_names {
                    let candidate = explicit_path.join(name);
                    if candidate.is_file() {
                        return Ok(candidate);
                    }
                    attempted_paths.push(candidate);
                }
            } else {
                attempted_paths.push(explicit_path);
            }
        }
    }

    for search_dir in daemon_search_dirs(parent) {
        for name in candidate_names {
            let candidate = search_dir.join(name);
            if candidate.is_file() {
                return Ok(candidate);
            }
            attempted_paths.push(candidate);
        }
    }

    let attempted = attempted_paths
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");

    Err(format!(
        "Unable to locate daemon binary (tried: {})",
        attempted
    ))
}

#[cfg(test)]
mod tests {
    use super::daemon_binary_candidates;

    #[test]
    fn daemon_binary_candidates_prioritize_underscored_name() {
        assert!(daemon_binary_candidates()[0].starts_with("codex_monitor_daemon"));
    }
}

pub(crate) fn resolve_diagnostics_binary_path() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let parent = exe.parent().ok_or("Executable directory missing")?;
    #[cfg(debug_assertions)]
    let dev = Some(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../crates/taucode-diag-mcp/target/debug"));
    #[cfg(not(debug_assertions))]
    let dev = None;
    resolve_diagnostics_in(&diagnostics_search_dirs(parent, dev))
}

fn diagnostics_search_dirs(parent: &std::path::Path, dev: Option<PathBuf>) -> Vec<PathBuf> {
    let mut dirs = daemon_search_dirs(parent);
    if let Some(dev) = dev {
        if parent.file_name().is_some_and(|n| n == "deps") {
            if let Some(target) = parent.parent() {
                dirs.insert(0, target.to_path_buf());
            }
        }
        dirs.push(dev);
    }
    dirs
}
fn resolve_diagnostics_in(dirs: &[PathBuf]) -> Result<PathBuf, String> {
    let name = if cfg!(windows) {
        "taucode-diag-mcp.exe"
    } else {
        "taucode-diag-mcp"
    };
    for dir in dirs {
        let path = dir.join(name);
        if path.is_file() {
            return path.canonicalize().map_err(|e| e.to_string());
        }
    }
    Err("Diagnostics MCP binary is unavailable. Build taucode-diag-mcp with cargo build --offline --bins.".into())
}

#[cfg(test)]
mod diagnostics_tests {
    use super::*;
    #[test]
    fn bundle_helpers_take_priority_and_release_has_no_dev_dirs() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let macos = root.join("taucode.app/Contents/MacOS");
        std::fs::create_dir_all(&macos).unwrap();
        let diagnostics = macos.join(if cfg!(windows) { "taucode-diag-mcp.exe" } else { "taucode-diag-mcp" });
        std::fs::write(&diagnostics, b"fixture").unwrap();
        assert_eq!(daemon_search_dirs(&macos)[0], macos);
        assert_eq!(resolve_diagnostics_in(&diagnostics_search_dirs(&macos, None)).unwrap(), diagnostics.canonicalize().unwrap());
        let dev = root.join("development");
        assert!(!diagnostics_search_dirs(&macos, None).contains(&dev));
        assert!(diagnostics_search_dirs(&macos, Some(dev.clone())).contains(&dev));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn resolves_existing_absolute_binary_and_rejects_missing() {
        let dir = std::env::temp_dir().join(format!("diagnostics-binary-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(resolve_diagnostics_in(&[dir.clone()]).is_err());
        let file = dir.join(if cfg!(windows) {
            "taucode-diag-mcp.exe"
        } else {
            "taucode-diag-mcp"
        });
        std::fs::write(&file, b"fixture").unwrap();
        let resolved = resolve_diagnostics_in(&[dir.clone()]).unwrap();
        assert!(resolved.is_absolute());
        assert_eq!(resolved, file.canonicalize().unwrap());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
