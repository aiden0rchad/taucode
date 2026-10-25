use crate::core::*;
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, SyncSender},
        Mutex, OnceLock,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
static SNIPPETS: AtomicBool = AtomicBool::new(false);
static RUN: OnceLock<String> = OnceLock::new();
static SEQUENCE: AtomicU64 = AtomicU64::new(0);
static ENABLED: AtomicBool = AtomicBool::new(false);
static DROPPED: AtomicU64 = AtomicU64::new(0);
static STATE: OnceLock<Mutex<Option<Writer>>> = OnceLock::new();
static CONTEXT: OnceLock<Value> = OnceLock::new();
pub fn redact_known(text: &str) -> String {
    let keys = SECRETS.get_or_init(Default::default).lock().unwrap();
    redact(text, &std::env::var("HOME").unwrap_or_default(), &keys)
}
pub fn set_context(value: Value) {
    let _ = CONTEXT.set(value);
}
static SECRETS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
enum Message {
    Event(Event),
    Flush(mpsc::Sender<Result<(), String>>),
    Stop,
}
struct Writer {
    tx: SyncSender<Message>,
    join: JoinHandle<()>,
    options: Options,
    dir: PathBuf,
}
pub fn snippets() -> bool {
    enabled() && SNIPPETS.load(Ordering::Relaxed)
}
pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}
pub fn dropped() -> u64 {
    DROPPED.load(Ordering::Relaxed)
}
pub fn remember_secret(key: &str) {
    if !enabled() || key.is_empty() {
        return;
    }
    let mut keys = SECRETS.get_or_init(Default::default).lock().unwrap();
    if !keys.iter().any(|k| k == key) {
        keys.push(key.into());
    }
}
pub fn record(build: impl FnOnce() -> Event) {
    if !enabled() {
        return;
    }
    let Some(state) = STATE.get() else { return };
    if let Ok(guard) = state.try_lock() {
        if let Some(w) = guard.as_ref() {
            if let Err(mpsc::TrySendError::Full(_)) = w.tx.try_send(Message::Event(build())) {
                DROPPED.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}
pub fn configure(on: bool, dir: PathBuf, options: Options) -> Result<(), String> {
    options.validate()?;
    let mut guard = STATE
        .get_or_init(Default::default)
        .lock()
        .map_err(|e| e.to_string())?;
    if on
        && guard
            .as_ref()
            .is_some_and(|w| w.dir == dir && w.options == options)
    {
        return Ok(());
    }
    ENABLED.store(false, Ordering::SeqCst);
    SNIPPETS.store(on && options.include_content_snippets, Ordering::Relaxed);
    if let Some(w) = guard.take() {
        let _ = w.tx.send(Message::Stop);
        let _ = w.join.join();
    }
    if !on {
        crate::observe::clear();
        return Ok(());
    }
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    }
    let (tx, rx) = mpsc::sync_channel(512);
    let path = dir.clone();
    let opts = options.clone();
    let join=std::thread::Builder::new().name("diagnostics-writer".into()).spawn(move|| {
        let run=RUN.get_or_init(||uuid::Uuid::new_v4().to_string()).clone();let mut batch=vec![];let mut inbox=issues(&path);
        let recovery=events(&path).into_iter().filter(|e|inbox.get(&e.fingerprint).is_none_or(|i|e.ts>i.last_seen || (e.ts==i.last_seen && e.run==i.last_event.run && e.seq>i.last_event.seq))).collect::<Vec<_>>();
        aggregate(&mut inbox,&recovery);
        let mut seen:BTreeSet<String>=fs::read(path.join("annotation-cursor.json")).ok().and_then(|v|serde_json::from_slice(&v).ok()).unwrap_or_default();
        let home=std::env::var("HOME").unwrap_or_default();let mut last_digest=Instant::now()-Duration::from_secs(300);let mut last_flush=Instant::now();let mut previous=fs::read(path.join("latest.json")).ok().and_then(|v|serde_json::from_slice::<Value>(&v).ok()).and_then(|v|v["lastDigest"].as_str().map(str::to_owned)).unwrap_or_default();let mut last_sample=Instant::now();
        let flush=|batch:&mut Vec<Event>,inbox:&mut Issues,seen:&mut BTreeSet<String>,last_digest:&mut Instant,previous:&mut String,force:bool|->Result<(),String>{
            merge_annotations(inbox,&path,seen);
            let mut files=std::collections::BTreeMap::<String,Vec<u8>>::new();
            for e in batch.iter(){let bytes=files.entry(format!("events-{}.ndjson",&e.ts[..10])).or_default();serde_json::to_writer(&mut *bytes,e).map_err(|e|e.to_string())?;bytes.push(b'\n');}
            for (name,bytes) in files {append(&path.join(name),&bytes).map_err(|e|e.to_string())?;}
            aggregate(inbox,batch);batch.clear();
            retain(&path,opts.retention,CAP).map_err(|e|e.to_string())?;
            atomic(&path,"issues.json",&serde_json::to_vec(inbox).unwrap()).map_err(|e|e.to_string())?;
            atomic(&path,"annotation-cursor.json",&serde_json::to_vec(seen).unwrap()).map_err(|e|e.to_string())?;
            if force || last_digest.elapsed()>=Duration::from_secs(300) {
                let all=events(&path);atomic(&path,"digest.md",digest(&all,inbox,previous).as_bytes()).map_err(|e|e.to_string())?;*last_digest=Instant::now();*previous=chrono::Utc::now().to_rfc3339();
            }
                atomic(&path,"latest.json",serde_json::to_string(&json!({"v":1,"events":event_files(&path).iter().filter_map(|p|p.file_name().map(|s|s.to_string_lossy().into_owned())).collect::<Vec<_>>(),"issues":"issues.json","digest":"digest.md","annotations":"annotations.ndjson","lastDigest":previous,"dropped":dropped()})).unwrap().as_bytes()).map_err(|e|e.to_string())?;
            Ok(())
        };
        loop {
            if opts.level=="verbose" && last_sample.elapsed()>=Duration::from_secs(60) {
                for e in crate::observe::sample() {if let Some(mut e)=sanitize(e,&opts,&home,&[]){e.seq=SEQUENCE.fetch_add(1,Ordering::Relaxed)+1;e.run=run.clone();e.ts=chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis,true);batch.push(e);}}
                last_sample=Instant::now();
            }
            let message=rx.recv_timeout(Duration::from_secs(2).saturating_sub(last_flush.elapsed()));
            match message {
                Ok(Message::Event(mut e))=>{
                    if let Some(defaults)=CONTEXT.get().and_then(Value::as_object){if let Some(context)=e.context.as_object_mut(){for (key,value) in defaults{context.entry(key).or_insert_with(||value.clone());}}}
                    if e.kind=="feedback" {if let Some(id)=e.context["responseId"].as_str(){if let Some(metadata)=crate::observe::response_context(id).or_else(||events(&path).into_iter().rev().find(|event|event.context["responseId"]==id).map(|event|event.context)){if let Some(context)=e.context.as_object_mut(){for (key,value) in metadata.as_object().unwrap(){context.entry(key).or_insert_with(||value.clone());}}}}}
                    let keys=SECRETS.get_or_init(Default::default).lock().unwrap();if let Some(mut e)=sanitize(e,&opts,&home,&keys) {e.seq=SEQUENCE.fetch_add(1,Ordering::Relaxed)+1;e.run=run.clone();e.ts=chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis,true);
                    if e.kind=="feedback" && e.detail["attachedEvents"]==20 {
                        let mut recent=events(&path);recent.extend(batch.iter().cloned());e.detail["attachedEvents"]=json!(recent.iter().rev().take(20).map(|e|json!({"run":e.run,"seq":e.seq})).collect::<Vec<_>>());
                    }
                    if e.kind=="feedback" && e.detail["attachedEvents"]==0 {e.detail.as_object_mut().unwrap().remove("attachedEvents");}
                    batch.push(e);}},
                Ok(Message::Flush(reply))=>{let result=flush(&mut batch,&mut inbox,&mut seen,&mut last_digest,&mut previous,true);let _=reply.send(result);last_flush=Instant::now();},
                Ok(Message::Stop)|Err(mpsc::RecvTimeoutError::Disconnected)=>{let _=flush(&mut batch,&mut inbox,&mut seen,&mut last_digest,&mut previous,true);break;},
                Err(mpsc::RecvTimeoutError::Timeout)=>{}
            }
            if batch.len()>=100 || last_flush.elapsed()>=Duration::from_secs(2) {if !batch.is_empty() {if let Err(e)=flush(&mut batch,&mut inbox,&mut seen,&mut last_digest,&mut previous,false) {eprintln!("Diagnostics write failed: {e}");batch.clear();}}last_flush=Instant::now();}
        }
    }).map_err(|e|e.to_string())?;
    *guard = Some(Writer {
        tx,
        join,
        options,
        dir,
    });
    ENABLED.store(true, Ordering::SeqCst);
    Ok(())
}
pub fn flush() -> Result<(), String> {
    let guard = STATE
        .get_or_init(Default::default)
        .lock()
        .map_err(|e| e.to_string())?;
    let Some(w) = guard.as_ref() else {
        return Ok(());
    };
    let (tx, rx) = mpsc::channel();
    w.tx.send(Message::Flush(tx)).map_err(|e| e.to_string())?;
    rx.recv().map_err(|e| e.to_string())?
}
pub fn stop() {
    let _ = configure(false, PathBuf::new(), Options::default());
}
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if enabled() {
            record(|| {
                let mut e = event("error", "startup", "app.panic", "Application panic");
                e.severity = "critical".into();
                if let Some(loc) = info.location() {
                    e.detail = json!({"stack":format!("{}:{}",loc.file(),loc.line())});
                }
                e
            });
        }
        previous(info);
    }));
}
pub fn metadata(kind: &str, area: &str, code: &str, detail: Value) {
    record(|| {
        let mut e = event(kind, area, code, code);
        e.detail = detail;
        e
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn off_batching_overflow_and_private_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("journal");
        configure(false, path.clone(), Options::default()).unwrap();
        record(|| panic!("disabled instrumentation must not run"));
        assert!(!path.exists());
        configure(true, path.clone(), Options::default()).unwrap();
        let thread = "e53185da-2175-4cf4-9846-79306911b175";
        crate::observe::ready("omp", Some(std::time::Instant::now() - std::time::Duration::from_millis(8200)));
        for n in 1..=2 {
            crate::observe::observe(&json!({"method":"turn/started","params":{"threadId":thread,"turn":{"id":format!("{thread}:turn:{n}")},"engine":"omp","model":"GLM 5.3"}}));
            crate::observe::observe(&json!({"method":"thread/tokenUsage/updated","params":{"threadId":thread,"tokenUsage":{"total":{"inputTokens":129*n,"cachedInputTokens":14080*n,"outputTokens":102*n,"cost":0}}}}));
            crate::observe::observe(&json!({"method":"turn/completed","params":{"threadId":thread,"turn":{"status":"completed"}}}));
        }
        flush().unwrap();
        let observed = events(&path);
        let ready = observed.iter().find(|e| e.code == "engine.ready").unwrap();
        assert!(ready.metrics["durationMs"].as_u64().unwrap() >= 8200);
        assert!(is_issue(ready));
        let turns = observed.iter().filter(|e| e.code == "agent.turn").collect::<Vec<_>>();
        assert_eq!(turns.len(), 2);
        for turn in turns {
            assert_eq!(turn.context["threadId"], thread);
            assert_eq!(turn.metrics["tokensIn"], 129);
            assert_eq!(turn.metrics["cachedTokens"], 14080);
            assert_eq!(turn.metrics["tokensOut"], 102);
            assert_eq!(turn.metrics.get("costUsd"), Some(&Value::Null));
        }
        assert!(!observed.iter().any(|e| e.code == "agent.high_tokens"));
        for (n, cost) in [json!(1), Value::Null, json!(2)].into_iter().enumerate() {
            crate::observe::observe(&json!({"method":"turn/started","params":{"threadId":"pricing","turn":{"id":format!("priced-{n}")}}}));
            crate::observe::observe(&json!({"method":"thread/tokenUsage/updated","params":{"threadId":"pricing","tokenUsage":{"last":{"cost":cost},"total":{"cost":if n == 2 {3} else {1}}}}}));
            crate::observe::observe(&json!({"method":"turn/completed","params":{"threadId":"pricing","turn":{"status":"completed"}}}));
        }
        flush().unwrap();
        let costs = events(&path).into_iter().filter(|e| e.code == "agent.turn" && e.context["threadId"] == "pricing").map(|e| e.metrics["costUsd"].clone()).collect::<Vec<_>>();
        assert_eq!(costs, vec![json!(1.0), Value::Null, json!(2.0)]);
        remember_secret("private-key-value");
        for _ in 0..2000 {
            record(|| {
                let mut e = event(
                    "error",
                    "ui",
                    "ui.test",
                    "private-key-value user@example.com /Users/test/project",
                );
                e.detail =
                    json!({"prompt":"private prompt","stderrTail":"private terminal output"});
                e
            });
        }
        flush().unwrap();
        assert!(dropped() > 0);
        let all = events(&path);
        assert!(!all.is_empty());
        assert!(all.windows(2).all(|e| e[0].seq < e[1].seq));
        for p in fs::read_dir(&path).unwrap().filter_map(Result::ok) {
            let text = fs::read_to_string(p.path()).unwrap();
            for secret in [
                "private-key-value",
                "user@example.com",
                "private prompt",
                "private terminal output",
            ] {
                assert!(!text.contains(secret));
            }
        }
        stop();
        let before = events(&path).len();
        record(|| panic!("off"));
        assert_eq!(events(&path).len(), before);
    }
}
