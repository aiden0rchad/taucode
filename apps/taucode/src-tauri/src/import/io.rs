use serde_json::Value;
use std::{
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
pub(super) const LIMIT: u64 = 200 * 1024 * 1024;

pub(super) fn check(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err("Import cancelled".into())
    } else {
        Ok(())
    }
}

// All source opens pass through this boundary, including attachments.
pub(super) fn safe(path: &Path) -> Result<(), String> {
    let mut current = PathBuf::new();
    for part in path.components() {
        if matches!(part, Component::ParentDir) {
            return Err("Parent paths are not allowed".into());
        }
        current.push(part);
        let name = part.as_os_str().to_string_lossy().to_lowercase();
        if [
            "auth.json",
            ".credentials.json",
            "buddy-tokens.json",
            "ant-did",
            "local storage",
            "partitions",
            "config.json",
            "ccd-ids.json",
            "keychain",
        ]
        .contains(&name.as_str())
            || name.starts_with("cookies")
            || name.starts_with("mcp-oauth")
        {
            return Err("Credential paths are not importable".into());
        }
        if fs::symlink_metadata(&current)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err("Source symlinks are not importable".into());
        }
    }
    Ok(())
}
pub(super) fn read(path: &Path, cancel: &AtomicBool) -> Result<Vec<u8>, String> {
    check(cancel)?;
    safe(path)?;
    let meta = fs::metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() > LIMIT {
        return Err("Not a regular file or exceeds 200 MB".into());
    }
    #[cfg(test)]
    OPENS.with(|v| v.borrow_mut().push(path.to_path_buf()));
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options.open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    let mut chunk = [0; 64 * 1024];
    loop {
        check(cancel)?;
        let n = file.read(&mut chunk).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        if bytes.len() as u64 + n as u64 > LIMIT {
            return Err("File exceeds 200 MB".into());
        }
        bytes.extend_from_slice(&chunk[..n]);
    }
    Ok(bytes)
}
#[cfg(test)]
thread_local! { pub(super) static OPENS: std::cell::RefCell<Vec<PathBuf>> = const { std::cell::RefCell::new(Vec::new()) }; }

pub(super) fn files(
    root: &Path,
    depth: usize,
    cancel: &AtomicBool,
) -> Result<Vec<PathBuf>, String> {
    check(cancel)?;
    if !root.exists() {
        return Ok(vec![]);
    }
    safe(root)?;
    let mut result = vec![];
    for entry in fs::read_dir(root).map_err(|e| e.to_string())? {
        check(cancel)?;
        let entry = entry.map_err(|e| e.to_string())?;
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if kind.is_dir() && depth > 0 && safe(&entry.path()).is_ok() {
            result.extend(files(&entry.path(), depth - 1, cancel)?);
        }
        if kind.is_file() {
            result.push(entry.path());
        }
    }
    result.sort();
    Ok(result)
}

pub(super) fn scrub(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.retain(|key, _| {
                let key = key.to_ascii_lowercase().replace(['_', '-'], "");
                !key.contains("account")
                    && !key.contains("email")
                    && ![
                        "user",
                        "userid",
                        "username",
                        "creatoruserid",
                        "creatoruser",
                        "oauth",
                        "accesstoken",
                        "refreshtoken",
                    ]
                    .contains(&key.as_str())
            });
            for v in map.values_mut() {
                scrub(v);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(scrub),
        _ => (),
    }
}

pub(super) fn json(path: &Path, cancel: &AtomicBool) -> Result<Value, String> {
    let mut value = serde_json::from_slice(&read(path, cancel)?).map_err(|e| e.to_string())?;
    scrub(&mut value);
    Ok(value)
}
pub(super) fn jsonl(
    path: &Path,
    cancel: &AtomicBool,
    defensive: bool,
) -> Result<Vec<Value>, String> {
    let bytes = read(path, cancel)?;
    let text = std::str::from_utf8(&bytes).map_err(|e| e.to_string())?;
    let mut values = vec![];
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        check(cancel)?;
        let mut v = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) if defensive => serde_json::json!({"type":"unparsed","text":line}),
            Err(e) => return Err(format!("Invalid transcript: {e}")),
        };
        scrub(&mut v);
        values.push(v);
    }
    Ok(values)
}
pub(super) fn lines(values: &[Value]) -> Vec<u8> {
    let mut bytes = vec![];
    for value in values {
        serde_json::to_writer(&mut bytes, value).expect("JSON value");
        bytes.push(b'\n');
    }
    bytes
}
