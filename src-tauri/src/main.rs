mod models;
mod paths;
mod runtime;
mod storage;

use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

struct AppState {
    runtime: Mutex<runtime::RuntimeManager>,
    sidecar: PathBuf,
}

fn app_paths() -> Result<paths::AppPaths, String> {
    paths::ensure_workspace()
}

#[tauri::command]
fn initialize_workspace() -> Result<paths::AppPaths, String> {
    app_paths()
}

#[tauri::command]
fn reveal_models_folder() -> Result<(), String> {
    paths::reveal_models_folder()
}

#[tauri::command]
fn scan_models() -> Result<Vec<models::ModelInfo>, String> {
    let paths = app_paths()?;
    models::scan(&paths)
}

#[tauri::command]
fn load_settings() -> Result<storage::LocalSettings, String> {
    let paths = app_paths()?;
    storage::load_settings(&paths)
}

#[tauri::command]
fn save_settings(settings: storage::LocalSettings) -> Result<(), String> {
    let paths = app_paths()?;
    storage::save_settings(&paths, settings)
}

#[tauri::command]
fn load_chats() -> Result<Vec<storage::StoredChat>, String> {
    let paths = app_paths()?;
    storage::load_chats(&paths)
}

#[tauri::command]
fn save_chats(chats: Vec<storage::StoredChat>) -> Result<(), String> {
    let paths = app_paths()?;
    storage::save_chats(&paths, chats)
}

#[tauri::command]
async fn start_runtime(
    state: tauri::State<'_, AppState>,
    request: runtime::RuntimeStartRequest,
) -> Result<runtime::RuntimeStatus, String> {
    let paths = app_paths()?;
    runtime::start(&state.runtime, &paths, &state.sidecar, request).await
}

#[tauri::command]
fn stop_runtime(state: tauri::State<'_, AppState>) -> Result<runtime::RuntimeStatus, String> {
    let paths = app_paths()?;
    let mut rt = state
        .runtime
        .lock()
        .map_err(|_| "Runtime manager lock was poisoned.".to_string())?;
    Ok(rt.stop(&paths))
}

#[tauri::command]
fn runtime_status(state: tauri::State<'_, AppState>) -> Result<runtime::RuntimeStatus, String> {
    let paths = app_paths()?;
    let mut rt = state
        .runtime
        .lock()
        .map_err(|_| "Runtime manager lock was poisoned.".to_string())?;
    Ok(rt.status(&paths))
}

#[tauri::command]
fn read_runtime_log() -> Result<String, String> {
    let paths = app_paths()?;
    runtime::read_log(&paths)
}

#[tauri::command]
async fn send_chat(
    state: tauri::State<'_, AppState>,
    request: runtime::ChatRequest,
) -> Result<runtime::ChatResponse, String> {
    let paths = app_paths()?;
    let status = {
        let mut rt = state
            .runtime
            .lock()
            .map_err(|_| "Runtime manager lock was poisoned.".to_string())?;
        rt.status(&paths)
    };
    runtime::send_chat(status, request).await
}

fn stop_on_close(app_handle: &tauri::AppHandle) {
    let Ok(paths) = app_paths() else { return };
    let state = app_handle.state::<AppState>();
    if let Ok(mut rt) = state.runtime.lock() {
        let _ = rt.stop(&paths);
    }
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let sidecar = paths::find_sidecar();
            app.manage(AppState {
                runtime: Mutex::new(runtime::RuntimeManager::default()),
                sidecar,
            });
            let _ = paths::ensure_workspace();
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                stop_on_close(&window.app_handle());
            }
        })
        .invoke_handler(tauri::generate_handler![
            initialize_workspace,
            reveal_models_folder,
            scan_models,
            load_settings,
            save_settings,
            load_chats,
            save_chats,
            start_runtime,
            stop_runtime,
            runtime_status,
            read_runtime_log,
            send_chat
        ])
        .run(tauri::generate_context!())
        .expect("error while running LocalChatBox");
}
