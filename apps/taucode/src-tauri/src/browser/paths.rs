use sha2::{Digest, Sha256};
use std::{
    io,
    os::unix::{
        ffi::OsStrExt,
        fs::{FileTypeExt, MetadataExt, PermissionsExt},
    },
    path::{Path, PathBuf},
};

// Leave room for sockaddr_un's terminating NUL on both macOS and Linux.
const SOCKET_LIMIT: usize = 100;

pub(super) fn candidates(data: &Path, temp: &Path) -> Vec<PathBuf> {
    let data = data.canonicalize().unwrap_or_else(|_| data.to_path_buf());
    let hash = format!("{:x}", Sha256::digest(data.as_os_str().as_bytes()));
    let name = format!("taucode-{}.sock", &hash[..16]);
    let mut paths = vec![
        data.join("browser/control.sock"),
        temp.join(&name),
        Path::new("/tmp").join(name),
    ];
    paths.retain(|p| p.as_os_str().as_bytes().len() <= SOCKET_LIMIT);
    paths.dedup();
    paths
}

fn record(data: &Path) -> PathBuf {
    data.join("browser/socket-path")
}

pub(super) fn recorded(data: &Path) -> Option<PathBuf> {
    let path: PathBuf = serde_json::from_slice(&std::fs::read(record(data)).ok()?).ok()?;
    (path.is_absolute() && path.as_os_str().as_bytes().len() <= SOCKET_LIMIT).then_some(path)
}

fn publish(data: &Path, path: &Path) -> io::Result<()> {
    let temporary = record(data).with_extension("tmp");
    std::fs::write(&temporary, serde_json::to_vec(path)?)?;
    std::fs::rename(temporary, record(data))
}

fn remove_socket(path: &Path) -> io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_socket() && meta.uid() == unsafe { libc::geteuid() } => {
            std::fs::remove_file(path)
        }
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Browser socket is not owned by this user",
        )),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

pub(super) async fn bind(data: &Path) -> Result<tokio::net::UnixListener, String> {
    let mut error = String::new();
    for path in candidates(data, &std::env::temp_dir()) {
        match tokio::net::UnixStream::connect(&path).await {
            Ok(_) => return Err("Browser socket already has an owner".into()),
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
                ) =>
            {
                ()
            }
            Err(e) => {
                error = e.to_string();
                continue;
            }
        }
        let bound = remove_socket(&path).and_then(|()| tokio::net::UnixListener::bind(&path));
        match bound {
            Ok(listener) => {
                if let Err(e) =
                    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                        .and_then(|()| publish(data, &path))
                {
                    let _ = remove_socket(&path);
                    return Err(e.to_string());
                }
                return Ok(listener);
            }
            Err(e) => error = e.to_string(),
        }
    }
    Err(error)
}

pub(super) async fn connect(data: &Path) -> io::Result<tokio::net::UnixStream> {
    let paths = recorded(data)
        .map(|p| vec![p])
        .unwrap_or_else(|| candidates(data, &std::env::temp_dir()));
    let mut error = io::Error::from(io::ErrorKind::NotFound);
    for path in paths {
        match tokio::net::UnixStream::connect(path).await {
            Ok(socket) => return Ok(socket),
            Err(e) => error = e,
        }
    }
    Err(error)
}

pub(super) fn cleanup(data: &Path) {
    if let Some(path) = recorded(data) {
        let _ = remove_socket(&path);
    }
    let _ = std::fs::remove_file(record(data));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn short_and_long_socket_paths_are_stable_and_bounded() {
        let short = Path::new("/short/data");
        assert_eq!(
            candidates(short, Path::new("/tmp"))[0],
            short.join("browser/control.sock")
        );
        let long = PathBuf::from(format!("/data/{}", "é".repeat(100)));
        let paths = candidates(&long, Path::new("/tmp"));
        assert_eq!(paths.len(), 1);
        assert!(paths[0].starts_with("/tmp"));
        assert_eq!(paths, candidates(&long, Path::new("/tmp")));
        assert_ne!(paths, candidates(&long.join("other"), Path::new("/tmp")));
        assert_eq!(paths, candidates(&long, &long)); // Even TMPDIR can exceed SUN_LEN.
        assert!(paths[0].as_os_str().as_bytes().len() <= SOCKET_LIMIT);
    }
    #[test]
    fn published_path_is_authoritative_for_different_temp_directories() {
        let data = std::env::temp_dir()
            .join(format!("browser-path-{}", uuid::Uuid::new_v4()))
            .join("long".repeat(40));
        std::fs::create_dir_all(data.join("browser")).unwrap();
        let selected = candidates(&data, Path::new("/tmp/host"))[0].clone();
        publish(&data, &selected).unwrap();
        assert_eq!(recorded(&data), Some(selected.clone()));
        assert_ne!(selected, candidates(&data, Path::new("/tmp/launcher"))[0]);
        std::fs::remove_dir_all(data.parent().unwrap()).unwrap();
    }
}
