use super::repository::Checkpoint;
use super::*;
use std::{fs, process::Command};
struct Fixture {
    data: PathBuf,
    root: PathBuf,
    repo: Repository,
}
impl Fixture {
    fn new() -> Self {
        let data =
            std::env::temp_dir().join(format!("taucode-checkpoint-{}", uuid::Uuid::new_v4()));
        let root = data.join("workspace");
        fs::create_dir_all(&root).unwrap();
        let repo = Repository::new(&data, "workspace", &root).unwrap();
        Self { data, root, repo }
    }
    fn write(&self, path: &str, text: &str) {
        let path = self.root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn snapshot(&self) -> Checkpoint {
        self.repo.snapshot(json!({"threadId":"thread","turnId":"turn","userItemId":"user","kind":"before","timestamp":"2026-10-23T00:00:00Z","engine":"pi"})).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.data);
    }
}
fn contents(path: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    let mut out = std::collections::BTreeMap::new();
    for entry in fs::read_dir(path).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(contents(&path));
        } else {
            out.insert(path.clone(), fs::read(path).unwrap());
        }
    }
    out
}
#[test]
fn shadow_snapshot_restore_and_undo_preserve_user_git_and_excludes() {
    let f = Fixture::new();
    assert!(Command::new(crate::utils::resolve_git_binary().unwrap())
        .args(["init", "--quiet"])
        .current_dir(&f.root)
        .status()
        .unwrap()
        .success());
    f.write("modified", "before");
    f.write("deleted", "before");
    f.write("unchecked", "before");
    f.write(".gitignore", "ignored\n");
    f.write(".taucodeignore", "private\n");
    f.write(".codegraph/db", "never");
    f.write("ignored", "never");
    f.write("private", "never");
    f.write(".git/info/exclude", "local-ignore\n");
    f.write("local-ignore", "never");
    assert!(Command::new(crate::utils::resolve_git_binary().unwrap())
        .args(["add", "modified"])
        .current_dir(&f.root)
        .status()
        .unwrap()
        .success());
    let git_before = contents(&f.root.join(".git"));
    let before = f.snapshot();
    assert!(before.commit.is_some(), "{:?}", before);
    assert_eq!(contents(&f.root.join(".git")), git_before);
    f.write("modified", "after");
    f.write("added", "new");
    f.write("unchecked", "after");
    fs::remove_file(f.root.join("deleted")).unwrap();
    let changes = f.repo.diff("thread", &before.id).unwrap();
    assert_eq!(changes.len(), 4);
    assert_eq!(
        changes.iter().find(|c| c.path == "added").unwrap().status,
        "added since"
    );
    let paths = vec!["modified".into(), "added".into(), "deleted".into()];
    let safety = f
        .repo
        .restore(
            "thread",
            &before.id,
            &paths,
            safety_metadata("thread", "pi"),
        )
        .unwrap();
    assert_eq!(
        fs::read_to_string(f.root.join("modified")).unwrap(),
        "before"
    );
    assert_eq!(
        fs::read_to_string(f.root.join("deleted")).unwrap(),
        "before"
    );
    assert!(!f.root.join("added").exists());
    assert_eq!(
        fs::read_to_string(f.root.join("unchecked")).unwrap(),
        "after"
    );
    assert_eq!(fs::read_to_string(f.root.join("ignored")).unwrap(), "never");
    assert_eq!(safety.metadata["kind"], "pre-rewind");
    f.repo
        .restore(
            "thread",
            &safety.id,
            &paths,
            safety_metadata("thread", "pi"),
        )
        .unwrap();
    assert_eq!(
        fs::read_to_string(f.root.join("modified")).unwrap(),
        "after"
    );
    assert_eq!(fs::read_to_string(f.root.join("added")).unwrap(), "new");
    assert!(!f.root.join("deleted").exists());
    assert_eq!(contents(&f.root.join(".git")), git_before);
}
#[test]
fn size_limits_and_new_content_accounting() {
    let f = Fixture::new();
    let file = fs::File::create(f.root.join("large")).unwrap();
    file.set_len(10 * 1024 * 1024 + 1).unwrap();
    let small = f.snapshot();
    assert_eq!(small.skipped, vec!["large"]);
    for n in 0..21 {
        let mut bytes = vec![0u8; 10 * 1024 * 1024];
        bytes[0] = n;
        fs::write(f.root.join(format!("big{n}")), bytes).unwrap();
    }
    let skipped = f.snapshot();
    assert_eq!(
        skipped.warning.as_deref(),
        Some("checkpoint skipped: too large")
    );
    assert!(skipped.commit.is_none());
    fs::remove_file(f.root.join("big20")).unwrap();
    assert!(f.snapshot().commit.is_some());
    // Only the new ten MB counts, not the existing 200 MB.
    fs::write(f.root.join("big20"), vec![21u8; 10 * 1024 * 1024]).unwrap();
    assert!(f.snapshot().commit.is_some());
}
#[test]
fn restore_rejects_excluded_and_unsafe_paths_before_any_deletion() {
    let f = Fixture::new();
    f.write("kept", "old");
    let before = f.snapshot();
    f.write("added", "keep me");
    f.write(".taucodeignore", "excluded\n");
    f.write("excluded", "keep me");
    for path in ["../outside", ".git/config", "excluded", ".codegraph/db"] {
        let result = f.repo.restore(
            "thread",
            &before.id,
            &["added".into(), path.into()],
            safety_metadata("thread", "pi"),
        );
        assert!(result.is_err(), "{path}");
        assert!(f.root.join("added").exists());
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&f.data, f.root.join("link")).unwrap();
        assert!(f
            .repo
            .restore(
                "thread",
                &before.id,
                &["link/outside".into()],
                safety_metadata("thread", "pi")
            )
            .is_err());
    }
}
#[test]
fn refuses_home_and_root() {
    let f = Fixture::new();
    assert!(Repository::new(&f.data, "w", Path::new("/")).is_err());
    if let Some(home) = std::env::var_os("HOME") {
        assert!(Repository::new(&f.data, "w", Path::new(&home)).is_err());
    }
}
#[test]
fn lock_contention_does_not_touch_files() {
    let f = Fixture::new();
    fs::create_dir_all(&f.repo.dir).unwrap();
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(f.repo.dir.join("taucode.lock"))
        .unwrap();
    lock.lock().unwrap();
    assert!(f
        .repo
        .snapshot(json!({"threadId":"thread"}))
        .unwrap_err()
        .contains("busy"));
}
#[test]
fn codex_rollback_fixture() {
    assert_eq!(
        rollback_params("thread-123", 3),
        json!({"threadId":"thread-123","numTurns":3})
    );
    assert!(unsupported("JSON-RPC -32601 Method not found"));
    assert!(!unsupported("numTurns must be positive"));
}

