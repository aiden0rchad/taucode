//! Same-user transport for browser and computer control.
use sha2::{Digest, Sha256};
use std::{
    fs::{self, Metadata},
    io,
    os::unix::{
        ffi::OsStrExt,
        fs::{DirBuilderExt, FileTypeExt, MetadataExt},
    },
    path::{Path, PathBuf},
    sync::Mutex,
};
use tokio::net::{UnixListener, UnixStream};

pub(crate) const SOCKET_LIMIT: usize = 100;
static UMASK_LOCK: Mutex<()> = Mutex::new(());

pub(crate) fn euid() -> u32 {
    unsafe { libc::geteuid() }
}

fn denied(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, message)
}

// Serialize process-wide mask changes, never across an await. Temporarily deny
// group/other access and restore the original mask even on unwinding.
fn private_create<T>(mask: libc::mode_t, create: impl FnOnce() -> io::Result<T>) -> io::Result<T> {
    let _lock = UMASK_LOCK
        .lock()
        .map_err(|_| denied("Socket creation lock poisoned"))?;
    struct Mask(libc::mode_t);
    impl Drop for Mask {
        fn drop(&mut self) {
            unsafe {
                libc::umask(self.0);
            }
        }
    }
    let _mask = Mask(unsafe { libc::umask(mask) });
    create()
}

fn directory_metadata(meta: &Metadata, uid: u32) -> io::Result<()> {
    if !meta.is_dir() || meta.uid() != uid || meta.mode() & 0o7777 != 0o700 {
        return Err(denied("Control socket directory must be a real directory owned by the current user with mode 0700"));
    }
    Ok(())
}

pub(crate) fn validate_directory(path: &Path) -> io::Result<()> {
    directory_metadata(&fs::symlink_metadata(path)?, euid())
        .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", path.display())))
}

pub(crate) fn ensure_directory(path: &Path) -> io::Result<()> {
    match private_create(0o077, || fs::DirBuilder::new().mode(0o700).create(path)) {
        Ok(()) => (),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(e),
    }
    validate_directory(path)
}

pub(crate) fn path(data: &Path, service: &str) -> PathBuf {
    let data = data.canonicalize().unwrap_or_else(|_| data.to_path_buf());
    let preferred = data.join("run").join(format!("{service}.sock"));
    if preferred.is_absolute() && preferred.as_os_str().as_bytes().len() <= SOCKET_LIMIT {
        return preferred;
    }
    let hash = format!("{:x}", Sha256::digest(data.as_os_str().as_bytes()));
    PathBuf::from(format!(
        "/tmp/taucode-{}/{}-{}.sock",
        euid(),
        service,
        &hash[..16]
    ))
}

fn socket_owner(is_socket: bool, owner: u32, uid: u32) -> io::Result<()> {
    if !is_socket || owner != uid {
        return Err(denied(
            "Control socket must be a socket owned by the current user",
        ));
    }
    Ok(())
}

pub(crate) fn validate_socket(path: &Path) -> io::Result<()> {
    validate_directory(
        path.parent()
            .ok_or_else(|| denied("Socket has no parent directory"))?,
    )?;
    let meta = fs::symlink_metadata(path)?;
    socket_owner(meta.file_type().is_socket(), meta.uid(), euid())
}

fn peer_uid(uid: u32, expected: u32) -> io::Result<()> {
    if uid != expected {
        return Err(denied("Control socket peer is not the current user"));
    }
    Ok(())
}

pub(crate) fn validate_peer(stream: &UnixStream) -> io::Result<()> {
    peer_uid(stream.peer_cred()?.uid(), euid())
}

pub(crate) async fn connect(path: &Path) -> io::Result<UnixStream> {
    validate_socket(path)?;
    let stream = UnixStream::connect(path).await?;
    validate_peer(&stream)?;
    Ok(stream)
}

