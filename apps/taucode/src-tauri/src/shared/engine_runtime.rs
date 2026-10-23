use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Stdio,
    sync::OnceLock,
    time::Duration,
};
use tokio::process::Command;

pub(crate) static RESOURCES: OnceLock<PathBuf> = OnceLock::new();

fn manifest(engine: &str) -> Result<&'static str, String> {
    match engine {
        "pi" | "browser" => Ok("package-lock.json"),
        "computer" => Ok("uv.lock"),
        _ => Err("Unknown engine runtime".into()),
    }
}

fn resolve(
    engine: &str,
    resource: Option<&Path>,
    executable: &Path,
    dev: Option<&Path>,
) -> Result<PathBuf, String> {
    let lock = manifest(engine)?;
    let parent = executable.parent().ok_or("Missing executable directory")?;
    resource
        .map(Path::to_path_buf)
        .into_iter()
        .chain([parent.to_path_buf(), parent.join("../Resources")])
        .map(|root| root.join("engines").join(engine))
        .chain(dev.map(|root| root.join(engine)))
        .find(|dir| dir.join(lock).is_file())
        .ok_or_else(|| {
            format!("Bundled {engine} runtime resources are missing. Reinstall taucode.")
        })
}

pub(crate) fn source(engine: &str, resource: Option<&Path>) -> Result<PathBuf, String> {
    #[cfg(debug_assertions)]
    let dev = Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("../engines"));
    #[cfg(not(debug_assertions))]
    let dev: Option<PathBuf> = None;
    resolve(
        engine,
        resource.or(RESOURCES.get().map(PathBuf::as_path)),
        &std::env::current_exe().map_err(|e| e.to_string())?,
        dev.as_deref(),
    )
}

fn copy_tree(source: &Path, target: &Path) -> Result<(), String> {
    fs::create_dir_all(target).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(source).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if matches!(
            entry.file_name().to_str(),
            Some("node_modules" | ".venv" | "__pycache__" | ".pytest_cache" | "tests")
        ) {
            continue;
        }
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        let path = target.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&entry.path(), &path)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn executable(engine: &str) -> &'static str {
    match engine {
        "pi" => "node_modules/@earendil-works/pi-coding-agent/dist/bundle/cli.js",
        "browser" => "node_modules/@playwright/mcp/cli.js",
        _ if cfg!(windows) => ".venv/Scripts/python.exe",
        _ => ".venv/bin/python",
    }
}

fn status_file(data: &Path, engine: &str, state: &str, message: &str) -> Result<(), String> {
    let path = data.join("engines").join(format!("{engine}-setup.json"));
    let temp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    fs::write(
        &temp,
        json!({"engine":engine,"state":state,"message":message}).to_string(),
    )
    .and_then(|_| fs::rename(&temp, path))
    .map_err(|e| e.to_string())
}

pub(crate) fn status(data: &Path) -> Value {
    json!(["pi", "browser", "computer"].map(|engine| {
        fs::read(data.join("engines").join(format!("{engine}-setup.json"))).ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .unwrap_or_else(|| json!({"engine":engine,"state":"pending","message":"Ready to set up on first use"}))
    }))
}

fn supported_node(version: &str) -> bool {
    let parts = version
        .trim()
        .trim_start_matches('v')
        .split('.')
        .map(str::parse::<u64>)
        .collect::<Result<Vec<_>, _>>();
    parts.is_ok_and(|p| p.len() == 3 && (p[0], p[1]) >= (22, 19))
}

pub(crate) async fn check_node(node: &std::ffi::OsStr) -> Result<(), String> {
    let output = tokio::time::timeout(
        Duration::from_secs(15),
        Command::new(node)
            .arg("--version")
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| "Node version check timed out")?
    .map_err(|_| "Install system Node >=22.19 (and npm), or set TAUCODE_NODE to its executable")?;
    if !output.status.success() || !supported_node(&String::from_utf8_lossy(&output.stdout)) {
        return Err("Install system Node >=22.19 (and npm) to run pi and browser tools".into());
    }
    Ok(())
}

