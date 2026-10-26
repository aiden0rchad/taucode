mod codegraph;
mod codegraph_commands;
mod checkpoint_commands;
mod connectors;
mod providers;
#[cfg(desktop)]
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Manager;
#[cfg(desktop)]
use tauri::RunEvent;
#[cfg(target_os = "macos")]
use tauri::WindowEvent;

mod backend;
mod codex;
mod daemon_binary;
mod diagnostics;
mod dictation;
mod event_sink;
mod files;
mod git;
mod git_utils;
mod local_usage;
#[cfg(desktop)]
mod menu;
#[cfg(not(desktop))]
#[path = "menu_mobile.rs"]
mod menu;
mod notifications;
mod prompts;
mod remote_backend;
mod rules;
mod settings;
mod shared;
mod state;
mod chats;
mod work;
pub mod evals;
mod import_commands;
mod storage;
mod tailscale;
#[cfg(desktop)]
mod terminal;
#[cfg(not(desktop))]
#[path = "terminal_mobile.rs"]
mod terminal;
mod tray;
mod types;
mod utils;
mod window;
mod window_material;
#[cfg(test)]
mod multiwebview_tests;
mod themes;
mod workspaces;

#[cfg(desktop)]
static EXIT_CLEANUP_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

#[cfg(desktop)]
fn keep_daemon_running_after_close(app_handle: &tauri::AppHandle) -> bool {
    let state = app_handle.state::<state::AppState>();
    tauri::async_runtime::block_on(async {
        state
            .app_settings
            .lock()
            .await
            .keep_daemon_running_after_app_close
    })
}

#[cfg(desktop)]
async fn stop_managed_daemons_for_exit(app_handle: tauri::AppHandle) {
    let state = app_handle.state::<state::AppState>();
    let _ = tailscale::tailscale_daemon_stop(state).await;
}

