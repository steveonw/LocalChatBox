use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Serialize)]
pub struct AppPaths {
    pub root: String,
    pub models_dir: String,
    pub runtime_win_dir: String,
    pub data_dir: String,
    pub logs_dir: String,
    pub runtime_log: String,
    pub engines_dir: String,
    pub llama_engine_dir: String,
    pub engine_manifest: String,
    pub router_preset: String,
    pub model_registry: String,
    pub diagnostics_dir: String,
    pub doctor_report: String,
}

fn normalize(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Returns the portable application root.
/// In release this is the folder beside LocalChatBox.exe.
/// In development it resolves back to the project root, even when Tauri starts in src-tauri.
pub fn app_root() -> Result<PathBuf, String> {
    if cfg!(debug_assertions) {
        let current = std::env::current_dir().map_err(|err| err.to_string())?;
        if current.file_name().and_then(|name| name.to_str()) == Some("src-tauri") {
            return current
                .parent()
                .map(Path::to_path_buf)
                .ok_or_else(|| "Could not resolve project root from src-tauri.".to_string());
        }
        return Ok(current);
    }

    let exe = std::env::current_exe().map_err(|err| err.to_string())?;
    exe.parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "Could not resolve application folder.".to_string())
}

pub fn workspace_paths() -> Result<AppPaths, String> {
    let root = app_root()?;
    let models_dir = root.join("models");
    let runtime_win_dir = root.join("runtime").join("win");
    let data_dir = root.join("data");
    let logs_dir = root.join("logs");
    let runtime_log = logs_dir.join("runtime.log");
    let engines_dir = data_dir.join("engines");
    let llama_engine_dir = engines_dir.join("llama.cpp");
    let engine_manifest = llama_engine_dir.join("manifest.json");
    let router_preset = llama_engine_dir.join("router.preset.ini");
    let model_registry = data_dir.join("models").join("registry.json");
    let diagnostics_dir = data_dir.join("diagnostics");
    let doctor_report = diagnostics_dir.join("doctor-report.json");

    Ok(AppPaths {
        root: normalize(&root),
        models_dir: normalize(&models_dir),
        runtime_win_dir: normalize(&runtime_win_dir),
        data_dir: normalize(&data_dir),
        logs_dir: normalize(&logs_dir),
        runtime_log: normalize(&runtime_log),
        engines_dir: normalize(&engines_dir),
        llama_engine_dir: normalize(&llama_engine_dir),
        engine_manifest: normalize(&engine_manifest),
        router_preset: normalize(&router_preset),
        model_registry: normalize(&model_registry),
        diagnostics_dir: normalize(&diagnostics_dir),
        doctor_report: normalize(&doctor_report),
    })
}

pub fn ensure_workspace() -> Result<AppPaths, String> {
    let paths = workspace_paths()?;

    for folder in [
        &paths.models_dir,
        &paths.runtime_win_dir,
        &paths.data_dir,
        &paths.logs_dir,
        &paths.engines_dir,
        &paths.llama_engine_dir,
        &paths.diagnostics_dir,
    ] {
        fs::create_dir_all(folder).map_err(|err| format!("Could not create {folder}: {err}"))?;
    }

    let model_registry_parent = Path::new(&paths.model_registry)
        .parent()
        .ok_or_else(|| "Could not resolve model registry folder.".to_string())?;
    fs::create_dir_all(model_registry_parent)
        .map_err(|err| format!("Could not create model registry folder: {err}"))?;

    let models_hint = Path::new(&paths.models_dir).join("put-gguf-models-here.txt");
    if !models_hint.exists() {
        fs::write(
            &models_hint,
            "Put GGUF model files in this folder, or use LocalChatBox's import/download flow when it is enabled.\n",
        )
        .map_err(|err| format!("Could not write models hint file: {err}"))?;
    }

    let runtime_hint = Path::new(&paths.runtime_win_dir).join("README.md");
    if !runtime_hint.exists() {
        fs::write(
            &runtime_hint,
            "# Runtime folder\n\nPlace llama.cpp `llama-server` Windows binaries here for developer builds.\n\nRecommended filenames:\n\n- llama-server-cuda.exe\n- llama-server-cpu-avx2.exe\n- llama-server-cpu-avx.exe\n- llama-server-cpu-basic.exe\n- llama-server.exe\n\nPublic installer builds should bundle at least one CPU runtime so normal users do not need to assemble this folder by hand.\n",
        )
        .map_err(|err| format!("Could not write runtime README: {err}"))?;
    }

    let log_path = Path::new(&paths.runtime_log);
    if !log_path.exists() {
        fs::File::create(log_path).map_err(|err| format!("Could not create runtime log: {err}"))?;
    }

    Ok(paths)
}

pub fn reveal_models_folder() -> Result<(), String> {
    let paths = ensure_workspace()?;

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(paths.models_dir)
            .spawn()
            .map_err(|err| format!("Could not open models folder: {err}"))?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(paths.models_dir)
            .spawn()
            .map_err(|err| format!("Could not open models folder: {err}"))?;
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(paths.models_dir)
            .spawn()
            .map_err(|err| format!("Could not open models folder: {err}"))?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err("Opening the models folder is not supported on this platform.".to_string())
}
