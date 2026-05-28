use crate::paths::AppPaths;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
    pub path: String,
    pub size_gb: f64,
    pub compatibility: String,
    pub compatibility_label: String,
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

pub fn model_id_from_name(name: &str) -> String {
    let stem = Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(name)
        .to_lowercase();

    let mut id = String::new();
    let mut last_dash = false;
    for ch in stem.chars() {
        if ch.is_ascii_alphanumeric() {
            id.push(ch);
            last_dash = false;
        } else if !last_dash {
            id.push('-');
            last_dash = true;
        }
    }

    let id = id.trim_matches('-').to_string();
    if id.is_empty() {
        "local-model".to_string()
    } else if id.len() > 80 {
        id[..80].trim_matches('-').to_string()
    } else {
        id
    }
}

fn display_name_from_stem(stem: &str) -> String {
    stem.replace(['-', '_', '.'], " ")
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().to_string() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn compatibility_for_size(size_gb: f64) -> (&'static str, &'static str) {
    if size_gb <= 4.0 {
        ("good", "Good fit · 8 GB RAM")
    } else if size_gb <= 8.0 {
        ("slow", "May be slow · 16 GB RAM recommended")
    } else {
        ("too_large", "Too large for most systems")
    }
}

pub fn scan(paths: &AppPaths) -> Result<Vec<ModelInfo>, String> {
    let dir = Path::new(&paths.models_dir);
    if !dir.exists() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        return Ok(Vec::new());
    }

    let mut models = Vec::new();

    for entry in fs::read_dir(dir).map_err(|e| format!("Could not read models dir: {e}"))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();

        if path.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("gguf")) != Some(true) {
            continue;
        }

        let metadata = match entry.metadata() {
            Ok(m) if m.is_file() => m,
            _ => continue,
        };

        let file_name = entry.file_name().to_string_lossy().to_string();
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&file_name)
            .to_string();

        let size_gb = round2(metadata.len() as f64 / 1024.0 / 1024.0 / 1024.0);
        let (compat, compat_label) = compatibility_for_size(size_gb);

        models.push(ModelInfo {
            id: model_id_from_name(&file_name),
            display_name: display_name_from_stem(&stem),
            path: path.to_string_lossy().replace('\\', "/"),
            size_gb,
            compatibility: compat.to_string(),
            compatibility_label: compat_label.to_string(),
        });
    }

    models.sort_by(|a, b| {
        a.size_gb
            .partial_cmp(&b.size_gb)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(models)
}
