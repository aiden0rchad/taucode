use super::Computer;
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

#[cfg(unix)]
fn socket(data: &Path) -> PathBuf {
    use sha2::{Digest, Sha256};
    // Keep UNIX paths short, private, and independent of TMPDIR differences between clients.
    let hash = format!("{:x}", Sha256::digest(data.to_string_lossy().as_bytes()));
    PathBuf::from(format!(
        "/tmp/taucode-cua-{}-{}.sock",
        unsafe { libc::geteuid() },
        &hash[..16]
    ))
}
pub async fn serve(computer: Arc<Computer>, app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::{
            fd::AsRawFd,
            unix::fs::{FileTypeExt, MetadataExt, PermissionsExt},
        };
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(computer.data.join("computer/owner.lock"))
            .map_err(|e| e.to_string())?;
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::WouldBlock {
                return Ok(());
            }
            return Err(error.to_string());
        }
        let path = socket(&computer.data);
        if let Ok(meta) = std::fs::symlink_metadata(&path) {
            if !meta.file_type().is_socket() || meta.uid() != unsafe { libc::geteuid() } {
                return Err("Computer socket has unexpected owner or type".into());
            }
            std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
        let listener = tokio::net::UnixListener::bind(&path).map_err(|e| e.to_string())?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
        computer
            .owned
            .store(true, std::sync::atomic::Ordering::SeqCst);
        tokio::spawn(async move {
            let _lock = lock;
            while let Ok((stream, _)) = listener.accept().await {
                let computer = computer.clone();
                let app = app.clone();
                tokio::spawn(async move {
                    let (read, mut write) = stream.into_split();
                    let mut read = BufReader::new(read.take(1024 * 1024));
                    let mut line = String::new();
                    if !matches!(tokio::time::timeout(std::time::Duration::from_secs(10),read.read_line(&mut line)).await,Ok(Ok(n)) if n>0)
                    {
                        return;
                    }
                    let p: Value = match serde_json::from_str(&line) {
                        Ok(v) => v,
                        Err(_) => return,
                    };
                    if !matches!(
                        p["action"].as_str(),
                        Some(
                            "run"
                                | "screenshot"
                                | "status"
                                | "runLog"
                                | "prepare"
                                | "stop"
                                | "pause"
                        )
                    ) {
                        return;
                    }
                    let mut events = computer.state.subscribe();
                    let request = computer.request(app, p);
                    tokio::pin!(request);
                    let mut closed = [0u8; 1];
                    loop {
                        tokio::select! {
                            result=&mut request=>{let response=match result{Ok(v)=>json!({"result":v}),Err(e)=>json!({"error":e})};let _=write.write_all(format!("{response}\n").as_bytes()).await;break;},
                            _=read.read(&mut closed)=>break,
                            event=events.changed()=>{if event.is_err(){break;}let state=events.borrow().clone();if write.write_all(format!("{}\n",json!({"progress":state})).as_bytes()).await.is_err(){break;}}
                        }
                    }
                    // Dropping the run future invokes StopOnDrop and releases inputs.
                });
            }
        });
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = (computer, app);
        Ok(())
    }
}
async fn exchange(data: &Path, p: Value, mut progress: impl FnMut(Value)) -> Result<Value, String> {
    #[cfg(unix)]
    {
        let mut socket = tokio::net::UnixStream::connect(socket(data))
            .await
            .map_err(|_| "Open taucode on the Mac to use Computer Use")?;
        socket
            .write_all(format!("{p}\n").as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        let mut read = BufReader::new(socket);
        loop {
            let mut line = String::new();
            let n = (&mut read)
                .take(32 * 1024 * 1024)
                .read_line(&mut line)
                .await
                .map_err(|e| e.to_string())?;
            if n == 0 || !line.ends_with('\n') {
                return Err("Computer host disconnected".into());
            }
            let v: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
            if let Some(p) = v.get("progress") {
                progress(p.clone());
                continue;
            }
            if let Some(e) = v["error"].as_str() {
                return Err(e.into());
            }
            return Ok(v["result"].clone());
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (data, p, &mut progress);
        Err("Computer Use requires macOS".into())
    }
}
pub async fn ipc(data: &Path, p: Value) -> Result<Value, String> {
    exchange(data, p, |_| {}).await
}
fn tools() -> Value {
    json!([
        {"name":"computer_run_task","description":"Operate allowed native macOS apps. Requires Computer Use settings, permissions, and per-action approvals. Return includes a persisted run and final screenshot.","inputSchema":{"type":"object","properties":{"instruction":{"type":"string"},"apps":{"type":"array","items":{"type":"string"}},"max_steps":{"type":"integer","minimum":1}},"required":["instruction"],"additionalProperties":false},"annotations":{"readOnlyHint":false,"destructiveHint":true}},
        {"name":"computer_screenshot","description":"Capture the main display excluding taucode windows.","inputSchema":{"type":"object","properties":{},"additionalProperties":false},"annotations":{"readOnlyHint":true}}
    ])
}
fn output(v: Value) {
    use std::io::Write;
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{v}");
    let _ = out.flush();
}
pub fn mcp_cli(args: Vec<std::ffi::OsString>) -> Result<(), String> {
    if args.len() != 1 {
        return Err("Usage: --tau-computer-mcp <app-data-dir>".into());
    }
    let data = PathBuf::from(&args[0]);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    // A dedicated stdin reader lets EOF/cancellation close the host socket during a run.
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    std::thread::spawn(move || {
        use std::io::{BufRead, Read};
        let mut input = std::io::stdin().lock();
        loop {
            let mut line = String::new();
            match (&mut input).take(1024 * 1024).read_line(&mut line) {
                Ok(n) if n > 0 && line.ends_with('\n') => {
                    if tx.send(line).is_err() {
                        break;
                    }
                }
                _ => break,
            }
        }
    });
    runtime.block_on(async move{
        let mut calls=std::collections::HashMap::<String,tokio::task::JoinHandle<()>>::new();
        while let Some(line)=rx.recv().await {
            let p:Value=match serde_json::from_str(&line){Ok(v)=>v,Err(_)=>{output(json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Parse error"}}));continue;}};
            if p["method"]=="notifications/cancelled"{if let Some(task)=calls.remove(&p["params"]["requestId"].to_string()){task.abort();}continue;}
            let Some(id)=p.get("id").cloned() else{continue};
            let reply=match p["method"].as_str(){
                Some("initialize")=>Ok(json!({"protocolVersion":p["params"]["protocolVersion"].as_str().unwrap_or("2024-11-05"),"capabilities":{"tools":{}},"serverInfo":{"name":"taucode-computer","version":"0.1.0"}})),
                Some("ping")=>Ok(json!({})),Some("tools/list")=>Ok(json!({"tools":tools()})),
                Some("tools/call")=>{
                    let name=p["params"]["name"].as_str().unwrap_or("");
                    let mut args=p["params"].get("arguments").cloned().unwrap_or(json!({}));
                    if !args.is_object() || !["computer_run_task","computer_screenshot"].contains(&name){Err("Unknown tool or invalid arguments")} else {
                        args["action"]=json!(if name=="computer_run_task"{"run"}else{"screenshot"});
                        let data=data.clone();let token=p["params"]["_meta"]["progressToken"].clone();let task_id=id.clone();
                        let task=tokio::spawn(async move{
                            let result=exchange(&data,args,|s|{if !token.is_null(){output(json!({"jsonrpc":"2.0","method":"notifications/progress","params":{"progressToken":token,"progress":s["steps"].as_array().map_or(0,Vec::len),"message":s["steps"].as_array().and_then(|a|a.last()).map(|s|s["action"].to_string()).unwrap_or_else(||"Preparing computer run".into())}}));}}).await;
                            let result=match result {Ok(mut v)=>{let image=v.as_object_mut().and_then(|o|o.remove("screenshot"));let mut content=vec![json!({"type":"text","text":v.to_string()})];if let Some(image)=image{content.push(json!({"type":"image","mimeType":"image/png","data":image}));}json!({"content":content,"isError":v.get("error").is_some()})},Err(e)=>json!({"content":[{"type":"text","text":e}],"isError":true})};
                            output(json!({"jsonrpc":"2.0","id":task_id,"result":result}));
                        });
                        calls.retain(|_,task|!task.is_finished());
                        if let Some(old)=calls.insert(id.to_string(),task){old.abort();}
                        continue;
                    }
                },_=>Err("Unknown method")
            };
            output(match reply{Ok(v)=>json!({"jsonrpc":"2.0","id":id,"result":v}),Err(e)=>json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":e}})});
        }
        for (_,task) in calls{task.abort();}
        Ok(())
    })
}
