use crate::types::WorkspaceEntry;
use std::path::PathBuf;
use std::sync::OnceLock;
static MANAGED_HOME: OnceLock<PathBuf> = OnceLock::new();
pub(crate) fn set_app_data_dir(path: &std::path::Path) {
    let _ = MANAGED_HOME.set(path.join("engines/codex-home"));
}
pub(crate) fn resolve_workspace_codex_home(
    _: &WorkspaceEntry,
    _: Option<&WorkspaceEntry>,
) -> Option<PathBuf> {
    resolve_default_codex_home()
}
pub(crate) fn resolve_default_codex_home() -> Option<PathBuf> {
    MANAGED_HOME.get().cloned()
}

pub(crate) fn resolve_home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}