#[tauri::command]
fn is_mobile_runtime() -> bool {
    cfg!(any(target_os = "ios", target_os = "android"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "linux")]
    {
        // Avoid WebKit compositing issues on NVIDIA Linux setups (GBM buffer errors).
        if std::env::var_os("__NV_PRIME_RENDER_OFFLOAD").is_none() {
            std::env::set_var("__NV_PRIME_RENDER_OFFLOAD", "1");
        }
        let is_wayland = std::env::var("XDG_SESSION_TYPE")
            .map(|session| session.eq_ignore_ascii_case("wayland"))
            .unwrap_or(false)
            || std::env::var_os("WAYLAND_DISPLAY").is_some();
        let has_nvidia = std::path::Path::new("/proc/driver/nvidia/version").exists();
        if is_wayland && has_nvidia && std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none()
        {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
        let is_x11 = !is_wayland && std::env::var_os("DISPLAY").is_some();
        // Work around sporadic blank WebKitGTK renders on X11 by disabling compositing mode.
        // Keep Wayland untouched because this can interfere with input behavior on some setups.
        if is_x11 && std::env::var_os("WEBKIT_DISABLE_COMPOSITING_MODE").is_none() {
            std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        }
    }

    #[cfg(desktop)]
    let builder = tauri::Builder::default()
        .manage(menu::MenuItemRegistry::<tauri::Wry>::default())
        .manage(tray::TrayState::default())
        .on_menu_event(menu::handle_menu_event)
        .enable_macos_default_menu(false)
        .menu(menu::build_menu);

    #[cfg(not(desktop))]
    let builder = tauri::Builder::default();

    let builder = builder
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _, event| computer_commands::shortcut(app, event))
                .build(),
        )
        .on_window_event(|window, event| {
            if matches!(event, WindowEvent::Destroyed) {
                window_material::cleanup(window);
            }
            if window.label() != "main" {
                return;
            }
            if matches!(event, WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. }) {
                let _ = window::resize_main(window);
            }
            #[cfg(target_os = "macos")]
            if matches!(event, WindowEvent::Destroyed) {
                window::remove_main_focus(window);
            }
            #[cfg(target_os = "macos")]
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(|app| {
            #[cfg(target_os = "macos")]
            if let Some(main) = app.get_webview("main") {
                window::install_main_focus(&main)?;
            }
            if let Err(error) = themes::watch(app.handle()) { eprintln!("Theme watcher unavailable: {error}"); }
            let _ = shared::engine_runtime::RESOURCES.set(app.path().resource_dir()?);
            let state = state::AppState::load(&app.handle());
            diagnostics::initialize(app.handle(), &tauri::async_runtime::block_on(state.app_settings.lock()).clone()).map_err(std::io::Error::other)?;
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            let _ = state.browser.native.set(std::sync::Arc::new(shared::browser::native::NativeBackend::new(app.handle().clone(), state.browser.native_state.clone(), app.path().app_data_dir()?.join("settings.json"))));
            #[cfg(desktop)]
            tauri::async_runtime::block_on(state.browser.serve()).map_err(std::io::Error::other)?;
            #[cfg(desktop)]
            {
                let mut signal = tauri::async_runtime::block_on(async {
                    shared::browser::signal::Shutdown::new()
                })
                .map_err(std::io::Error::other)?;
                let browser = state.browser.clone();
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    signal.wait().await;
                    browser.shutdown().await;
                    handle.exit(0);
                });
            }
            app.manage(state);
            #[cfg(desktop)]
            computer_commands::install(app.handle()).map_err(std::io::Error::other)?;
            #[cfg(target_os = "macos")]
            {
                let tray_state = app.state::<tray::TrayState>();
                tray::initialize(&app.handle(), tray_state.inner())?;
            }
            #[cfg(target_os = "windows")]
            {
                if let Some(main_window) = app.get_window("main") {
                    let _ = main_window.set_decorations(false);
                    // Keep menu accelerators wired while suppressing a visible native menu bar.
                    let _ = main_window.hide_menu();
                }
            }
            #[cfg(desktop)]
            {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let state = app_handle.state::<state::AppState>();
                    let settings = state.app_settings.lock().await.clone();
                    if matches!(
                        settings.remote_backend_provider,
                        crate::types::RemoteBackendProvider::Tcp
                    ) {
                        if matches!(settings.backend_mode, crate::types::BackendMode::Remote) {
                            // Remote mode: ensure daemon is up and version-current.
                            let state = app_handle.state::<state::AppState>();
                            let _ = tailscale::tailscale_daemon_start(state).await;
                        } else {
                            // Local mode: only enforce version if daemon is already running.
                            let state = app_handle.state::<state::AppState>();
                            if let Ok(status) = tailscale::tailscale_daemon_status(state).await {
                                if matches!(status.state, crate::types::TcpDaemonState::Running) {
                                    let state = app_handle.state::<state::AppState>();
                                    let _ = tailscale::tailscale_daemon_start(state).await;
                                }
                            }
                        }
                    }
                });
            }
            #[cfg(target_os = "ios")]
            {
                if let Some(main_webview) = app.get_webview("main") {
                    let _ = window::configure_ios_webview_edge_to_edge(&main_webview);
                }
            }
            Ok(())
        });

    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_window_state::Builder::default().build());

    let app = builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(|invoke| {
            if invoke.message.webview_ref().label().starts_with("browser-") { invoke.resolver.reject("Browser content cannot invoke app commands"); return true; }
            let handler: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool = tauri::generate_handler![
            themes::themes_list, themes::themes_save, themes::themes_delete,
            themes::themes_import, themes::themes_export, themes::themes_open_folder, themes::themes_reveal_file,
            themes::window_set_material,
            work::work_request,
            evals::eval_request,
            work::work_attachment,
            work::work_output,
            chats::chat_request,
            import_commands::import_request,
            chats::read_chat_attachment,
            chats::set_window_mode,
            providers::engine_runtime,
            providers::setup_status,
            providers::engine_test,
            computer_commands::computer_request,
            computer_commands::computer_request_permissions,
            browser_commands::browser_request,
            browser_commands::browser_native_attach,
            browser_commands::browser_native_bounds,
            browser_commands::browser_native_navigate,
            browser_commands::browser_native_back,
            browser_commands::browser_native_forward,
            browser_commands::browser_native_reload,
            browser_commands::browser_native_stop,
            browser_commands::browser_native_close,
            browser_commands::browser_native_snapshot_image,
            browser_commands::browser_subscribe,
            browser_commands::browser_unsubscribe,
            checkpoint_commands::checkpoint_request,
            codegraph_commands::codegraph_status,
            codegraph_commands::codegraph_rebuild,
            providers::provider_set_api_key,
            providers::provider_has_api_key,
            providers::provider_delete_api_key,
            providers::provider_test_and_fetch,
            connectors::connector_request,
            diagnostics::diagnostics_request,
            settings::get_app_settings,
            settings::update_app_settings,
            settings::get_codex_config_path,
            files::file_read,
            files::file_write,
            files::read_image_as_data_url,
            files::write_text_file,
            codex::get_config_model,
            menu::menu_set_accelerators,
            tray::set_tray_recent_threads,
            tray::set_tray_session_usage,
            codex::codex_doctor,
            codex::codex_update,
            workspaces::list_workspaces,
            workspaces::is_workspace_path_dir,
            workspaces::add_workspace,
            workspaces::add_workspace_from_git_url,
            workspaces::add_clone,
            workspaces::add_worktree,
            workspaces::worktree_setup_status,
            workspaces::worktree_setup_mark_ran,
            workspaces::remove_workspace,
            workspaces::remove_worktree,
            workspaces::rename_worktree,
            workspaces::rename_worktree_upstream,
            workspaces::apply_worktree_changes,
            workspaces::update_workspace_settings,
            workspaces::set_workspace_runtime_codex_args,
            codex::start_thread,
            codex::send_user_message,
            codex::turn_steer,
            codex::turn_interrupt,
            codex::start_review,
            codex::respond_to_server_request,
            codex::remember_approval_rule,
            codex::generate_commit_message,
            codex::generate_run_metadata,
            codex::generate_agent_description,
            codex::resume_thread,
            codex::read_thread,
            codex::create_side_chat,
            codex::get_subagent_transcript,
            codex::list_thread_history,
            codex::thread_live_subscribe,
            codex::thread_live_unsubscribe,
            codex::fork_thread,
            codex::list_threads,
            codex::list_mcp_server_status,
            codex::archive_thread,
            codex::compact_thread,
            codex::set_thread_name,
            codex::set_thread_access_mode,
            codex::collaboration_mode_list,
            workspaces::connect_workspace,
            git::get_git_status,
            git::init_git_repo,
            git::create_github_repo,
            git::list_git_roots,
            git::get_git_diffs,
            git::get_git_log,
            git::get_git_commit_diff,
            git::get_git_remote,
            git::stage_git_file,
            git::stage_git_all,
            git::unstage_git_file,
            git::revert_git_file,
            git::revert_git_all,
            git::commit_git,
            git::push_git,
            git::pull_git,
            git::fetch_git,
            git::sync_git,
            git::get_github_issues,
            git::get_github_pull_requests,
            git::get_github_pull_request_diff,
            git::get_github_pull_request_comments,
            git::checkout_github_pull_request,
            workspaces::list_workspace_files,
            workspaces::read_workspace_file,
            workspaces::open_workspace_in,
            workspaces::get_open_app_icon,
            git::list_git_branches,
            git::checkout_git_branch,
            git::create_git_branch,
            codex::model_list,
            codex::list_models,
            codex::experimental_feature_list,
            codex::set_codex_feature_flag,
            codex::get_agents_settings,
            codex::set_agents_core_settings,
            codex::create_agent,
            codex::update_agent,
            codex::delete_agent,
            codex::read_agent_config_toml,
            codex::write_agent_config_toml,
            codex::skills_list,
            prompts::prompts_list,
            prompts::prompts_create,
            prompts::prompts_update,
            prompts::prompts_delete,
            prompts::prompts_move,
            prompts::prompts_workspace_dir,
            prompts::prompts_global_dir,
            terminal::terminal_open,
            terminal::terminal_write,
            terminal::terminal_resize,
            terminal::terminal_close,
            dictation::dictation_model_status,
            dictation::dictation_download_model,
            dictation::dictation_cancel_download,
            dictation::dictation_remove_model,
            dictation::dictation_start,
            dictation::dictation_request_permission,
            dictation::dictation_stop,
            dictation::dictation_cancel,
            local_usage::local_usage_snapshot,
            notifications::is_macos_debug_build,
            notifications::app_build_type,
            notifications::send_notification_fallback,
            tailscale::tailscale_status,
            tailscale::tailscale_daemon_command_preview,
            tailscale::tailscale_daemon_start,
            tailscale::tailscale_daemon_stop,
            tailscale::tailscale_daemon_status,
            is_mobile_runtime
        ];
            handler(invoke)
        })
        .build(tauri::generate_context!())
        .expect("error while running tauri application");

    app.run(|app_handle, event| {
        if let tauri::RunEvent::Exit = event {
            #[cfg(desktop)]
            if let Err(error) = window::save_main_state(app_handle) {
                eprintln!("Cannot save main window state: {error}");
            }
            taucode_diag_mcp::writer::stop();
            let state = app_handle.state::<state::AppState>();
            state.computer.stop();
            tauri::async_runtime::block_on(state.browser.shutdown());
        }
        #[cfg(desktop)]
        if let RunEvent::ExitRequested { api, .. } = event {
            if !EXIT_CLEANUP_IN_PROGRESS.load(Ordering::SeqCst)
                && !keep_daemon_running_after_close(app_handle)
            {
                api.prevent_exit();
                EXIT_CLEANUP_IN_PROGRESS.store(true, Ordering::SeqCst);
                let app_handle = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    stop_managed_daemons_for_exit(app_handle.clone()).await;
                    app_handle.exit(0);
                });
            }
            return;
        }

        #[cfg(target_os = "macos")]
        if let RunEvent::Reopen { .. } = event {
            if let Some(window) = app_handle.get_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
    });
}

mod browser_commands;
mod computer_commands;
pub fn computer_mcp_cli(args: Vec<std::ffi::OsString>) -> Result<(), String> {
    shared::computer::mcp_cli(args)
}
pub fn browser_mcp_cli(args: Vec<std::ffi::OsString>) -> Result<(), String> {
    shared::browser::mcp_cli(args)
}
