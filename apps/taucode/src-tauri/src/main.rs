// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod proxy_cli;

fn main() {
    proxy_cli::dispatch();
    if let Err(err) = fix_path_env::fix() {
        eprintln!("Failed to sync PATH from shell: {err}");
    }
    codex_monitor_lib::run()
}
