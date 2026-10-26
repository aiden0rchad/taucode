use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::{Path, PathBuf}, sync::{Mutex, OnceLock}, time::{Duration, Instant, SystemTime}};
use tauri::Manager;
use tauri_plugin_opener::OpenerExt;

const BUILTINS: &[&str] = &["system", "light", "dark", "dim", "glass-dark", "glass-light", "matrix", "claude-light", "claude-dark", "cyberpunk-neon"];
fn tokens() -> &'static Value {
    static TOKENS: OnceLock<Value> = OnceLock::new();
    TOKENS.get_or_init(|| serde_json::from_str(include_str!("../../src/features/themes/tokens.json")).unwrap())
}
fn safe(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    s.encode_utf16().count() <= 120 && !["url(", "image-set(", "expression"].iter().any(|v| lower.contains(v))
        && !s.chars().any(|c| "@;{}\\<".contains(c))
}
fn number(s: &str) -> bool {
    let s = s.strip_suffix('%').unwrap_or(s);
    let s = s.strip_prefix('-').unwrap_or(s);
    !s.is_empty() && !s.ends_with('.') && s.chars().all(|c| c.is_ascii_digit() || c == '.')
        && s.matches('.').count() <= 1 && s.chars().any(|c| c.is_ascii_digit())
}
fn color(s: &str) -> bool {
    if !safe(s) { return false; }
    if let Some(hex) = s.strip_prefix('#') { return [3,4,6,8].contains(&hex.len()) && hex.bytes().all(|c| c.is_ascii_hexdigit()); }
    if let Some(key) = s.strip_prefix("var(").and_then(|s| s.strip_suffix(')')) { return tokens().get(key).is_some(); }
    for name in ["rgb(", "rgba(", "hsl(", "hsla(", "oklch("] {
        if let Some(body) = s.strip_prefix(name).and_then(|s| s.strip_suffix(')')) {
            let body = body.trim();
            let legacy = body.contains(',');
            if legacy && (name == "oklch(" || body.contains('/')) { return false; }
            let alpha: Vec<_> = body.split('/').collect();
            if alpha.len() > 2 { return false; }
            let mut p: Vec<_> = if legacy { body.split(',').map(str::trim).collect() } else { alpha[0].split_whitespace().collect() };
            if if legacy { ![3,4].contains(&p.len()) } else { p.len() != 3 } { return false; }
            if !legacy && alpha.len() == 2 { p.push(alpha[1].trim()); }
            if name.starts_with("hsl") && (!p[1].ends_with('%') || !p[2].ends_with('%')) { return false; }
            return p.iter().all(|s| number(s));
        }
    }
    if let Some(body) = s.strip_prefix("color-mix(").and_then(|s| s.strip_suffix(')')) {
        let mut depth = 0i32;
        let mut start = 0;
        let mut parts = Vec::new();
        for (i,c) in body.char_indices() {
            if c == '(' { depth += 1; }
            if c == ')' { depth -= 1; }
            if depth < 0 { return false; }
            if c == ',' && depth == 0 { parts.push(body[start..i].trim()); start = i + 1; }
        }
        parts.push(body[start..].trim());
        return depth == 0 && parts.len() == 3 && parts[0] == "in srgb" && parts[1..].iter().all(|s| {
            let raw = s.rsplit_once(char::is_whitespace).filter(|(_, p)| p.ends_with('%') && !p.starts_with('-') && number(p)).map(|(v,_)| v).unwrap_or(s);
            color(raw.trim_end())
        });
    }
    false
}
fn font(s: &str) -> bool {
    safe(s) && s.split(',').all(|p| {
        let p = p.trim();
        if ["serif","sans-serif","monospace","system-ui","ui-serif","ui-sans-serif","ui-monospace","ui-rounded","cursive","fantasy"].contains(&p) { return true; }
        let q = p.as_bytes().first().copied().unwrap_or(0);
        (q == b'"' || q == b'\'') && p.len() > 2 && p.as_bytes().last() == Some(&q)
            && p[1..p.len()-1].chars().all(|c| c.is_ascii_alphanumeric() || c == ' ' || c == '-')
    })
}
fn valid_id(id: &str) -> bool { !id.is_empty() && id.len() <= 40 && id.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-') }
fn custom_id(id: &str) -> Result<(), String> {
    if !valid_id(id) { return Err("Invalid theme id".into()); }
    if BUILTINS.contains(&id) { return Err("Built-in theme ids are reserved".into()); }
    Ok(())
}
pub(crate) fn validate(t: &Value) -> Result<(), String> {
    let obj = t.as_object().ok_or("Theme must be an object")?;
    for k in obj.keys() {
        if !["$schema","schemaVersion","id","name","author","version","description","appearance","extends","tokens","fonts","window","effects","shape","border"].contains(&k.as_str()) { return Err(format!("Unknown field: {k}")); }
    }
    if t["schemaVersion"].as_f64() != Some(1.) { return Err(if t["schemaVersion"].as_f64().unwrap_or(0.) > 1. { "Theme made for a newer taucode" } else { "schemaVersion must be 1" }.into()); }
    if !valid_id(t["id"].as_str().unwrap_or("")) { return Err("Invalid theme id".into()); }
    let name = t["name"].as_str().ok_or("name must be text")?;
    if name.trim().is_empty() || name.encode_utf16().count() > 120 { return Err("name must contain 1–120 characters".into()); }
    for k in ["$schema","author","version","description"] {
        if let Some(v) = t.get(k) { if v.as_str().is_none_or(|s| s.encode_utf16().count() > 1000) { return Err(format!("{k} must be text (at most 1000 characters)")); } }
    }
    if !["dark","light"].contains(&t["appearance"].as_str().unwrap_or("")) { return Err("appearance must be light or dark".into()); }
    if let Some(v) = t.get("extends") { if v.as_str().is_none_or(|s| s == "system" || !BUILTINS.contains(&s)) { return Err("extends must name a built-in theme".into()); } }
    let values = t["tokens"].as_object().ok_or("tokens must be an object")?;
    for (k,v) in values {
        if tokens().get(k).is_none() { return Err(format!("Unknown token: {k}")); }
        let s = v.as_str().ok_or_else(|| format!("Invalid value for {k}"))?;
        let valid = if k.contains("radius") {
            safe(s) && s.strip_suffix("px").or_else(|| s.strip_suffix("rem")).is_some_and(|n| !n.starts_with('-') && !n.ends_with('%') && number(n))
        } else { color(s) };
        if !valid { return Err(format!("Invalid value for {k}")); }
    }
    fn cycle(key: &str, values: &serde_json::Map<String,Value>, path: &mut Vec<String>) -> bool {
        if path.iter().any(|p| p == key) { return true; }
        path.push(key.to_owned());
        if let Some(s) = values.get(key).and_then(Value::as_str) {
            for part in s.split("var(").skip(1) {
                if let Some((key,_)) = part.split_once(')') { if cycle(key, values, path) { return true; } }
            }
        }
        path.pop();
        false
    }
    if values.keys().any(|k| cycle(k, values, &mut Vec::new())) { return Err("Token references contain a cycle".into()); }
    if let Some(v) = t.get("fonts") {
        let obj = v.as_object().ok_or("Invalid local font stack")?;
        if obj.iter().any(|(k,v)| !["ui","mono","prose"].contains(&k.as_str()) || v.as_str().is_none_or(|s| !font(s))) { return Err("Invalid local font stack".into()); }
    }
    if let Some(v) = t.get("window") {
        let obj = v.as_object().ok_or("Invalid window material")?;
        if obj.keys().any(|k| !["translucent","material"].contains(&k.as_str())) || !v["translucent"].is_boolean() || !["none","sidebar","underWindow","hudWindow"].contains(&v["material"].as_str().unwrap_or("")) { return Err("Invalid window material".into()); }
    }
    if let Some(v) = t.get("effects") {
        let obj = v.as_object().ok_or("Invalid effects")?;
        if obj.iter().any(|(k,v)| !["digitalRain","neonGlow"].contains(&k.as_str()) || !v.is_boolean()) { return Err("Invalid effects".into()); }
    }
    if let Some(v) = t.get("shape") {
        let obj = v.as_object().ok_or("Invalid shape")?;
        if obj.iter().any(|(k,v)| match k.as_str() {
            "corners" => v.as_str().is_none_or(|s| !["squircle","chamfer","round"].contains(&s)),
            "chamfer" => v.as_f64().is_none_or(|n| !(4.0..=20.0).contains(&n)),
            _ => true,
        }) { return Err("Invalid shape".into()); }
    }
    if let Some(v) = t.get("border") {
        let obj = v.as_object().ok_or("Invalid border")?;
        if obj.iter().any(|(k,v)| match k.as_str() {
            "gradient" => v.as_array().is_none_or(|a| !(2..=4).contains(&a.len()) || a.iter().any(|c| c.as_str().is_none_or(|s| !color(s)))),
            "angle" => v.as_f64().is_none_or(|n| !(0.0..=360.0).contains(&n)),
            "highlight" => !v.is_boolean(),
            _ => true,
        }) { return Err("Invalid border".into()); }
    }
    Ok(())
}
fn read(path: &Path) -> Result<Value,String> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len() > 128 * 1024 { return Err("Theme file exceeds 128 KiB".into()); }
    let t = serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    validate(&t)?;
    Ok(t)
}
fn atomic(path: &Path, bytes: &[u8]) -> Result<(),String> {
    use std::io::Write;
    let tmp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut f = fs::OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        fs::rename(&tmp, path)
    })();
    if result.is_err() { let _ = fs::remove_file(tmp); }
    result.map_err(|e: std::io::Error| e.to_string())
}
fn save_to(dir: &Path, t: &Value, replace: bool) -> Result<(),String> {
    static WRITE: Mutex<()> = Mutex::new(());
    let _guard = WRITE.lock().map_err(|e| e.to_string())?;
    validate(t)?;
    let id = t["id"].as_str().unwrap();
    custom_id(id)?;
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{id}.taucode-theme.json"));
    if path.exists() && !replace { return Err("Theme id already exists".into()); }
    atomic(&path, &serde_json::to_vec_pretty(t).map_err(|e| e.to_string())?)
}
fn list(dir: &Path) -> Result<Vec<Value>,String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut result = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if !path.to_string_lossy().ends_with(".taucode-theme.json") { continue; }
        let file = path.file_name().unwrap().to_string_lossy().to_string();
        let theme = read(&path).and_then(|t| {
            let id = t["id"].as_str().unwrap();
            custom_id(id)?;
            if file != format!("{id}.taucode-theme.json") { return Err("Filename must match theme id".into()); }
            Ok(t)
        });
        result.push(match theme { Ok(t) => json!({"file":file,"manifest":t}), Err(e) => json!({"file":file,"error":e}) });
    }
    result.sort_by_key(|t| t["file"].as_str().unwrap_or("").to_owned());
    Ok(result)
}
fn folder(app: &tauri::AppHandle) -> Result<PathBuf,String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("themes");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}
#[tauri::command]
pub(crate) fn themes_list(app: tauri::AppHandle) -> Result<Vec<Value>,String> { list(&folder(&app)?) }
#[tauri::command]
pub(crate) fn themes_save(app: tauri::AppHandle, manifest: Value, replace: Option<bool>) -> Result<(),String> { save_to(&folder(&app)?, &manifest, replace.unwrap_or(false)) }
fn delete(dir: &Path, id: &str) -> Result<(),String> { custom_id(id)?; fs::remove_file(dir.join(format!("{id}.taucode-theme.json"))).map_err(|e| e.to_string()) }
#[tauri::command]
pub(crate) fn themes_delete(app: tauri::AppHandle, id: String) -> Result<(),String> { delete(&folder(&app)?, &id) }
fn import(dir: &Path, path: &Path, resolution: Option<&str>) -> Result<Value,String> {
    let mut t = read(path)?;
    let id = t["id"].as_str().unwrap().to_owned();
    let clash = BUILTINS.contains(&id.as_str()) || dir.join(format!("{id}.taucode-theme.json")).exists();
    if clash {
        match resolution {
            Some("replace") => custom_id(&id)?,
            Some("keepBoth") => {
                for n in 2.. {
                    let suffix = format!("-{n}");
                    let next = format!("{}{}", &id[..id.len().min(40 - suffix.len())], suffix);
                    if !dir.join(format!("{next}.taucode-theme.json")).exists() && !BUILTINS.contains(&next.as_str()) { t["id"] = json!(next); break; }
                }
            }
            _ => return Ok(json!({"conflict":true,"manifest":t})),
        }
    }
    save_to(dir, &t, resolution == Some("replace"))?;
    Ok(json!({"manifest":t}))
}
#[tauri::command]
pub(crate) fn themes_import(app: tauri::AppHandle, path: String, resolution: Option<String>) -> Result<Value,String> { import(&folder(&app)?, Path::new(&path), resolution.as_deref()) }
fn export(dir: &Path, id: &str, path: &Path, manifest: Option<Value>) -> Result<(),String> {
    if !valid_id(id) { return Err("Invalid theme id".into()); }
    let t = if BUILTINS.contains(&id) { let t = manifest.ok_or("Built-in manifest required")?; validate(&t)?; if t["id"] != id { return Err("Theme id mismatch".into()); } t } else { read(&dir.join(format!("{id}.taucode-theme.json")))? };
    atomic(path, &serde_json::to_vec_pretty(&t).map_err(|e| e.to_string())?)
}
#[tauri::command]
pub(crate) fn themes_export(app: tauri::AppHandle, id: String, path: String, manifest: Option<Value>) -> Result<(),String> { export(&folder(&app)?, &id, Path::new(&path), manifest) }
#[tauri::command]
pub(crate) fn themes_open_folder(app: tauri::AppHandle) -> Result<(),String> { app.opener().open_path(folder(&app)?.to_string_lossy(), None::<&str>).map_err(|e| e.to_string()) }
fn reveal_path(dir: &Path, file: &str) -> Result<PathBuf,String> {
    if Path::new(file).file_name().is_none_or(|name| name != file) || file.contains(['/', '\\']) || !file.ends_with(".taucode-theme.json") {
        return Err("Invalid theme filename".into());
    }
    Ok(dir.join(file))
}
#[tauri::command]
pub(crate) fn themes_reveal_file(app: tauri::AppHandle, file: String) -> Result<(),String> {
    app.opener().reveal_item_in_dir(reveal_path(&folder(&app)?, &file)?).map_err(|e| e.to_string())
}
fn window_theme(appearance: Option<&str>) -> Result<Option<tauri::Theme>, String> {
    match appearance {
        None => Ok(None),
        Some("dark") => Ok(Some(tauri::Theme::Dark)),
        Some("light") => Ok(Some(tauri::Theme::Light)),
        _ => Err("Invalid window appearance".into()),
    }
}
#[tauri::command]
pub(crate) fn window_set_material<R: tauri::Runtime>(window: tauri::Window<R>, material: Option<String>, appearance: Option<String>) -> Result<(),String> {
    use tauri::window::{Effect, EffectState, EffectsBuilder};
    let effect = match material.as_deref() {
        None | Some("none") => None,
        Some("sidebar") => Some(Effect::Sidebar),
        Some("underWindow") => Some(Effect::UnderWindowBackground),
        Some("hudWindow") => Some(Effect::HudWindow),
        _ => return Err("Invalid window material".into()),
    };
    let theme = window_theme(appearance.as_deref())?;
    window.set_theme(theme).map_err(|e| format!("Window appearance: {e}"))?;
    // macOS 26+: the system Liquid Glass view (what taucode used before M4l) is the one that
    // reliably shows the desktop behind the web view; older systems fall back to vibrancy.
    let liquid = crate::window_material::set_glass(&window, effect.is_some())?;
    // Keep the blur live when the window loses focus; macOS otherwise swaps in an opaque fill.
    window.set_effects(effect.filter(|_| !liquid).map(|e| EffectsBuilder::new().effect(e).state(EffectState::Active).build()))
        .map_err(|e| format!("Window material: {e}"))?;
    eprintln!("{}", json!({"level":"info", "event":"window_material_applied", "window":window.label(), "material":material, "liquidGlass":liquid, "appearance":theme.map(|t| t.to_string())}));
    Ok(())
}
type Metadata = BTreeMap<PathBuf, (u64, SystemTime)>;
fn snapshot(dir: &Path) -> Result<Metadata, String> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(e) => return Err(e.to_string()),
    };
    let mut result = BTreeMap::new();
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if !path.to_string_lossy().ends_with(".taucode-theme.json") { continue; }
        let meta = entry.metadata().map_err(|e| e.to_string())?;
        if meta.is_file() { result.insert(path, (meta.len(), meta.modified().map_err(|e| e.to_string())?)); }
    }
    Ok(result)
}
#[derive(Default)]
struct Watcher { previous: Metadata, contents: BTreeMap<PathBuf, Result<Vec<u8>, String>> }
impl Watcher {
    fn poll(&mut self, next: Metadata, mut read: impl FnMut(&Path) -> Result<Vec<u8>, String>) -> bool {
        let mut changed = self.previous.keys().any(|p| !next.contains_key(p));
        self.contents.retain(|p, _| next.contains_key(p));
        for (path, meta) in &next {
            if self.previous.get(path) == Some(meta) { continue; }
            let contents = if meta.0 > 128 * 1024 { Err("Theme file exceeds 128 KiB".into()) } else { read(path) };
            changed |= self.contents.get(path) != Some(&contents);
            self.contents.insert(path.clone(), contents);
        }
        self.previous = next;
        changed
    }
}
pub(crate) fn watch(app: &tauri::AppHandle) -> Result<(),String> {
    // Commands create the directory lazily. A missing folder is an empty snapshot.
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("themes");
    let previous = snapshot(&dir)?;
    let app = app.clone();
    std::thread::Builder::new().name("theme-watcher".into()).spawn(move || {
        let mut watcher = Watcher { previous, ..Default::default() };
        loop {
            let next_poll = Instant::now() + Duration::from_secs(1);
            match snapshot(&dir) {
                Ok(next) => {
                    let changed = watcher.poll(next, |path| {
                        use std::io::Read;
                        let mut bytes = Vec::new();
                        fs::File::open(path).and_then(|f| f.take(128 * 1024 + 1).read_to_end(&mut bytes)).map_err(|e| e.to_string())?;
                        Ok(bytes)
                    });
                    if changed {
                        std::thread::sleep(Duration::from_millis(200));
                        let _ = crate::event_sink::emit(&app, "themes-changed", ());
                    }
                }
                Err(error) => eprintln!("Theme watcher: {error}"),
            }
            std::thread::sleep(next_poll.saturating_duration_since(Instant::now()));
        }
    }).map_err(|e| e.to_string())?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn appearance_is_independent_of_material() {
        assert_eq!(window_theme(Some("dark")).unwrap(), Some(tauri::Theme::Dark));
        assert_eq!(window_theme(Some("light")).unwrap(), Some(tauri::Theme::Light));
        assert_eq!(window_theme(None).unwrap(), None);
        assert!(window_theme(Some("system")).is_err());
        assert!(window_theme(Some("invalid")).is_err());
    }
    #[test]
    fn shared_fixtures() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/features/themes/fixtures");
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|e| e != "json") { continue; }
            let t: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            assert_eq!(validate(&t).is_ok(), path.file_name().unwrap().to_string_lossy().starts_with("valid-"), "{}", path.display());
        }
    }
    #[test]
    fn reveal_is_limited_to_theme_filenames() {
        let dir = Path::new("themes");
        assert_eq!(reveal_path(dir, "broken.taucode-theme.json").unwrap(), dir.join("broken.taucode-theme.json"));
        for file in ["../outside.taucode-theme.json", "nested/outside.taucode-theme.json", "nested\\\\outside.taucode-theme.json", "other.json"] {
            assert!(reveal_path(dir, file).is_err());
        }
    }
    #[test]
    fn watcher_reads_only_metadata_changes() {
        let path = PathBuf::from("custom.taucode-theme.json");
        let other = PathBuf::from("other.taucode-theme.json");
        let now = SystemTime::now();
        let mut metadata = BTreeMap::from([(path.clone(), (3, now)), (other.clone(), (3, now))]);
        let mut watcher = Watcher::default();
        let mut reads = Vec::new();
        assert!(watcher.poll(metadata.clone(), |p| { reads.push(p.to_path_buf()); Ok(b"one".to_vec()) }));
        assert_eq!(reads.len(), 2);
        for _ in 0..10 { assert!(!watcher.poll(metadata.clone(), |_| panic!("No contents read on unchanged metadata"))); }
        metadata.insert(path.clone(), (3, now + Duration::from_secs(1)));
        reads.clear();
        assert!(watcher.poll(metadata.clone(), |p| { reads.push(p.to_path_buf()); Ok(b"two".to_vec()) }));
        assert_eq!(reads, vec![path.clone()]);
        metadata.insert(path.clone(), (4, now + Duration::from_secs(1)));
        assert!(watcher.poll(metadata.clone(), |p| { assert_eq!(p, path); Ok(b"more".to_vec()) }));
        metadata.remove(&other);
        assert!(watcher.poll(metadata, |_| panic!("Deletion must not read contents")));
        let missing = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        assert!(snapshot(&missing).unwrap().is_empty());
        assert!(!missing.exists());
    }
    #[test]
    fn storage_and_watcher() {
        let dir = std::env::temp_dir().join(format!("themes-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let t = json!({"schemaVersion":1,"id":"test-theme","name":"Test","appearance":"dark","tokens":{}});
        save_to(&dir, &t, false).unwrap();
        assert!(save_to(&dir, &t, false).is_err());
        save_to(&dir, &t, true).unwrap();
        assert_eq!(list(&dir).unwrap()[0]["manifest"], t);
        let output = dir.join("export.json");
        export(&dir,"test-theme",&output,None).unwrap();
        assert_eq!(import(&dir,&output,None).unwrap()["conflict"], true);
        assert_eq!(import(&dir,&output,Some("keepBoth")).unwrap()["manifest"]["id"], "test-theme-2");
        import(&dir,&output,Some("replace")).unwrap();
        let mut reserved = t.clone(); reserved["id"] = json!("dark");
        assert!(save_to(&dir,&reserved,true).is_err());
        assert!(delete(&dir, "../escape").is_err());
        assert!(delete(&dir, "dark").is_err());
        let mut watcher = Watcher { previous: snapshot(&dir).unwrap(), ..Default::default() };
        delete(&dir,"test-theme").unwrap();
        assert!(watcher.poll(snapshot(&dir).unwrap(), |_| panic!("Deletion must not read unchanged files")));
        assert!(!watcher.poll(snapshot(&dir).unwrap(), |_| panic!("Unchanged files must not be read")));
        assert!(!fs::read_dir(&dir).unwrap().any(|e| e.unwrap().path().extension().is_some_and(|s| s == "tmp")));
        fs::remove_dir_all(dir).unwrap();
    }
}

