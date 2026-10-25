use super::model::{inside, Task};
use std::{path::Path, process::Stdio, time::Duration};
use tokio::{io::AsyncReadExt, process::Command};

pub fn tail(text: &str) -> String {
    let mut start=text.len().saturating_sub(2048);
    while !text.is_char_boundary(start) { start+=1; }
    text[start..].into()
}
struct ProcessGroup(u32);
impl Drop for ProcessGroup {
    fn drop(&mut self) {
        #[cfg(unix)] unsafe { libc::kill(-(self.0 as i32), libc::SIGKILL); }
    }
}
async fn drain(mut reader: impl tokio::io::AsyncRead + Unpin) -> String {
    let mut tail=Vec::new(); let mut buf=[0;4096];
    while let Ok(n)=reader.read(&mut buf).await {
        if n==0 { break; } tail.extend_from_slice(&buf[..n]);
        if tail.len()>2048 { tail.drain(..tail.len()-2048); }
    }
    String::from_utf8_lossy(&tail).into()
}
pub async fn command(root: &Path, script: &str, seconds: u64) -> Result<(i32,String),String> {
    let mut cmd=Command::new("/bin/sh");
    cmd.args(["-c",script]).current_dir(root).env_clear()
        .envs(std::env::vars().filter(|(k,_)|matches!(k.as_str(),"PATH"|"LANG"|"SYSTEMROOT"|"WINDIR")))
        .env("HOME",root).env("TMPDIR",root).env("PYTHONDONTWRITEBYTECODE","1")
        .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    #[cfg(unix)] { use std::os::unix::process::CommandExt; cmd.as_std_mut().process_group(0); }
    let mut child=cmd.spawn().map_err(|e|e.to_string())?;
    let _group=ProcessGroup(child.id().ok_or("Missing process ID")?);
    let stdout=child.stdout.take().unwrap(); let stderr=child.stderr.take().unwrap();
    tokio::time::timeout(Duration::from_secs(seconds), async {
        let (status,out,err)=tokio::join!(child.wait(),drain(stdout),drain(stderr));
        Ok((status.map_err(|e|e.to_string())?.code().unwrap_or(-1),tail(&format!("{out}{err}"))))
    }).await.map_err(|_|format!("Check timed out after {seconds}s"))?
}
pub async fn check(root: &Path, task: &Task, answer: &str) -> Result<(bool,String),String> {
    let (code,mut output)=command(root,&task.check.command,task.timeout_sec.min(60)).await?;
    let mut passed=code==task.check.expect_exit;
    for extra in &task.extra_checks {
        if !inside(root,&extra.file) { return Err("Check path escaped eval workdir".into()); }
        if !std::fs::read_to_string(root.join(&extra.file)).is_ok_and(|s|s.contains(&extra.contains)) {
            passed=false; output.push_str(&format!("\nMissing {:?} in {}",extra.contains,extra.file));
        }
    }
    for token in &task.answer_contains {
        if !answer.contains(token) { passed=false; output.push_str(&format!("\nAnswer missing {token:?}")); }
    }
    Ok((passed,tail(&output)))
}
pub async fn materialize(root: &Path, task: &Task) -> Result<(),String> {
    std::fs::create_dir_all(root).map_err(|e|e.to_string())?;
    for (name,text) in &super::model::suite().repos[&task.repo] {
        if !inside(root,name) { return Err("Invalid fixture path".into()); }
        let path=root.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e|e.to_string())?;
        std::fs::write(path,text).map_err(|e|e.to_string())?;
    }
    let (exit,out)=command(root,"git init -q && git add . && git -c user.name=Eval -c user.email=eval@localhost -c commit.gpgsign=false commit -qm fixture",30).await?;
    if exit!=0 { return Err(out); }
    for setup in &task.setup {
        let (exit,out)=command(root,setup,task.timeout_sec.min(60)).await?;
        if exit!=0 { return Err(format!("Setup failed: {out}")); }
    }
    Ok(())
}
pub struct Server { pub url: String, task: tokio::task::JoinHandle<()> }
impl Drop for Server { fn drop(&mut self) { self.task.abort(); } }
pub async fn serve(root: &Path) -> Result<Server,String> {
    use tokio::{net::TcpListener, io::AsyncWriteExt};
    let listener=TcpListener::bind("127.0.0.1:0").await.map_err(|e|e.to_string())?;
    let url=format!("http://{}",listener.local_addr().map_err(|e|e.to_string())?);
    let root=root.to_owned();
    let task=tokio::spawn(async move {
        while let Ok((mut stream,_))=listener.accept().await {
            let mut request=[0;2048];
            let Ok(Ok(n))=tokio::time::timeout(Duration::from_secs(2),stream.read(&mut request)).await else { continue };
            let request=String::from_utf8_lossy(&request[..n]);
            let path=request.split_whitespace().nth(1).unwrap_or("/");
            let file=match path { "/"|"/index.html"=>"index.html","/app.js"=>"app.js",_=>continue };
            if !inside(&root,file) { continue; }
            let Ok(body)=std::fs::read(root.join(file)) else { continue };
            let mime=if file.ends_with(".js") {"text/javascript"} else {"text/html"};
            let header=format!("HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len());
            let _=tokio::time::timeout(Duration::from_secs(2),async {
                stream.write_all(header.as_bytes()).await?;
                stream.write_all(&body).await
            }).await;
        }
    });
    Ok(Server {url,task})
}

