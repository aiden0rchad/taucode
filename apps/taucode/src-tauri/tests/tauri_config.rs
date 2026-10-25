use std::fs;
use std::path::PathBuf;

use serde_json::Value;

#[path = "../src/shared/engine_files.rs"]
mod engine_files;

fn engine_files_in(dir: &std::path::Path, exclude: bool) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        if exclude && engine_files::excluded(&entry.file_name()) {
            continue;
        }
        let kind = entry.file_type().unwrap();
        assert!(
            !kind.is_symlink(),
            "Engine resources must not contain symlinks: {:?}",
            entry.path()
        );
        if kind.is_dir() {
            files.extend(engine_files_in(&entry.path(), exclude));
        } else {
            assert!(kind.is_file());
            files.push(entry.path());
        }
    }
    files
}

#[test]
fn every_engine_file_is_bundled_at_its_runtime_path() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let engines = root.join("../engines").canonicalize().unwrap();
    let config: Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    let resources = config["bundle"]["resources"].as_object().unwrap();
    let mappings = resources
        .iter()
        .filter(|(_, target)| target.as_str().is_some_and(|p| p.starts_with("engines/")))
        .map(|(source, target)| {
            (
                root.join(source).to_string_lossy().into_owned(),
                target.as_str().unwrap().to_owned(),
            )
        })
        .collect();
    let mut covered = std::collections::HashSet::new();
    // Exercise Tauri's actual directory expansion and destination layout.
    for resource in tauri::utils::resources::ResourcePaths::from_map(&mappings, true).iter() {
        let resource = resource.unwrap();
        let file = resource.path();
        let destination = resource.target();
        assert!(
            !destination
                .components()
                .any(|c| engine_files::excluded(c.as_os_str())),
            "Excluded resource: {destination:?}"
        );
        let original = file
            .strip_prefix(root.join("target/engine-resources"))
            .map(|relative| engines.join(relative))
            .unwrap_or_else(|_| file.to_path_buf())
            .canonicalize()
            .unwrap();
        assert_eq!(
            destination,
            PathBuf::from("engines").join(original.strip_prefix(&engines).unwrap())
        );
        assert_eq!(
            fs::read(file).unwrap(),
            fs::read(&original).unwrap(),
            "Stale staged resource: {file:?}"
        );
        assert!(
            covered.insert(original),
            "Duplicate resource destination: {destination:?}"
        );
    }
    for engine in ["pi", "browser", "computer"] {
        for file in engine_files_in(&engines.join(engine), true) {
            assert!(
                covered.contains(&file),
                "Engine file missing from bundle.resources: {}",
                file.display()
            );
        }
    }
    for dir in ["computer/taucode_cua", "pi/extensions"] {
        assert_eq!(
            resources[&format!("target/engine-resources/{dir}/")],
            format!("engines/{dir}/")
        );
    }
}

#[test]
fn packaging_hooks_bundle_runtimes_and_all_helpers() {
    let config: Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    for runtime in ["node", "uv"] {
        assert_eq!(config["bundle"]["resources"][format!("target/engine-resources/{runtime}/")], format!("{runtime}/"));
    }
    for helper in ["codex_monitor_daemon", "codex_monitor_daemonctl", "taucode-diag-mcp"] {
        assert!(config["bundle"]["externalBin"].as_array().unwrap().contains(&Value::String(format!("binaries/{helper}"))));
    }
    assert!(config["build"]["beforeBuildCommand"].as_str().unwrap().contains("bundle-node.mjs"));
    assert!(config["build"]["beforeBundleCommand"].as_str().unwrap().contains("bundle-helpers.mjs"));
}

#[test]
fn staging_excludes_local_dependencies_and_caches() {
    let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
    let source = root.join("source");
    let target = root.join("staged");
    for dir in [
        "node_modules",
        ".venv",
        "tests",
        "__pycache__",
        "evals",
        ".pytest_cache",
    ] {
        fs::create_dir_all(source.join("nested").join(dir)).unwrap();
        fs::write(source.join("nested").join(dir).join("excluded"), "excluded").unwrap();
    }
    fs::write(source.join("nested/elements.py"), "new module").unwrap();
    engine_files::copy_tree(&source, &target).unwrap();
    assert_eq!(
        engine_files_in(&target, false),
        vec![target.join("nested/elements.py")]
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn macos_private_api_feature_matches_config() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let config_path = manifest_dir.join("tauri.conf.json");
    let config_contents = fs::read_to_string(&config_path)
        .unwrap_or_else(|error| panic!("Failed to read {config_path:?}: {error}"));
    let config: Value = serde_json::from_str(&config_contents)
        .unwrap_or_else(|error| panic!("Failed to parse tauri.conf.json: {error}"));
    let macos_private_api = config
        .get("app")
        .and_then(|app| app.get("macOSPrivateApi"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false);

    if macos_private_api {
        let cargo_path = manifest_dir.join("Cargo.toml");
        let cargo_contents = fs::read_to_string(&cargo_path)
            .unwrap_or_else(|error| panic!("Failed to read {cargo_path:?}: {error}"));
        let mut in_dependencies = false;
        let mut has_feature = false;

        for line in cargo_contents.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') {
                in_dependencies = trimmed == "[dependencies]";
                continue;
            }
            if !in_dependencies {
                continue;
            }
            if trimmed.starts_with("tauri") && trimmed.contains("macos-private-api") {
                has_feature = true;
                break;
            }
        }

        assert!(
            has_feature,
            "Cargo.toml [dependencies] must enable macos-private-api when app.macOSPrivateApi is true"
        );
    }
}

#[test]
fn app_updater_is_disabled() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let config: Value =
        serde_json::from_str(&fs::read_to_string(root.join("tauri.conf.json")).unwrap()).unwrap();
    assert_eq!(config["bundle"]["createUpdaterArtifacts"], false);
    assert!(config["plugins"]["updater"]["pubkey"].is_null());
    assert!(config["plugins"]["updater"]["endpoints"].is_null());
    for name in [
        "tauri.windows.conf.json",
        "tauri.linux.conf.json",
        "tauri.ios.conf.json",
    ] {
        let platform: Value =
            serde_json::from_str(&fs::read_to_string(root.join(name)).unwrap()).unwrap();
        assert_ne!(platform["bundle"]["createUpdaterArtifacts"], true);
        assert!(platform["plugins"]["updater"].is_null());
    }
    let lib = fs::read_to_string(root.join("src/lib.rs")).unwrap();
    assert!(!lib.contains("tauri_plugin_updater::"));
    let capabilities = fs::read_to_string(root.join("capabilities/default.json")).unwrap();
    assert!(!capabilities.contains("updater:"));
    let menu = fs::read_to_string(root.join("src/menu.rs")).unwrap();
    assert!(!menu.contains("check_for_updates"));
}
