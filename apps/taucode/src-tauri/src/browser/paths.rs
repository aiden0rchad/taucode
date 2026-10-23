use crate::shared::local_socket::{self, SOCKET_LIMIT};
use std::{
    io,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

pub(super) fn candidates(data: &Path, _temp: &Path) -> Vec<PathBuf> {
    vec![local_socket::path(data, "browser")]
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
pub(super) async fn bind(data: &Path) -> Result<tokio::net::UnixListener, String> {
    let path = candidates(data, Path::new("/tmp")).remove(0);
    let listener = local_socket::bind(&path).await.map_err(|e| e.to_string())?;
    if let Err(e) = publish(data, &path) {
        let _ = local_socket::remove_socket(&path);
        return Err(e.to_string());
    }
    Ok(listener)
}
pub(super) async fn connect(data: &Path) -> io::Result<tokio::net::UnixStream> {
    let path = recorded(data).unwrap_or_else(|| local_socket::path(data, "browser"));
    local_socket::connect(&path).await
}
pub(super) fn cleanup(data: &Path) {
    if let Some(path) = recorded(data) {
        let _ = local_socket::remove_socket(&path);
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
            short.join("run/browser.sock")
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
        assert_eq!(selected, candidates(&data, Path::new("/tmp/launcher"))[0]);
        std::fs::remove_dir_all(data.parent().unwrap()).unwrap();
    }
}
