fn main() {
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