pub(crate) fn remove_socket(path: &Path) -> io::Result<()> {
    match validate_socket(path) {
        Ok(()) => fs::remove_file(path),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

pub(crate) async fn bind(path: &Path) -> io::Result<UnixListener> {
    ensure_directory(
        path.parent()
            .ok_or_else(|| denied("Socket has no parent directory"))?,
    )?;
    match connect(path).await {
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                "Control socket already has an owner",
            ))
        }
        Err(e)
            if matches!(
                e.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
            ) =>
        {
            ()
        }
        Err(e) => return Err(e),
    }
    remove_socket(path)?;
    private_create(0o177, || UnixListener::bind(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{symlink, PermissionsExt};

    #[test]
    fn directory_and_client_validation_matrix() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        fs::create_dir(&root).unwrap();
        let dir = root.join("run");
        assert_eq!(
            validate_directory(&dir).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        ensure_directory(&dir).unwrap();
        let meta = fs::symlink_metadata(&dir).unwrap();
        assert!(directory_metadata(&meta, euid()).is_ok());
        assert!(directory_metadata(&meta, euid().wrapping_add(1)).is_err());
        for mode in [0o755, 0o777, 0o750, 0o600, 0o1700] {
            fs::set_permissions(&dir, fs::Permissions::from_mode(mode)).unwrap();
            assert!(ensure_directory(&dir).is_err());
            assert_eq!(
                validate_socket(&dir.join("control.sock"))
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::PermissionDenied
            );
        }
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
        let link = root.join("link");
        symlink(&dir, &link).unwrap();
        assert!(ensure_directory(&link).is_err());
        assert_eq!(
            validate_socket(&link.join("control.sock"))
                .unwrap_err()
                .kind(),
            io::ErrorKind::PermissionDenied
        );
        let file = dir.join("file");
        fs::write(&file, "").unwrap();
        assert!(ensure_directory(&file).is_err());
        assert!(validate_socket(&file).is_err());
        symlink(&file, dir.join("socket-link")).unwrap();
        assert!(validate_socket(&dir.join("socket-link")).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn socket_ownership_and_peer_decisions_without_binding() {
        assert!(socket_owner(true, 42, 42).is_ok());
        assert!(socket_owner(true, 43, 42).is_err());
        assert!(socket_owner(false, 42, 42).is_err());
        assert!(peer_uid(42, 42).is_ok());
        assert!(peer_uid(43, 42).is_err());
        assert!(peer_uid(0, 42).is_err());
    }

    #[tokio::test]
    async fn unsafe_endpoints_fail_before_connect_or_bind() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        ensure_directory(&root).unwrap();
        let socket = root.join("control.sock");
        fs::write(&socket, "keep").unwrap();
        assert_eq!(
            connect(&socket).await.unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        assert_eq!(
            bind(&socket).await.unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        assert_eq!(fs::read_to_string(&socket).unwrap(), "keep");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            bind(&socket).await.unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        assert_eq!(fs::symlink_metadata(&root).unwrap().mode() & 0o7777, 0o755);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn short_and_long_paths() {
        for service in ["browser", "computer"] {
            let suffix_len = format!("/run/{service}.sock").len();
            let boundary = PathBuf::from(format!("/{}", "a".repeat(SOCKET_LIMIT - suffix_len - 1)));
            assert!(path(&boundary, service).starts_with(&boundary));
            let over = PathBuf::from(format!("{}b", boundary.display()));
            assert!(path(&over, service).starts_with(format!("/tmp/taucode-{}", euid())));
            assert_eq!(
                path(Path::new("/short/data"), service),
                Path::new("/short/data/run").join(format!("{service}.sock"))
            );
            let long = PathBuf::from(format!("/data/{}", "é".repeat(100)));
            let selected = path(&long, service);
            assert!(selected.starts_with(format!("/tmp/taucode-{}", euid())));
            assert!(selected.as_os_str().as_bytes().len() <= SOCKET_LIMIT);
            assert_eq!(selected, path(&long, service));
            assert_ne!(selected, path(&long.join("other"), service));
        }
        assert_ne!(
            path(Path::new("/data"), "computer"),
            path(Path::new("/data"), "browser")
        );
    }
}
