#[path = "src/shared/engine_files.rs"]
mod engine_files;

fn main() {
    // Tauri directory mappings have no excludes. Stage clean trees so local
    // Python caches, test fixtures and dependencies can never enter the bundle.
    for dir in ["computer/taucode_cua", "pi/extensions", "work/skills"] {
        let source = std::path::Path::new("../engines").join(dir);
        let target = std::path::Path::new("target/engine-resources").join(dir);
        println!("cargo:rerun-if-changed={}", source.display());
        if target.exists() {
            std::fs::remove_dir_all(&target).expect("clean staged engine resources");
        }
        engine_files::copy_tree(&source, &target).expect("stage engine resources");
    }
    // The CLI bundles these after beforeBundleCommand builds the real helpers.
    // tauri-build must not copy stale sidecars over Cargo's own output binaries.
    let original = std::env::var_os("TAURI_CONFIG");
    let mut config: serde_json::Value = original.as_ref()
        .map(|v| serde_json::from_str(&v.to_string_lossy()).expect("TAURI_CONFIG JSON"))
        .unwrap_or_else(|| serde_json::json!({}));
    config["bundle"]["externalBin"] = serde_json::json!([]);
    // Resource staging belongs to the CLI bundler, not Cargo's target output.
    config["bundle"]["resources"] = serde_json::json!([]);
    std::env::set_var("TAURI_CONFIG", config.to_string());
    tauri_build::build();
    match original {
        Some(value) => std::env::set_var("TAURI_CONFIG", value),
        None => std::env::remove_var("TAURI_CONFIG"),
    }

    // ScreenCaptureKit's Swift bridge needs the active toolchain's compatibility libraries.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
        if let Ok(output) = std::process::Command::new("xcrun")
            .args(["--find", "swiftc"])
            .output()
        {
            let compiler = std::path::PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
            if let Some(usr) = compiler.parent().and_then(std::path::Path::parent) {
                let libraries = usr.join("lib/swift/macosx");
                if libraries.is_dir() {
                    println!("cargo:rustc-link-search=native={}", libraries.display());
                }
            }
        }
    }

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("ios") {
        println!("cargo:rustc-link-lib=z");
        println!("cargo:rustc-link-lib=iconv");
    }
}