#[tokio::test]
async fn lifecycle_binds_engine_ids_and_snapshots_after_completion() {
    let f = Fixture::new();
    f.write("file", "before");
    let checkpoints = Checkpoints::new(
        f.data.clone(),
        "workspace".into(),
        f.root.to_str().unwrap().into(),
        Arc::new(|_| {}),
    );
    checkpoints
        .before(
            "workspace",
            "thread",
            "pi",
            &json!({"input":[{"text":"fix parser"}]}),
        )
        .await
        .unwrap();
    checkpoints.observe(&AppServerEvent { workspace_id: "workspace".into(), message: json!({"method":"item/started","params":{"threadId":"thread","turnId":"real-turn","item":{"type":"userMessage","id":"real-user"}}}) });
    f.write("file", "after");
    checkpoints.observe(&AppServerEvent {
        workspace_id: "workspace".into(),
        message: json!({"method":"turn/completed","params":{"threadId":"thread"}}),
    }).unwrap().await.unwrap();
    let rows = f.repo.list("thread").unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].metadata["userItemId"], "real-user");
    assert_eq!(rows[0].metadata["turnId"], "real-turn");
    assert_eq!(rows[1].metadata["kind"], "after");
    let output = Command::new(crate::utils::resolve_git_binary().unwrap())
        .env("GIT_DIR", &f.repo.dir)
        .args(["cat-file", "-p", rows[0].commit.as_ref().unwrap()])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&output.stdout).contains("real-user"));
}
#[tokio::test]
async fn before_timeout_proceeds_and_is_unavailable_but_after_still_runs() {
    let f = Fixture::new();
    f.write("file", "before");
    let events = Arc::new(Mutex::new(vec![]));
    let captured = events.clone();
    let checkpoints = Checkpoints::new(
        f.data.clone(),
        "workspace".into(),
        f.root.to_str().unwrap().into(),
        Arc::new(move |e| captured.lock().unwrap().push(e)),
    );
    let operation = checkpoints.operation.lock().await;
    let started = Instant::now();
    checkpoints
        .before("workspace", "thread", "codex", &json!({"input":[]}))
        .await
        .unwrap();
    assert!(started.elapsed() < Duration::from_secs(3));
    assert!(events
        .lock()
        .unwrap()
        .iter()
        .any(|e| e.message["params"]["warning"]
            .as_str()
            .is_some_and(|s| s.contains("unavailable"))));
    let completion = checkpoints.complete("workspace", "thread").unwrap();
    drop(operation);
    completion.await.unwrap();
    let rows = f.repo.list("thread").unwrap();
    assert_eq!(rows.len(), 2);
    assert!(rows[0].commit.is_none());
    assert!(rows[0].warning.as_ref().unwrap().contains("unavailable"));
    assert!(rows[1].commit.is_some());
}

#[cfg(unix)]
#[test]
fn failed_deletion_restores_all_previously_staged_deletions() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let before = f.snapshot();
    f.write("added", "keep");
    f.write("locked/added", "keep");
    fs::set_permissions(f.root.join("locked"), fs::Permissions::from_mode(0o555)).unwrap();
    let result = f.repo.restore(
        "thread",
        &before.id,
        &["added".into(), "locked/added".into()],
        safety_metadata("thread", "pi"),
    );
    fs::set_permissions(f.root.join("locked"), fs::Permissions::from_mode(0o755)).unwrap();
    assert!(result.is_err());
    assert!(f.root.join("added").exists());
    assert!(f.root.join("locked/added").exists());
}
