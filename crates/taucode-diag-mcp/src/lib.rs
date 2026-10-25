#[path = "../../../apps/taucode/src-tauri/src/diagnostics/core.rs"]
pub mod core;
#[path = "../../../apps/taucode/src-tauri/src/diagnostics/observe.rs"]
pub mod observe;
pub mod server;
#[path = "../../../apps/taucode/src-tauri/src/diagnostics/writer.rs"]
pub mod writer;
pub use server::run;
