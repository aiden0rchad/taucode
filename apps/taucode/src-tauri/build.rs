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
    tauri_build::build();

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
