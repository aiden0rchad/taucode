#[path = "engine_files.rs"]
mod engine_files;
use engine_files::copy_tree;

use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Stdio,
    sync::OnceLock,
    time::Duration,
};
use tokio::process::Command;

static INSTALLS: OnceLock<std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<tokio::sync::Notify>>>> = OnceLock::new();
pub(crate) fn cancel(engine: &str) {
    if let Some(notify) = INSTALLS.get_or_init(Default::default).lock().unwrap().get(engine) { notify.notify_one(); }
}
pub(crate) static RESOURCES: OnceLock<PathBuf> = OnceLock::new();

fn manifest(engine: &str) -> Result<&'static str, String> {
    match engine {
        "pi" | "omp" | "browser" | "claude-acp" => Ok("package-lock.json"),
        "computer" | "work" => Ok("uv.lock"),
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

fn executable(engine: &str) -> &'static str {
    match engine {
        "pi" => "node_modules/@earendil-works/pi-coding-agent/dist/bundle/cli.js",
        "omp" => "node_modules/@oh-my-pi/pi-coding-agent/dist/cli.js",
        "browser" => "node_modules/@playwright/mcp/cli.js",
        "claude-acp" => "node_modules/@agentclientprotocol/claude-agent-acp/dist/index.js",
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

pub(crate) async fn status(data: &Path) -> Value {
    let mut status = json!(["pi", "browser", "computer", "claude-acp", "work", "omp"].map(|engine| {
        fs::read(data.join("engines").join(format!("{engine}-setup.json"))).ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .unwrap_or_else(|| json!({"engine":engine,"state":"pending","message":"Ready to set up on first use"}))
    }));
    for engine in ["pi", "omp", "claude-acp"] {
        let installed = data.join("engines").join(engine);
        let dir = if installed.join(executable(engine)).is_file() { Some(installed) } else {
            #[cfg(debug_assertions)] { source(engine, None).ok().filter(|p| p.join(executable(engine)).is_file()) }
            #[cfg(not(debug_assertions))] { None }
        };
        if let Some(dir) = dir {
            let package = match engine { "omp" => "@oh-my-pi/pi-coding-agent", "pi" => "@earendil-works/pi-coding-agent", _ => "@agentclientprotocol/claude-agent-acp" };
            let version = fs::read(dir.join("node_modules").join(package).join("package.json")).ok().and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok()).and_then(|v| v["version"].as_str().map(str::to_owned)).unwrap_or_default();
            if let Some(row) = status.as_array_mut().and_then(|rows| rows.iter_mut().find(|r| r["engine"] == engine)) {
                row["version"] = if version.is_empty() { Value::Null } else { json!(version) };
                if !matches!(row["state"].as_str(), Some("installing" | "preparing")) {
                    let bun = engine != "omp" || dir.join("node_modules/.bin/bun").is_file();
                    row["state"] = json!(if bun { "ready" } else { "error" });
                    row["message"] = json!(if bun { format!("Ready v{version}") } else { "Bun is missing; reinstall oh-my-pi".into() });
                }
            }
        }
    }
    if let Err(error) = check_node(&std::env::var_os("TAUCODE_NODE").unwrap_or_else(|| "node".into())).await {
        if let Some(claude) = status.as_array_mut().and_then(|r| r.iter_mut().find(|r| r["engine"] == "claude-acp")) {
            claude["state"] = json!("error");
            claude["message"] = json!(error);
        }
    }
    let settings = super::engine_registry::settings(Some(data));
    let mut command = Command::new(settings.codex_bin.as_deref().unwrap_or("codex"));
    command.arg("--version").stdin(Stdio::null()).kill_on_drop(true);
    let (state, message, version) = match tokio::time::timeout(Duration::from_secs(3), command.output()).await {
        Ok(Ok(output)) if output.status.success() => {
            let text=String::from_utf8_lossy(&output.stdout).trim().to_owned();
            let version=text.strip_prefix("codex-cli ").unwrap_or(&text).to_owned();
            ("ready", format!("Ready {text}"), if version.is_empty(){None}else{Some(version)})
        },
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => ("pending", "Install Codex to use this engine".into(), None),
        _ => ("error", "Codex version check failed".into(), None),
    };
    status.as_array_mut().unwrap().push(json!({"engine":"codex","state":state,"message":message,"version":version}));
    status
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
        return Err("Install system Node >=22.19 (and npm) to run pi, Claude Code, and browser tools".into());
    }
    Ok(())
}

