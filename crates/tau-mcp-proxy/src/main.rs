use std::{path::PathBuf, sync::Arc, time::Duration};

use rmcp::ServiceExt;
use tau_mcp_proxy::Proxy;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args_os().skip(1);
    anyhow::ensure!(
        args.next().as_deref() == Some(std::ffi::OsStr::new("--")),
        "usage: tau-mcp-proxy -- <upstream cmd> [args...]"
    );
    let proxy = Arc::new(
        Proxy::new(
            args.collect(),
            std::env::var_os("TAUCODE_STATE_DIR").map(PathBuf::from),
        )
        .await?,
    );
    let result = async {
        let service = proxy.clone().serve(rmcp::transport::stdio()).await?;
        let waiting = service.waiting();
        tokio::pin!(waiting);
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            tokio::select! {
                result = &mut waiting => { result?; break; }
                _ = interval.tick() => proxy.reap_idle().await,
            }
        }
        anyhow::Ok(())
    }
    .await;
    proxy.shutdown().await;
    result
}
