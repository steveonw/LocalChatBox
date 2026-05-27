mod diagnostics;
mod engine;
mod hardware;
mod models;
mod paths;
mod runtime;
mod storage;

use std::sync::Mutex;
use tauri::Manager;

struct AppState {
    runtime: Mutex<runtime::RuntimeManager>,
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
fn scan_hardware() -> Result<hardware::HardwareProfile, String> {
    let paths = app_paths()?;
    hardware::scan(&paths)
}

#[tauri::command]
fn scan_models() -> Result<Vec<models::ModelInfo>, String> {
    let paths = app_paths()?;
    models::scan(&paths)
}

#[tauri::command]
fn scan_model_registry() -> Result<models::ModelRegistry, String> {
    let paths = app_paths()?;
    models::scan_registry(&paths)
}

#[tauri::command]
fn probe_runtime_manifest() -> Result<engine::EngineManifest, String> {
    let paths = app_paths()?;
    engine::probe_runtime_manifest(&paths)
}

#[tauri::command]
fn run_doctor() -> Result<diagnostics::DoctorReport, String> {
    let paths = app_paths()?;
    diagnostics::run(&paths)
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
    runtime::start_with_fallbacks(&state.runtime, &paths, request).await
}

#[tauri::command]
async fn switch_local_model(
    state: tauri::State<'_, AppState>,
    request: runtime::RuntimeStartRequest,
) -> Result<runtime::RuntimeStatus, String> {
    let paths = app_paths()?;
    runtime::switch_model(&state.runtime, &paths, request).await
}

#[tauri::command]
fn stop_runtime(state: tauri::State<'_, AppState>) -> Result<runtime::RuntimeStatus, String> {
    let paths = app_paths()?;
    let mut runtime = state
        .runtime
        .lock()
        .map_err(|_| "Runtime manager lock was poisoned.".to_string())?;
    Ok(runtime.stop(&paths))
}

#[tauri::command]
fn runtime_status(state: tauri::State<'_, AppState>) -> Result<runtime::RuntimeStatus, String> {
    let paths = app_paths()?;
    let mut runtime = state
        .runtime
        .lock()
        .map_err(|_| "Runtime manager lock was poisoned.".to_string())?;
    Ok(runtime.status(&paths))
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
        let mut runtime = state
            .runtime
            .lock()
            .map_err(|_| "Runtime manager lock was poisoned.".to_string())?;
        runtime.status(&paths)
    };

    runtime::send_chat_request(status, request).await
}

fn stop_runtime_for_shutdown(app_handle: &tauri::AppHandle) {
    let Ok(paths) = app_paths() else {
        return;
    };

    let state = app_handle.state::<AppState>();
    if let Ok(mut runtime) = state.runtime.lock() {
        let _ = runtime.stop(&paths);
    }
}

fn main() {
    tauri::Builder::default()
        .manage(AppState {
            runtime: Mutex::new(runtime::RuntimeManager::default()),
        })
        .setup(|_app| {
            let _ = paths::ensure_workspace();
            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::CloseRequested { .. } => {
                    stop_runtime_for_shutdown(&window.app_handle());
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            initialize_workspace,
            reveal_models_folder,
            scan_hardware,
            scan_models,
            scan_model_registry,
            probe_runtime_manifest,
            run_doctor,
            load_settings,
            save_settings,
            load_chats,
            save_chats,
            start_runtime,
            switch_local_model,
            stop_runtime,
            runtime_status,
            read_runtime_log,
            send_chat
        ])
        .run(tauri::generate_context!())
        .expect("error while running LocalChatBox");
}
