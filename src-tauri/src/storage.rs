use crate::paths::AppPaths;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LocalSettings {
    pub port: u16,
    pub context_length: u32,
    pub temperature: f32,
    pub max_tokens: u32,
    pub remote_base_url: String,
    pub remote_api_key: String,
    pub remote_model_name: String,
}

impl Default for LocalSettings {
    fn default() -> Self {
        Self {
            port: 8080,
            context_length: 2048,
            temperature: 0.7,
            max_tokens: 512,
            remote_base_url: String::new(),
            remote_api_key: String::new(),
            remote_model_name: String::new(),
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

fn sanitize(mut s: LocalSettings) -> LocalSettings {
    if s.port < 1024 {
        s.port = 8080;
    }
    s.context_length = s.context_length.clamp(512, 32768);
    s.temperature = s.temperature.clamp(0.0, 2.0);
    s.max_tokens = s.max_tokens.clamp(16, 8192);
    s
}

pub fn load_settings(paths: &AppPaths) -> Result<LocalSettings, String> {
    let path = &paths.settings_file;
    if !Path::new(path).exists() {
        let defaults = LocalSettings::default();
        save_settings(paths, defaults.clone())?;
        return Ok(defaults);
    }

    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;

    match serde_json::from_str::<LocalSettings>(&text) {
        Ok(s) => {
            let s = sanitize(s);
            save_settings(paths, s.clone())?;
            Ok(s)
        }
        Err(e) => {
            let backup = format!("{path}.bad");
            let _ = fs::write(&backup, &text);
            let defaults = LocalSettings::default();
            save_settings(paths, defaults.clone())?;
            Err(format!(
                "Settings were malformed and reset. Backup: {backup}. Error: {e}"
            ))
        }
    }
}

pub fn save_settings(paths: &AppPaths, settings: LocalSettings) -> Result<(), String> {
    let settings = sanitize(settings);
    let json = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    fs::write(&paths.settings_file, json).map_err(|e| e.to_string())
}

pub fn load_chats(paths: &AppPaths) -> Result<Vec<StoredChat>, String> {
    let path = &paths.chats_file;
    if !Path::new(path).exists() {
        fs::write(path, "[]").map_err(|e| e.to_string())?;
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

pub fn save_chats(paths: &AppPaths, chats: Vec<StoredChat>) -> Result<(), String> {
    let json = serde_json::to_string_pretty(&chats).map_err(|e| e.to_string())?;
    fs::write(&paths.chats_file, json).map_err(|e| e.to_string())
}