async fn prepare_inner(
    data: &Path,
    engine: &str,
    resource: Option<&Path>,
) -> Result<PathBuf, String> {
    let lock_name = manifest(engine)?;
    let cancel = std::sync::Arc::new(tokio::sync::Notify::new());
    INSTALLS.get_or_init(Default::default).lock().unwrap().insert(engine.into(), cancel.clone());
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
        copy_tree(&src, &dir).map_err(|e| e.to_string())?;
        if !matches!(engine, "computer" | "work") {
            check_node(&std::env::var_os("TAUCODE_NODE").unwrap_or_else(|| "node".into())).await?;
        }
        if installed.as_ref() != Some(&locked) || !dir.join(executable(engine)).is_file() {
            if dir.join(".installed-lock").exists() {
                fs::remove_file(dir.join(".installed-lock")).map_err(|e| e.to_string())?;
            }
            let mut command = if matches!(engine, "computer" | "work") {
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
            let mut child = command.current_dir(&dir).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).kill_on_drop(true).spawn()
                .map_err(|_| if matches!(engine, "computer" | "work") { "Install uv to prepare Computer Use" } else { "Install system Node >=22.19 and npm to prepare this runtime" })?;
            let output = tokio::select! {
                output = child.wait() => output.map_err(|e| e.to_string())?,
                _ = cancel.notified() => { super::process_core::kill_child_process_tree(&mut child).await; return Err("Installation cancelled".into()); }
            };
            if !output.success() || !dir.join(executable(engine)).is_file() {
                return Err(format!("{engine} runtime installation failed. Check {} and network access, then retry setup in Settings.", if matches!(engine, "computer" | "work") { "uv" } else { "Node >=22.19 and npm" }));
            }
            fs::write(dir.join(".installed-lock"), locked).map_err(|e| e.to_string())?;
        }
        if engine == "omp" {
            let bun = dir.join("node_modules/.bin/bun");
            let version = Command::new(&bun).arg("--version").output().await.map_err(|_| "Bun is missing; reinstall oh-my-pi")?;
            let parts: Vec<u32> = String::from_utf8_lossy(&version.stdout).trim().split('.').filter_map(|part| part.parse().ok()).collect();
            if !version.status.success() || parts.len() != 3 || parts.as_slice() < [1, 3, 14].as_slice() { return Err("oh-my-pi requires Bun >= 1.3.14".into()); }
        }
        Ok(dir)
    }.await;
    INSTALLS.get_or_init(Default::default).lock().unwrap().remove(engine);
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
    fn omp_install_copies_bundled_auth_guard_without_dependencies() {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../engines/omp");
        let target = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let bundle: Value = serde_json::from_str(include_str!("../../tauri.conf.json")).unwrap();
        assert_eq!(bundle["bundle"]["resources"]["../engines/omp/no-accounts.mjs"], "engines/omp/no-accounts.mjs");
        copy_tree(&source, &target).unwrap();
        assert_eq!(fs::read(target.join("no-accounts.mjs")).unwrap(), fs::read(source.join("no-accounts.mjs")).unwrap());
        assert!(!target.join("node_modules").exists());
        fs::remove_dir_all(target).unwrap();
    }
    #[test]
    fn copy_tree_adds_updates_prunes_and_preserves_installed_dependencies() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let src = root.join("bundle");
        let dst = root.join("installed");
        fs::create_dir_all(src.join("taucode_cua/obsolete")).unwrap();
        fs::write(src.join("taucode_cua/actions.py"), "old actions").unwrap();
        fs::write(src.join("taucode_cua/removed.py"), "removed").unwrap();
        fs::write(src.join("taucode_cua/obsolete/module.py"), "removed").unwrap();
        copy_tree(&src, &dst).unwrap();
        for dir in [
            "node_modules",
            ".venv",
            "taucode_cua/__pycache__",
            "tests",
            "evals",
        ] {
            fs::create_dir_all(dst.join(dir)).unwrap();
            fs::write(dst.join(dir).join("local"), "local").unwrap();
        }
        fs::write(dst.join(".installed-lock"), "locked").unwrap();
        fs::write(src.join("taucode_cua/actions.py"), "import elements").unwrap();
        fs::write(src.join("taucode_cua/elements.py"), "new module").unwrap();
        fs::remove_file(src.join("taucode_cua/removed.py")).unwrap();
        fs::remove_dir_all(src.join("taucode_cua/obsolete")).unwrap();
        for _ in 0..2 {
            copy_tree(&src, &dst).unwrap();
            assert_eq!(
                fs::read_to_string(dst.join("taucode_cua/actions.py")).unwrap(),
                "import elements"
            );
            assert_eq!(
                fs::read_to_string(dst.join("taucode_cua/elements.py")).unwrap(),
                "new module"
            );
            for path in [
                "taucode_cua/removed.py",
                "taucode_cua/obsolete",
                "taucode_cua/__pycache__",
                "tests",
                "evals",
            ] {
                assert!(!dst.join(path).exists(), "Stale resource: {path}");
            }
            for path in ["node_modules/local", ".venv/local"] {
                assert_eq!(fs::read_to_string(dst.join(path)).unwrap(), "local");
            }
            assert_eq!(
                fs::read_to_string(dst.join(".installed-lock")).unwrap(),
                "locked"
            );
        }
        assert!(copy_tree(&root.join("missing"), &dst).is_err());
        assert!(dst.join("taucode_cua/elements.py").is_file());
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn copy_tree_replaces_types_without_following_destination_symlinks() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let src = root.join("bundle");
        let dst = root.join("installed");
        fs::create_dir_all(src.join("directory")).unwrap();
        fs::create_dir_all(dst.join("file")).unwrap();
        fs::write(src.join("file"), "new file").unwrap();
        fs::write(src.join("directory/new"), "new child").unwrap();
        fs::write(src.join("link"), "source").unwrap();
        fs::write(dst.join("directory"), "old file").unwrap();
        let outside = root.join("outside");
        fs::write(&outside, "untouched").unwrap();
        std::os::unix::fs::symlink(&outside, dst.join("link")).unwrap();
        std::os::unix::fs::symlink(&outside, dst.join("stale-link")).unwrap();
        copy_tree(&src, &dst).unwrap();
        assert_eq!(fs::read_to_string(&outside).unwrap(), "untouched");
        assert_eq!(fs::read_to_string(dst.join("file")).unwrap(), "new file");
        assert_eq!(
            fs::read_to_string(dst.join("directory/new")).unwrap(),
            "new child"
        );
        assert_eq!(fs::read_to_string(dst.join("link")).unwrap(), "source");
        assert!(fs::symlink_metadata(dst.join("stale-link")).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn resolves_packaged_daemon_and_debug_paths_without_release_fallback() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        for engine in ["pi", "browser", "computer", "claude-acp", "work", "omp"] {
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
        assert_eq!(status(&data).await[2]["state"], "ready");
        fs::remove_dir_all(root).unwrap();
    }
}

pub(crate) async fn prepare(data:&Path,engine:&str,resource:Option<&Path>)->Result<PathBuf,String> {
    let result=prepare_inner(data,engine,resource).await;
    if result.is_err() {taucode_diag_mcp::writer::record(||{let mut e=taucode_diag_mcp::core::event("error","engine","engine.install_failed","Runtime install failed");e.context=serde_json::json!({"engine":engine});e});}
    result
}