pub(crate) async fn prepare(
    data: &Path,
    engine: &str,
    resource: Option<&Path>,
) -> Result<PathBuf, String> {
    let lock_name = manifest(engine)?;
    let root = data.join("engines");
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    // The browser launcher, daemon and app can all request the same install.
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(root.join(format!("{engine}.lock")))
        .map_err(|e| e.to_string())?;
    loop {
        match lock.try_lock() {
            Ok(()) => break,
            Err(fs::TryLockError::WouldBlock) => {
                tokio::time::sleep(Duration::from_millis(100)).await
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    let result = async {
        status_file(data, engine, "preparing", "Checking bundled runtime…")?;
        let src = source(engine, resource)?;
        let dir = root.join(engine);
        let locked = fs::read(src.join(lock_name)).map_err(|e| e.to_string())?;
        let installed = fs::read(dir.join(".installed-lock")).ok();
        copy_tree(&src, &dir)?;
        if engine != "computer" {
            check_node(&std::env::var_os("TAUCODE_NODE").unwrap_or_else(|| "node".into())).await?;
        }
        if installed.as_ref() != Some(&locked) || !dir.join(executable(engine)).is_file() {
            if dir.join(".installed-lock").exists() {
                fs::remove_file(dir.join(".installed-lock")).map_err(|e| e.to_string())?;
            }
            let mut command = if engine == "computer" {
                status_file(data, engine, "installing", "Installing Python 3.12 dependencies with uv…")?;
                let mut cmd = Command::new("uv");
                cmd.args(["sync", "--frozen", "--no-dev", "--python", "3.12"]);
                cmd
            } else {
                status_file(data, engine, "installing", "Installing Node dependencies from the bundled lockfile…")?;
                let mut cmd = Command::new(if cfg!(windows) { "npm.cmd" } else { "npm" });
                cmd.args(["ci", "--omit=dev", "--no-audit", "--no-fund"]);
                cmd
            };
            let output = command.current_dir(&dir).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).kill_on_drop(true).status().await
                .map_err(|_| if engine == "computer" { "Install uv to prepare Computer Use" } else { "Install system Node >=22.19 and npm to prepare this runtime" })?;
            if !output.success() || !dir.join(executable(engine)).is_file() {
                return Err(format!("{engine} runtime installation failed. Check {} and network access, then retry setup in Settings.", if engine == "computer" { "uv" } else { "Node >=22.19 and npm" }));
            }
            fs::write(dir.join(".installed-lock"), locked).map_err(|e| e.to_string())?;
        }
        Ok(dir)
    }.await;
    match &result {
        Ok(_) => status_file(data, engine, "ready", "Ready")?,
        Err(error) => status_file(data, engine, "error", error)?,
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resolves_packaged_daemon_and_debug_paths_without_release_fallback() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        for engine in ["pi", "browser", "computer"] {
            for dir in [
                root.join("resources/engines"),
                root.join("bin/engines"),
                root.join("dev"),
            ] {
                fs::create_dir_all(dir.join(engine)).unwrap();
                fs::write(dir.join(engine).join(manifest(engine).unwrap()), b"locked").unwrap();
            }
            let exe = root.join("bin/daemon");
            assert_eq!(
                resolve(
                    engine,
                    Some(&root.join("resources")),
                    &exe,
                    Some(&root.join("dev"))
                )
                .unwrap(),
                root.join("resources/engines").join(engine)
            );
            assert_eq!(
                resolve(engine, None, &exe, None).unwrap(),
                root.join("bin/engines").join(engine)
            );
            let exe = root.join("missing/app");
            assert!(resolve(engine, None, &exe, None).is_err());
            assert_eq!(
                resolve(engine, None, &exe, Some(&root.join("dev"))).unwrap(),
                root.join("dev").join(engine)
            );
        }
        assert!(resolve("../escape", None, &root.join("app"), None).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn validates_node_version() {
        for version in ["v22.19.0\n", "v23.0.0", "v24.1.2"] {
            assert!(supported_node(version));
        }
        for version in ["v22.18.9", "v20.99.0", "garbage", "22.19"] {
            assert!(!supported_node(version));
        }
    }
    #[tokio::test]
    async fn prepared_environment_reuses_lock_and_copies_only_sources() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let resources = root.join("resources");
        let src = resources.join("engines/computer");
        let data = root.join("data");
        let installed = data.join("engines/computer");
        fs::create_dir_all(src.join("taucode_cua/__pycache__")).unwrap();
        fs::create_dir_all(src.join(".venv")).unwrap();
        fs::write(src.join("uv.lock"), "locked").unwrap();
        fs::write(src.join("pyproject.toml"), "project").unwrap();
        fs::write(src.join("taucode_cua/server.py"), "source").unwrap();
        fs::write(src.join("taucode_cua/__pycache__/skip.pyc"), "skip").unwrap();
        fs::write(src.join(".venv/skip"), "skip").unwrap();
        let python = installed.join(executable("computer"));
        fs::create_dir_all(python.parent().unwrap()).unwrap();
        fs::write(&python, "installed python").unwrap();
        fs::write(installed.join(".installed-lock"), "locked").unwrap();
        assert_eq!(
            prepare(&data, "computer", Some(&resources)).await.unwrap(),
            installed
        );
        assert_eq!(fs::read_to_string(python).unwrap(), "installed python");
        assert_eq!(
            fs::read_to_string(installed.join("taucode_cua/server.py")).unwrap(),
            "source"
        );
        assert!(!installed.join(".venv/skip").exists());
        assert!(!installed.join("taucode_cua/__pycache__").exists());
        assert_eq!(status(&data)[2]["state"], "ready");
        fs::remove_dir_all(root).unwrap();
    }
}
