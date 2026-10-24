use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const FILE_LIMIT: u64 = 10 * 1024 * 1024;
const SNAPSHOT_LIMIT: u64 = 200 * 1024 * 1024;
type Result<T> = std::result::Result<T, String>;
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Checkpoint {
    pub id: String,
    pub commit: Option<String>,
    pub metadata: Value,
    pub skipped: Vec<String>,
    pub warning: Option<String>,
    pub files_changed: usize,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Change {
    pub path: String,
    pub status: String,
    pub diff: String,
}
#[derive(Clone)]
pub struct Repository {
    pub root: PathBuf,
    pub dir: PathBuf,
    limits: super::CheckpointLimits,
}

struct Lock(fs::File);
impl Drop for Lock {
    fn drop(&mut self) {
        // A concurrent fork can inherit this open file description. Closing only
        // our descriptor can leave its flock held until that child execs/exits.
        let _ = self.0.unlock();
    }
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn valid_id(id: &str) -> Result<()> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_:".contains(&b))
    {
        return Err("Invalid checkpoint identifier".into());
    }
    Ok(())
}
fn thread_ref(id: &str) -> Result<String> {
    valid_id(id)?;
    Ok(format!("refs/taucode/{}", id.replace(':', "%3A")))
}
impl Repository {
    pub fn new(data: &Path, workspace: &str, root: &Path) -> Result<Self> {
        Self::with_limits(data, workspace, root, super::CheckpointLimits::default())
    }
    pub fn with_limits(data: &Path, workspace: &str, root: &Path, limits: super::CheckpointLimits) -> Result<Self> {
        valid_id(workspace)?;
        let root = root.canonicalize().map_err(err)?;
        let home = std::env::var_os("HOME").and_then(|p| fs::canonicalize(p).ok());
        if root.parent().is_none()
            || home.as_ref() == Some(&root)
            || [
                "/Users",
                "/Volumes",
                "/private",
                "/private/var",
                "/var",
                "/home",
            ]
            .iter()
            .any(|p| root == Path::new(p))
            || [
                "/System",
                "/Applications",
                "/Library",
                "/boot",
                "/lib",
                "/lib64",
                "/run",
                "/private/var/db",
                "/bin",
                "/sbin",
                "/usr",
                "/etc",
                "/private/etc",
                "/dev",
                "/proc",
                "/sys",
                "/Windows",
            ]
            .iter()
            .any(|p| root.starts_with(p))
        {
            return Err("Checkpoints disabled for this folder.".into());
        }
        Ok(Self {
            limits,
            root,
            dir: data.join("checkpoints").join(format!("{workspace}.git")),
        })
    }
    fn lock(&self) -> Result<Lock> {
        fs::create_dir_all(&self.dir).map_err(err)?;
        let path = self.dir.join("taucode.lock");
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .map_err(err)?;
        file.try_lock()
            .map_err(|e| format!("Checkpoint workspace busy: {e}"))?;
        Ok(Lock(file))
    }
    fn command(&self) -> Result<Command> {
        let mut cmd = Command::new(crate::utils::resolve_git_binary()?);
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("GIT_") {
                cmd.env_remove(key);
            }
        }
        cmd.current_dir(&self.root)
            .env("PATH", crate::utils::git_env_path())
            .env("GIT_DIR", &self.dir)
            .env("GIT_WORK_TREE", &self.root)
            .env("GIT_INDEX_FILE", self.dir.join("index"))
            .env("GIT_AUTHOR_NAME", "taucode")
            .env("GIT_AUTHOR_EMAIL", "checkpoints@taucode.local")
            .env("GIT_COMMITTER_NAME", "taucode")
            .env("GIT_COMMITTER_EMAIL", "checkpoints@taucode.local")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_TERMINAL_PROMPT", "0")
            .args([
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.autocrlf=false",
            ]);
        Ok(cmd)
    }
    fn git(&self, args: &[&str], input: &[u8], deadline: Instant) -> Result<Vec<u8>> {
        run(self.command()?, args, input.to_vec(), deadline)
    }
    fn text(&self, args: &[&str], deadline: Instant) -> Result<String> {
        String::from_utf8(self.git(args, &[], deadline)?)
            .map(|s| s.trim().into())
            .map_err(err)
    }
    fn initialize(&self, deadline: Instant) -> Result<()> {
        if !self.dir.join("HEAD").exists() {
            // git init --bare rejects GIT_WORK_TREE. Initialize with explicit
            // routing first, then mark the shadow storage bare.
            self.git(&["init"], &[], deadline)?;
            let _ = self.git(&["config", "--unset", "core.worktree"], &[], deadline);
            self.git(&["config", "core.logAllRefUpdates", "false"], &[], deadline)?;
            self.git(&["config", "core.bare", "true"], &[], deadline)?;
        }
        let git_path = self.root.join(".git");
        let user_git = if git_path.is_file() {
            let text = fs::read_to_string(&git_path).map_err(err)?;
            self.root.join(
                text.trim()
                    .strip_prefix("gitdir: ")
                    .ok_or("Invalid worktree git directory")?,
            )
        } else {
            git_path
        };
        let common = fs::read_to_string(user_git.join("commondir"))
            .map(|s| user_git.join(s.trim()))
            .unwrap_or(user_git);
        let mut excludes = fs::read_to_string(common.join("info/exclude")).unwrap_or_default();
        excludes.push('\n');
        excludes
            .push_str(&fs::read_to_string(self.root.join(".taucodeignore")).unwrap_or_default());
        excludes.push_str("\n.git\n.git/\n.codegraph*\n");
        fs::write(self.dir.join("info/exclude"), excludes).map_err(err)?;
        let _ = self.git(&["config", "--unset", "core.excludesFile"], &[], deadline);
        let config = common.join("config");
        if config.is_file() {
            if let Ok(path) = self.text(
                &[
                    "config",
                    "--file",
                    config.to_str().ok_or("Non-UTF8 git config")?,
                    "--path",
                    "--get",
                    "core.excludesFile",
                ],
                deadline,
            ) {
                self.git(&["config", "core.excludesFile", &path], &[], deadline)?;
            }
        }
        Ok(())
    }
    fn records(&self, thread: &str) -> Result<Vec<Checkpoint>> {
        valid_id(thread)?;
        match fs::read(self.dir.join(format!("{thread}.json"))) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(err),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
            Err(e) => Err(err(e)),
        }
    }
    fn save(&self, thread: &str, rows: &[Checkpoint]) -> Result<()> {
        let temp = self.dir.join("records.tmp");
        fs::write(&temp, serde_json::to_vec(rows).map_err(err)?).map_err(err)?;
        fs::rename(temp, self.dir.join(format!("{thread}.json"))).map_err(err)
    }
    pub fn list(&self, thread: &str) -> Result<Vec<Checkpoint>> {
        let _lock = self.lock()?;
        self.records(thread)
    }
    pub fn unavailable(&self, thread: &str, id: &str) -> Result<()> {
        let _lock = self.lock()?;
        let mut rows = self.records(thread)?;
        if let Some(row) = rows.iter_mut().find(|r| r.id == id) {
            row.commit = None;
            row.warning = Some(self.limits.unavailable());
        }
        self.save(thread, &rows)
    }
    pub fn bind(&self, thread: &str, id: &str, turn: &str, item: &str) -> Result<()> {
        let _lock = self.lock()?;
        let mut rows = self.records(thread)?;
        if let Some(row) = rows.iter_mut().find(|r| r.id == id) {
            if !turn.is_empty() {
                row.metadata["turnId"] = turn.into();
            }
            if !item.is_empty() {
                row.metadata["userItemId"] = item.into();
            }
            if let Some(old) = &row.commit {
                let deadline = Instant::now() + self.limits.snapshot;
                let tree = self.text(&["rev-parse", &format!("{old}^{{tree}}")], deadline)?;
                let parent = self
                    .text(&["rev-parse", "--verify", &format!("{old}^")], deadline)
                    .ok();
                let mut args = vec!["commit-tree", &tree];
                if let Some(parent) = &parent {
                    args.extend(["-p", parent]);
                }
                let message = format!("taucode checkpoint\n\n{}\n", row.metadata);
                let commit = String::from_utf8(self.git(&args, message.as_bytes(), deadline)?)
                    .map_err(err)?
                    .trim()
                    .to_owned();
                self.git(
                    &["update-ref", &thread_ref(thread)?, &commit, old],
                    &[],
                    deadline,
                )?;
                row.commit = Some(commit);
            }
            self.save(thread, &rows)?;
        }
        Ok(())
    }
    fn safe_path(&self, path: &str) -> Result<PathBuf> {
        if path.is_empty()
            || Path::new(path)
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err(format!("{path}: unsafe path"));
        }
        let mut full = self.root.clone();
        for part in Path::new(path).components() {
            let name = part.as_os_str().to_string_lossy();
            if name.eq_ignore_ascii_case(".git")
                || name.to_ascii_lowercase().starts_with(".codegraph")
            {
                return Err(format!("{path}: excluded path"));
            }
            full.push(part);
            if let Ok(meta) = fs::symlink_metadata(&full) {
                if meta.file_type().is_symlink() {
                    return Err(format!("{path}: symlinks are excluded"));
                }
            }
        }
        Ok(full)
    }
    fn ignored(&self, path: &str, deadline: Instant) -> Result<bool> {
        match self.git(&["check-ignore", "--no-index", "--", path], &[], deadline) {
            Ok(_) => Ok(true),
            Err(e) if e.starts_with("git exit 1:") => Ok(false),
            Err(e) => Err(e),
        }
    }
    fn tree(&self, deadline: Instant) -> Result<(String, Vec<String>)> {
        self.git(&["read-tree", "--empty"], &[], deadline)?;
        let paths = self.git(
            &["ls-files", "--others", "--exclude-standard", "-z"],
            &[],
            deadline,
        )?;
        let mut skipped = vec![];
        let mut files = vec![];

        for path in paths.split(|b| *b == 0).filter(|p| !p.is_empty()) {
            let path = std::str::from_utf8(path).map_err(err)?.to_owned();
            let full = match self.safe_path(&path) {
                Ok(p) => p,
                Err(_) => {
                    skipped.push(path);
                    continue;
                }
            };
            let meta = fs::metadata(&full).map_err(|e| format!("{path}: {e}"))?;
            if !meta.is_file() || meta.len() > FILE_LIMIT {
                skipped.push(path);
                continue;
            }
            #[cfg(unix)]
            let mode = {
                use std::os::unix::fs::PermissionsExt;
                if meta.permissions().mode() & 0o111 != 0 {
                    "100755"
                } else {
                    "100644"
                }
            };
            #[cfg(not(unix))]
            let mode = "100644";
            files.push((path, meta.len(), mode));
        }
        let mut hashes = Vec::new();
        for batch in files.chunks(512) {
            let mut args = vec!["hash-object", "--no-filters", "--"];
            args.extend(batch.iter().map(|(p, _, _)| p.as_str()));
            hashes.extend(self.text(&args, deadline)?.lines().map(str::to_owned));
        }
        let present = self.git(
            &["cat-file", "--batch-check=%(objectname) %(objecttype)"],
            format!("{}\n", hashes.join("\n")).as_bytes(),
            deadline,
        )?;
        let present = String::from_utf8(present).map_err(err)?;
        let mut unique = HashSet::new();
        let new_bytes: u64 = files
            .iter()
            .zip(hashes.iter())
            .zip(present.lines())
            .filter_map(|(((_, size, _), hash), line)| {
                (line.ends_with(" missing") && unique.insert(hash)).then_some(*size)
            })
            .sum();
        if new_bytes > SNAPSHOT_LIMIT {
            return Err("checkpoint skipped: too large".into());
        }
        let mut written = Vec::new();
        for batch in files.chunks(512) {
            for (path, _, _) in batch {
                self.safe_path(path)?;
            }
            let mut args = vec!["hash-object", "-w", "--no-filters", "--"];
            args.extend(batch.iter().map(|(p, _, _)| p.as_str()));
            written.extend(self.text(&args, deadline)?.lines().map(str::to_owned));
        }
        if written != hashes {
            return Err("Files changed during checkpoint".into());
        }
        let mut index = Vec::new();
        for ((path, _, mode), hash) in files.iter().zip(hashes) {
            index.extend_from_slice(format!("{mode} {hash}\t{path}\0").as_bytes());
        }
        self.git(&["update-index", "-z", "--index-info"], &index, deadline)?;
        Ok((self.text(&["write-tree"], deadline)?, skipped))
    }
    pub fn snapshot(&self, metadata: Value) -> Result<Checkpoint> {
        let _lock = self.lock()?;
        self.snapshot_locked(metadata, Instant::now() + self.limits.snapshot)
    }
    fn snapshot_locked(&self, metadata: Value, deadline: Instant) -> Result<Checkpoint> {
        self.initialize(deadline)?;
        let thread = metadata["threadId"].as_str().ok_or("Missing thread ID")?;
        let reference = thread_ref(thread)?;
        let mut rows = self.records(thread)?;
        let mut row = Checkpoint {
            id: metadata["checkpointId"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            commit: None,
            metadata: metadata.clone(),
            skipped: vec![],
            warning: None,
            files_changed: 0,
        };
        match self.tree(deadline) {
            Ok((tree, skipped)) => {
                row.skipped = skipped;
                let parent = rows.iter().rev().find_map(|r| r.commit.as_deref());
                let mut args = vec!["commit-tree", &tree];
                if let Some(parent) = parent {
                    args.extend(["-p", parent]);
                }
                let message = format!("taucode checkpoint\n\n{}\n", metadata);
                let commit = String::from_utf8(self.git(&args, message.as_bytes(), deadline)?)
                    .map_err(err)?
                    .trim()
                    .to_owned();
                row.files_changed = self
                    .git(
                        &[
                            "diff-tree",
                            "--root",
                            "--no-commit-id",
                            "--name-only",
                            "-r",
                            "-z",
                            &commit,
                        ],
                        &[],
                        deadline,
                    )?
                    .split(|b| *b == 0)
                    .filter(|s| !s.is_empty())
                    .count();
                self.git(&["update-ref", &reference, &commit], &[], deadline)?;
                row.commit = Some(commit);
            }
            Err(e) => row.warning = Some(e),
        }
        rows.push(row.clone());
        if rows.len() > 300 {
            rows.drain(..rows.len() - 300);
        }
        self.save(thread, &rows)?;
        // Sever older history while preserving the retained commit IDs.
        let mut boundaries = Vec::new();
        for entry in fs::read_dir(&self.dir).map_err(err)? {
            let path = entry.map_err(err)?.path();
            if path.extension().is_some_and(|s| s == "json") {
                let records: Vec<Checkpoint> =
                    serde_json::from_slice(&fs::read(path).map_err(err)?).map_err(err)?;
                if records.len() == 300 {
                    if let Some(commit) = records.iter().find_map(|r| r.commit.as_ref()) {
                        boundaries.push(commit.clone());
                    }
                }
            }
        }
        if !boundaries.is_empty() {
            fs::write(
                self.dir.join("shallow"),
                format!("{}\n", boundaries.join("\n")),
            )
            .map_err(err)?;
        }
        let _ = self.git(
            &["-c", "gc.autoDetach=false", "gc", "--auto"],
            &[],
            deadline,
        );
        Ok(row)
    }
    fn target(&self, thread: &str, id: &str) -> Result<Checkpoint> {
        self.records(thread)?
            .into_iter()
            .find(|r| r.id == id && r.commit.is_some())
            .ok_or("Checkpoint unavailable".into())
    }
    fn entries(&self, tree: &str, deadline: Instant) -> Result<BTreeMap<String, (String, String)>> {
        let bytes = self.git(&["ls-tree", "-r", "-z", tree], &[], deadline)?;
        let mut out = BTreeMap::new();
        for entry in bytes.split(|b| *b == 0).filter(|s| !s.is_empty()) {
            let line = std::str::from_utf8(entry).map_err(err)?;
            let (info, path) = line.split_once('\t').ok_or("Invalid tree entry")?;
            let parts: Vec<_> = info.split(' ').collect();
            if parts.len() != 3 || parts[1] != "blob" {
                return Err(format!("{path}: unsupported tree entry"));
            }
            out.insert(path.into(), (parts[0].into(), parts[2].into()));
        }
        Ok(out)
    }
    pub fn diff(&self, thread: &str, id: &str) -> Result<Vec<Change>> {
        let _lock = self.lock()?;
        let deadline = Instant::now() + self.limits.snapshot;
        self.initialize(deadline)?;
        let target = self.target(thread, id)?;
        let (current, skipped) = self.tree(deadline)?;
        let old = self.entries(target.commit.as_ref().unwrap(), deadline)?;
        let new = self.entries(&current, deadline)?;
        let paths: std::collections::BTreeSet<_> = old.keys().chain(new.keys()).collect();
        let mut changes = vec![];
        for path in paths {
            if old.get(path) == new.get(path)
                || skipped.contains(path)
                || target.skipped.contains(path)
                || self.safe_path(path).is_err()
                || self.ignored(path, deadline)?
            {
                continue;
            }
            let status = if !old.contains_key(path) {
                "added since"
            } else if !new.contains_key(path) {
                "deleted since"
            } else {
                "modified"
            };
            let diff = self.text(
                &[
                    "diff",
                    "--no-ext-diff",
                    "--no-textconv",
                    target.commit.as_ref().unwrap(),
                    &current,
                    "--",
                    path,
                ],
                deadline,
            )?;
            changes.push(Change {
                path: path.clone(),
                status: status.into(),
                diff,
            });
        }
        Ok(changes)
    }
    pub fn restore(
        &self,
        thread: &str,
        id: &str,
        paths: &[String],
        metadata: Value,
    ) -> Result<Checkpoint> {
        let _lock = self.lock()?;
        let deadline = Instant::now() + self.limits.restore;
        self.initialize(deadline)?;
        let target = self.target(thread, id)?;
        let old = self.entries(target.commit.as_ref().unwrap(), deadline)?;
        let mut prepared = vec![];
        let mut errors = vec![];
        let mut seen = HashSet::new();
        for path in paths {
            if !seen.insert(path) {
                continue;
            }
            let result = (|| {
                let full = self.safe_path(path)?;
                if self.ignored(path, deadline)? || target.skipped.contains(path) {
                    return Err(format!("{path}: excluded path"));
                }
                if let Ok(meta) = fs::metadata(&full) {
                    if !meta.is_file() || meta.len() > FILE_LIMIT {
                        return Err(format!("{path}: excluded or non-file path"));
                    }
                }
                let content = match old.get(path) {
                    Some((mode, hash)) if mode == "100644" || mode == "100755" => Some((
                        mode.clone(),
                        self.git(&["cat-file", "blob", hash], &[], deadline)?,
                    )),
                    Some(_) => return Err(format!("{path}: unsupported file type")),
                    None => None,
                };
                Ok((path.clone(), full, content))
            })();
            match result {
                Ok(p) => prepared.push(p),
                Err(e) => errors.push(e),
            }
        }
        if !errors.is_empty() {
            return Err(errors.join("\n"));
        }
        let safety = self.snapshot_locked(
            metadata,
            deadline.min(Instant::now() + self.limits.snapshot),
        )?;
        if safety.commit.is_none() || paths.iter().any(|p| safety.skipped.contains(p)) {
            return Err("Safety checkpoint unavailable; no files restored".into());
        }
        // All blobs and paths are validated before writes; no deletion follows a failed write.
        for (path, full, content) in &prepared {
            if let Some((mode, bytes)) = content {
                let result = (|| {
                    if Instant::now() >= deadline {
                        return Err("Restore timed out".into());
                    }
                    self.safe_path(path)?;
                    fs::create_dir_all(full.parent().ok_or("Missing parent")?).map_err(err)?;
                    let temp = full.with_file_name(format!(".taucode-{}", uuid::Uuid::new_v4()));
                    let result =
                        (|| {
                            let mut file = OpenOptions::new()
                                .write(true)
                                .create_new(true)
                                .open(&temp)
                                .map_err(err)?;
                            file.write_all(bytes).map_err(err)?;
                            #[cfg(unix)]
                            {
                                use std::os::unix::fs::PermissionsExt;
                                file.set_permissions(fs::Permissions::from_mode(
                                    if mode == "100755" { 0o755 } else { 0o644 },
                                ))
                                .map_err(err)?;
                            }
                            self.safe_path(path)?;
                            fs::rename(&temp, full).map_err(err)
                        })();
                    let _ = fs::remove_file(temp);
                    result
                })();
                if let Err(e) = result {
                    errors.push(format!("{path}: {e}"));
                }
            }
        }
        if !errors.is_empty() {
            return Err(format!(
                "{}\nSafety checkpoint: {}",
                errors.join("\n"),
                safety.id
            ));
        }
        // Stage deletions by renaming first. If any path fails, put every
        // staged file back before returning; never partially delete a selection.
        let mut deletions: Vec<(PathBuf, PathBuf)> = vec![];
        for (path, full, content) in prepared {
            if content.is_some() {
                continue;
            }
            let result = (|| {
                if Instant::now() >= deadline {
                    return Err("Restore timed out".into());
                }
                self.safe_path(&path)?;
                let backup =
                    full.with_file_name(format!(".taucode-delete-{}", uuid::Uuid::new_v4()));
                match fs::rename(&full, &backup) {
                    Ok(()) => deletions.push((full, backup)),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                    Err(e) => return Err(err(e)),
                }
                Ok(())
            })();
            if let Err(e) = result {
                errors.push(format!("{path}: {e}"));
                break;
            }
        }
        if !errors.is_empty() {
            for (original, backup) in deletions.into_iter().rev() {
                if let Err(e) = fs::rename(&backup, &original) {
                    errors.push(format!(
                        "{}: {e}; original retained at {}",
                        original.display(),
                        backup.display()
                    ));
                }
            }
            return Err(format!(
                "{}\nSafety checkpoint: {}",
                errors.join("\n"),
                safety.id
            ));
        }
        for (_, backup) in deletions {
            // The selected path is already removed and the safety commit owns
            // its content. A cleanup failure must leave the backup intact.
            let _ = fs::remove_file(backup);
        }
        Ok(safety)
    }
}
fn run(mut cmd: Command, args: &[&str], input: Vec<u8>, deadline: Instant) -> Result<Vec<u8>> {
    if Instant::now() >= deadline {
        return Err("Checkpoint timed out".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = cmd
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(err)?;
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let writer = std::thread::spawn(move || stdin.write_all(&input));
    let reader = std::thread::spawn(move || {
        let mut b = vec![];
        stdout.read_to_end(&mut b).map(|_| b)
    });
    let errors = std::thread::spawn(move || {
        let mut b = vec![];
        stderr.read_to_end(&mut b).map(|_| b)
    });
    let status = loop {
        match child.try_wait().map_err(err)? {
            Some(status) => break Some(status),
            None if Instant::now() >= deadline => {
                #[cfg(unix)]
                unsafe {
                    libc::kill(-(child.id() as i32), libc::SIGKILL);
                }
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            None => std::thread::sleep(Duration::from_millis(5)),
        }
    };
    let _ = writer.join();
    let output = reader
        .join()
        .map_err(|_| "Git output reader failed")?
        .map_err(err)?;
    let stderr = errors
        .join()
        .map_err(|_| "Git error reader failed")?
        .map_err(err)?;
    match status {
        Some(s) if s.success() => Ok(output),
        Some(s) => Err(format!(
            "git exit {}: {}",
            s.code().unwrap_or(-1),
            String::from_utf8_lossy(&stderr)
        )),
        None => Err("Checkpoint timed out".into()),
    }
}

#[cfg(test)]
mod timeout_tests {
    use super::*;
    #[test]
    fn releases_lock_even_when_another_descriptor_is_alive() {
        let data = std::env::temp_dir().join(format!("checkpoint-lock-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&data).unwrap();
        let repo = Repository::new(&data, "workspace", &data).unwrap();
        let lock = repo.lock().unwrap();
        let inherited = lock.0.try_clone().unwrap();
        assert!(repo.lock().is_err());
        drop(lock);
        let next = repo.lock().expect("explicit unlock releases inherited open file descriptions");
        drop(next);
        drop(inherited);
        fs::remove_dir_all(data).unwrap();
    }
    #[test]
    fn kills_timed_out_process() {
        let deadline = Instant::now() + Duration::from_millis(25);
        assert_eq!(
            run(Command::new("/bin/sleep"), &["10"], vec![], deadline).unwrap_err(),
            "Checkpoint timed out"
        );
        assert!(Instant::now() < deadline + Duration::from_secs(1));
    }
}
