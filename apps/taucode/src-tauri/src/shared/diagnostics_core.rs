use std::sync::atomic::{AtomicU64, Ordering};
pub(crate) static PROCESS_START: AtomicU64 = AtomicU64::new(0);
use crate::types::AppSettings;
use serde_json::json;
use std::path::PathBuf;
use taucode_diag_mcp::writer;
pub(crate) fn configure(dir: PathBuf, settings: &AppSettings) -> Result<(), String> {
    writer::configure(
        settings.features().diagnostics,
        dir,
        settings.diagnostics.clone(),
    )?;
    if writer::enabled() {
        writer::set_context(
            json!({"appVersion":env!("CARGO_PKG_VERSION"),"os":std::env::consts::OS}),
        );
        #[cfg(target_os = "macos")]
        if PROCESS_START.load(Ordering::Relaxed) == 0 {
            // Query process launch time only when recording is enabled.
            unsafe {
                let mut info: libc::proc_bsdinfo = std::mem::zeroed();
                let size = std::mem::size_of_val(&info) as i32;
                if libc::proc_pidinfo(
                    std::process::id() as i32,
                    libc::PROC_PIDTBSDINFO,
                    0,
                    &mut info as *mut _ as *mut _,
                    size,
                ) == size
                {
                    PROCESS_START.store(
                        info.pbi_start_tvsec * 1000 + info.pbi_start_tvusec / 1000,
                        Ordering::Relaxed,
                    );
                }
            }
        }
    }
    Ok(())
}
