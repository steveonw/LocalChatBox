use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Serialize)]
pub struct AppPaths {
    pub data_root: String,
    pub models_dir: String,
    pub settings_file: String,
    pub chats_file: String,
    pub runtime_log: String,
}

fn normalize(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn data_root_path() -> Result<PathBuf, String> {
    #[cfg(debug_assertions)]
    {
        let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
        let root = if cwd.file_name().and_then(|n| n.to_str()) == Some("src-tauri") {
            cwd.parent()
                .map(|p| p.to_path_buf())
                .ok_or_else(|| "Cannot resolve project root from src-tauri".to_string())?
        } else {
            cwd
        };
        return Ok(root.join("data"));
    }

    #[cfg(not(debug_assertions))]
    std::env::var("LOCALAPPDATA")
        .map(|p| PathBuf::from(p).join("LocalChatBox"))
        .map_err(|_| "LOCALAPPDATA environment variable is not set".to_string())
}

pub fn app_paths() -> Result<AppPaths, String> {
    let root = data_root_path()?;
    Ok(AppPaths {
        data_root: normalize(&root),
        models_dir: normalize(&root.join("models")),
        settings_file: normalize(&root.join("settings.json")),
        chats_file: normalize(&root.join("chats.json")),
        runtime_log: normalize(&root.join("runtime.log")),
    })
}

pub fn ensure_workspace() -> Result<AppPaths, String> {
    let paths = app_paths()?;

    for dir in [&paths.data_root, &paths.models_dir] {
        fs::create_dir_all(dir).map_err(|e| format!("Could not create {dir}: {e}"))?;
    }

    let hint = Path::new(&paths.models_dir).join("put-gguf-models-here.txt");
    if !hint.exists() {
        let _ = fs::write(&hint, "Put GGUF model files (.gguf) in this folder.\n");
    }

    Ok(paths)
}

/// Finds the bundled llama-server.exe sidecar.
/// In a release build, looks next to the exe in runtime/win/.
/// In a dev build, walks up from cwd until it finds runtime/win/llama-server.exe.
/// Returns a path even when the file is absent — start() will surface a clear error.
pub fn find_sidecar() -> PathBuf {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_default();

    let release_path = exe_dir.join("runtime").join("win").join("llama-server.exe");
    if release_path.exists() {
        return release_path;
    }

    let mut dir = std::env::current_dir().unwrap_or_default();
    for _ in 0..5 {
        let candidate = dir.join("runtime").join("win").join("llama-server.exe");
        if candidate.exists() {
            return candidate;
        }
        match dir.parent() {
            Some(parent) => dir = parent.to_path_buf(),
            None => break,
        }
    }

    exe_dir.join("runtime").join("win").join("llama-server.exe")
}

pub fn reveal_models_folder() -> Result<(), String> {
    let paths = ensure_workspace()?;

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(&paths.models_dir)
            .spawn()
            .map_err(|e| format!("Could not open models folder: {e}"))?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err("Opening the models folder is not supported on this platform.".to_string())
}
