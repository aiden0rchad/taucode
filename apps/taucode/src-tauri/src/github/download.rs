use super::{app_update_check, CURRENT_VERSION, REPO};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use tauri::{ipc::Channel, AppHandle, Manager};
use tokio::io::AsyncWriteExt;

const MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;

pub(super) fn release_asset(release: &Value, os: &str, arch: &str) -> Option<Value> {
    let suffix = match os {
        "macos" => ".dmg",
        "windows" => ".exe",
        "linux" => ".appimage",
        _ => return None,
    };
    release["assets"]
        .as_array()?
        .iter()
        .find(|asset| {
            let name = asset["name"]
                .as_str()
                .unwrap_or_default()
                .to_ascii_lowercase();
            let arm = name.contains("aarch64") || name.contains("arm64");
            let intel = name.contains("x86_64") || name.contains("x64") || name.contains("amd64");
            let compatible = name.contains("universal")
                || match arch {
                    "aarch64" => !intel,
                    "x86_64" => !arm,
                    _ => false,
                };
            name.ends_with(suffix)
                && compatible
                && asset["browser_download_url"].as_str().is_some_and(|url| {
                    url.starts_with(&format!("https://github.com/{REPO}/releases/download/"))
                })
        })
        .cloned()
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DownloadProgress {
    downloaded: u64,
    total: u64,
}

// The destination is created exclusively for this transfer. Failed/partial downloads
// never acquire an installer extension, and existing user downloads are never replaced.
async fn save_download(
    client: &reqwest::Client,
    asset: &Value,
    folder: &Path,
    progress: impl Fn(DownloadProgress),
) -> Result<PathBuf, String> {
    let name = asset["name"].as_str().ok_or("Missing installer name")?;
    if name.is_empty()
        || name.len() > 200
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        || name.starts_with('.')
    {
        return Err("Invalid installer name".into());
    }
    let size = asset["size"]
        .as_u64()
        .filter(|s| *s > 0 && *s <= MAX_BYTES)
        .ok_or("Invalid installer size")?;
    let digest = match asset["digest"].as_str() {
        Some(value) => Some(
            value
                .strip_prefix("sha256:")
                .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
                .ok_or("Invalid installer checksum")?,
        ),
        None => None,
    };
    let directory = folder.join(format!("taucode-update-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir(&directory)
        .await
        .map_err(|e| e.to_string())?;
    let partial = directory.join("download.partial");
    let target = directory.join(name);
    let result = async {
        let mut response = client
            .get(
                asset["browser_download_url"]
                    .as_str()
                    .ok_or("Missing installer URL")?,
            )
            .send()
            .await
            .map_err(|e| format!("Download failed: {e}"))?
            .error_for_status()
            .map_err(|e| format!("Download failed: {e}"))?;
        if response
            .content_length()
            .is_some_and(|length| length != size)
        {
            return Err("Installer size does not match the release".to_string());
        }
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&partial)
            .await
            .map_err(|e| e.to_string())?;
        let mut hash = Sha256::new();
        let mut downloaded = 0;
        let mut last_report = std::time::Instant::now();
        progress(DownloadProgress {
            downloaded,
            total: size,
        });
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| format!("Download interrupted: {e}"))?
        {
            downloaded += chunk.len() as u64;
            if downloaded > size {
                return Err("Installer exceeds the release size".into());
            }
            file.write_all(&chunk)
                .await
                .map_err(|e| format!("Couldn't save update: {e}"))?;
            hash.update(&chunk);
            if last_report.elapsed() >= Duration::from_millis(100) {
                progress(DownloadProgress {
                    downloaded,
                    total: size,
                });
                last_report = std::time::Instant::now();
            }
        }
        if downloaded != size {
            return Err("Installer download is incomplete".into());
        }
        if digest.is_some_and(|expected| {
            !format!("{:x}", hash.finalize()).eq_ignore_ascii_case(expected)
        }) {
            return Err("Installer checksum does not match the release".into());
        }
        file.sync_all().await.map_err(|e| e.to_string())?;
        drop(file);
        // Preserve macOS's normal checks for software downloaded from the Internet.
        #[cfg(target_os = "macos")]
        {
            let status = tokio::process::Command::new("/usr/bin/xattr")
                .args(["-w", "com.apple.quarantine", "0081;00000000;taucode;"])
                .arg(&partial)
                .status()
                .await
                .map_err(|e| e.to_string())?;
            if !status.success() {
                return Err("Couldn't mark the installer as an Internet download".into());
            }
        }
        tokio::fs::rename(&partial, &target)
            .await
            .map_err(|e| e.to_string())?;
        progress(DownloadProgress {
            downloaded,
            total: size,
        });
        Ok(target)
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(&partial).await;
        let _ = tokio::fs::remove_dir(&directory).await;
    }
    result
}

#[tauri::command]
pub(crate) async fn app_update_download(
    app: AppHandle,
    version: String,
    on_progress: Channel<DownloadProgress>,
) -> Result<String, String> {
    // Resolve the asset server-side; never download a URL or write a path supplied by the UI.
    let release = app_update_check().await?;
    if release.status != "available" || release.latest_version.as_deref() != Some(&version) {
        return Err("The release changed or is unavailable. Check for updates again.".into());
    }
    let asset = release
        .asset
        .ok_or("No compatible installer is published for this computer.")?;
    let client = reqwest::Client::builder()
        .https_only(true)
        .timeout(Duration::from_secs(1800))
        .connect_timeout(Duration::from_secs(15))
        .user_agent(format!("taucode/{CURRENT_VERSION}"))
        .build()
        .map_err(|e| e.to_string())?;
    let folder = app.path().download_dir().map_err(|e| e.to_string())?;
    let path = save_download(&client, &asset, &folder, |event| {
        let _ = on_progress.send(event);
    })
    .await?;
    Ok(path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn selects_only_compatible_installers_from_our_releases() {
        let asset = |name: &str| json!({"name": name, "browser_download_url": format!("https://github.com/{REPO}/releases/download/v1/{name}")});
        let release = json!({"assets": [asset("taucode_x64.dmg"), asset("taucode_aarch64.dmg"), asset("taucode_x64-setup.exe")]});
        assert_eq!(
            release_asset(&release, "macos", "aarch64").unwrap()["name"],
            "taucode_aarch64.dmg"
        );
        assert_eq!(
            release_asset(&release, "macos", "x86_64").unwrap()["name"],
            "taucode_x64.dmg"
        );
        assert!(release_asset(&release, "windows", "aarch64").is_none());
        assert!(release_asset(&json!({"assets": [{"name":"taucode.dmg","browser_download_url":"https://evil.test/update.dmg"}]}), "macos", "aarch64").is_none());
    }
    #[tokio::test]
    async fn streams_verifies_and_cleans_failed_downloads() {
        use tokio::{io::AsyncReadExt, net::TcpListener};
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/installer", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            for _ in 0..3 {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = [0; 2048];
                socket.read(&mut request).await.unwrap();
                socket
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\ntest",
                    )
                    .await
                    .unwrap();
            }
        });
        let folder = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir(&folder).unwrap();
        let mut asset = json!({"name":"taucode.dmg","size":4,"digest":format!("sha256:{:x}", Sha256::digest(b"test")),"browser_download_url":url});
        let client = reqwest::Client::new();
        let path = save_download(&client, &asset, &folder, |_| {})
            .await
            .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"test");
        asset["digest"] = json!(format!("sha256:{}", "0".repeat(64)));
        assert!(save_download(&client, &asset, &folder, |_| {})
            .await
            .unwrap_err()
            .contains("checksum"));
        asset["size"] = json!(5);
        assert!(save_download(&client, &asset, &folder, |_| {})
            .await
            .is_err());
        for name in ["../escape.dmg", "/tmp/escape.dmg", "a\\b.dmg"] {
            asset["name"] = json!(name);
            assert!(save_download(&client, &asset, &folder, |_| {})
                .await
                .is_err());
        }
        assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 1);
        server.await.unwrap();
        std::fs::remove_dir_all(folder).unwrap();
    }
}
