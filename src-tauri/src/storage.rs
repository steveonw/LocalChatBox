use crate::paths::AppPaths;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LocalSettings {
    pub port: u16,
    pub context_length: u32,
    pub gpu_layers: u32,
    pub preferred_runtime: String,
    pub runtime_mode: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub remote_base_url: String,
    pub remote_api_key: String,
    pub remote_model_name: String,
    pub first_run_complete: bool,
}

impl Default for LocalSettings {
    fn default() -> Self {
        Self {
            port: 8080,
            context_length: 2048,
            gpu_layers: 0,
            preferred_runtime: "auto".to_string(),
            runtime_mode: "auto".to_string(),
            temperature: 0.7,
            max_tokens: 512,
            remote_base_url: String::new(),
            remote_api_key: String::new(),
            remote_model_name: String::new(),
            first_run_complete: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredChat {
    pub id: String,
    pub title: String,
    pub model: String,
    pub created_at: String,
    pub updated_at: String,
    pub messages: Vec<ChatMessage>,
}

fn settings_path(paths: &AppPaths) -> String {
    Path::new(&paths.data_dir)
        .join("settings.json")
        .to_string_lossy()
        .to_string()
}

fn chats_path(paths: &AppPaths) -> String {
    Path::new(&paths.data_dir)
        .join("chats.json")
        .to_string_lossy()
        .to_string()
}

fn sanitize_settings(mut settings: LocalSettings) -> LocalSettings {
    if settings.port < 1024 {
        settings.port = 8080;
    }

    settings.context_length = settings.context_length.clamp(512, 32768);
    settings.gpu_layers = settings.gpu_layers.min(256);
    settings.temperature = settings.temperature.clamp(0.0, 2.0);
    settings.max_tokens = settings.max_tokens.clamp(16, 8192);

    let allowed_runtime = ["auto", "cuda", "cpu-avx2", "cpu-avx", "cpu-basic"];
    if !allowed_runtime.contains(&settings.preferred_runtime.as_str()) {
        settings.preferred_runtime = "auto".to_string();
    }

    let allowed_mode = ["auto", "router", "classic"];
    if !allowed_mode.contains(&settings.runtime_mode.as_str()) {
        settings.runtime_mode = "auto".to_string();
    }

    settings
}

pub fn load_settings(paths: &AppPaths) -> Result<LocalSettings, String> {
    let path = settings_path(paths);
    if !Path::new(&path).exists() {
        let settings = LocalSettings::default();
        save_settings(paths, settings.clone())?;
        return Ok(settings);
    }

    let text = fs::read_to_string(&path).map_err(|err| err.to_string())?;
    let parsed: Result<LocalSettings, _> = serde_json::from_str(&text);

    match parsed {
        Ok(settings) => {
            let settings = sanitize_settings(settings);
            save_settings(paths, settings.clone())?;
            Ok(settings)
        }
        Err(err) => {
            let backup = format!("{path}.bad");
            let _ = fs::write(&backup, text);
            let settings = LocalSettings::default();
            save_settings(paths, settings.clone())?;
            Err(format!(
                "Settings file was malformed and has been reset. Backup saved to {backup}. Parse error: {err}"
            ))
        }
    }
}

pub fn save_settings(paths: &AppPaths, settings: LocalSettings) -> Result<(), String> {
    fs::create_dir_all(&paths.data_dir).map_err(|err| err.to_string())?;
    let settings = sanitize_settings(settings);
    let json = serde_json::to_string_pretty(&settings).map_err(|err| err.to_string())?;
    fs::write(settings_path(paths), json).map_err(|err| err.to_string())
}

pub fn load_chats(paths: &AppPaths) -> Result<Vec<StoredChat>, String> {
    let path = chats_path(paths);
    if !Path::new(&path).exists() {
        fs::write(&path, "[]").map_err(|err| err.to_string())?;
        return Ok(Vec::new());
    }

    let text = fs::read_to_string(&path).map_err(|err| err.to_string())?;
    let chats: Vec<StoredChat> = serde_json::from_str(&text).map_err(|err| err.to_string())?;
    Ok(chats)
}

pub fn save_chats(paths: &AppPaths, chats: Vec<StoredChat>) -> Result<(), String> {
    fs::create_dir_all(&paths.data_dir).map_err(|err| err.to_string())?;
    let json = serde_json::to_string_pretty(&chats).map_err(|err| err.to_string())?;
    fs::write(chats_path(paths), json).map_err(|err| err.to_string())
}
