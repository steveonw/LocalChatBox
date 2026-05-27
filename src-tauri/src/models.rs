use crate::hardware::{load_saved, HardwareProfile};
use crate::paths::AppPaths;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Compatibility {
    GoodLocalFit,
    MayRunSlowly,
    TooLarge,
    RemoteRecommended,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub file_name: String,
    pub display_name: String,
    pub path: String,
    pub size_bytes: u64,
    pub size_gb: f64,
    pub estimated_required_ram_gb: f64,
    pub estimated_tier: String,
    pub quant_hint: Option<String>,
    pub family_hint: Option<String>,
    pub compatibility: Compatibility,
    pub compatibility_label: String,
    pub fit_status: String,
    pub recommendation: String,
    pub recommended_context: u32,
    pub recommended_gpu_layers: u32,
    pub reasons: Vec<String>,
    pub last_verified: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRegistry {
    pub schema_version: u32,
    pub generated_at: String,
    pub models: Vec<ModelInfo>,
    pub notes: Vec<String>,
}

fn now_string() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!("{seconds}")
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn bytes_to_gb(bytes: u64) -> f64 {
    round2(bytes as f64 / 1024.0 / 1024.0 / 1024.0)
}

pub fn model_id_from_name(name: &str) -> String {
    let stem = Path::new(name)
        .file_stem()
        .and_then(|value| value.to_str())
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

fn detect_quant_hint(name: &str) -> Option<String> {
    let lower = name.to_lowercase();
    let hints = [
        "q2_k", "q3_k_s", "q3_k_m", "q3_k_l", "q4_0", "q4_1", "q4_k_s", "q4_k_m", "q5_0",
        "q5_1", "q5_k_s", "q5_k_m", "q6_k", "q8_0", "iq2", "iq3", "iq4", "f16", "bf16",
    ];

    hints
        .iter()
        .find(|hint| lower.contains(*hint))
        .map(|hint| (*hint).to_uppercase())
}

fn detect_family_hint(name: &str) -> Option<String> {
    let lower = name.to_lowercase();
    let families = [
        ("qwen", "Qwen"),
        ("llama", "Llama"),
        ("mistral", "Mistral"),
        ("mixtral", "Mixtral"),
        ("gemma", "Gemma"),
        ("phi", "Phi"),
        ("smollm", "SmolLM"),
        ("tinyllama", "TinyLlama"),
        ("deepseek", "DeepSeek"),
        ("granite", "Granite"),
    ];

    families
        .iter()
        .find(|(needle, _label)| lower.contains(*needle))
        .map(|(_needle, label)| (*label).to_string())
}

fn marker_at_boundary(lower: &str, marker: &str) -> bool {
    let bytes = lower.as_bytes();
    let marker_bytes = marker.as_bytes();

    if marker_bytes.is_empty() || marker_bytes.len() > bytes.len() {
        return false;
    }

    let mut i = 0;
    while i + marker_bytes.len() <= bytes.len() {
        if &bytes[i..i + marker_bytes.len()] == marker_bytes {
            let before_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
            let after = i + marker_bytes.len();
            let after_ok = after >= bytes.len() || !bytes[after].is_ascii_alphanumeric();
            if before_ok && after_ok {
                return true;
            }
        }
        i += 1;
    }

    false
}

fn estimate_tier(size_gb: f64, name: &str) -> String {
    let lower = name.to_lowercase();

    // Larger markers are intentionally checked first so "14B" does not match "4B",
    // "13B" does not match "3B", and "32B" does not match "2B".
    for marker in [
        "405b", "120b", "72b", "70b", "34b", "32b", "30b", "14b", "13b", "9b", "8b", "7b",
        "4b", "3b", "2b", "1.7b", "1.5b", "1.1b", "1b", "0.5b",
    ] {
        if marker_at_boundary(&lower, marker) || lower.contains(marker) {
            return marker.to_uppercase();
        }
    }

    if size_gb <= 1.2 {
        "tiny / 1B-ish".to_string()
    } else if size_gb <= 3.2 {
        "1B–3B-ish".to_string()
    } else if size_gb <= 5.8 {
        "4B–7B-ish".to_string()
    } else if size_gb <= 8.5 {
        "7B–9B-ish".to_string()
    } else if size_gb <= 12.5 {
        "13B-ish".to_string()
    } else {
        "large".to_string()
    }
}

fn kv_cache_gb_per_1k_context(size_gb: f64) -> f64 {
    if size_gb <= 1.5 {
        0.12
    } else if size_gb <= 3.5 {
        0.25
    } else if size_gb <= 6.0 {
        0.65
    } else if size_gb <= 9.0 {
        0.80
    } else if size_gb <= 13.0 {
        1.10
    } else {
        1.60
    }
}

fn recommended_context(size_gb: f64, hardware: Option<&HardwareProfile>) -> u32 {
    let ram = hardware.map(|profile| profile.ram_gb).unwrap_or(16.0);

    if ram < 8.0 {
        1024
    } else if ram < 12.0 || size_gb > 7.5 {
        2048
    } else if ram < 24.0 || size_gb > 4.5 {
        4096
    } else {
        8192
    }
}

fn estimate_required_ram(size_gb: f64, context: u32) -> f64 {
    // Not a GGUF parser. This is an intentionally conservative first-run heuristic:
    // model file + runtime overhead + context-sensitive KV cache approximation.
    let kv = kv_cache_gb_per_1k_context(size_gb) * (context as f64 / 1024.0);
    round2(size_gb * 1.08 + kv + 1.15)
}

fn advise(size_gb: f64, required_ram_gb: f64, hardware: Option<&HardwareProfile>) -> (Compatibility, String, String, Vec<String>) {
    let mut reasons = Vec::new();

    let Some(profile) = hardware else {
        reasons.push("Hardware profile is missing; run Hardware Scan first.".to_string());
        return (
            Compatibility::RemoteRecommended,
            "Unknown".to_string(),
            "Run the hardware scan so LocalChatBox can estimate fit.".to_string(),
            reasons,
        );
    };

    reasons.push(format!("Estimated RAM need: {:.1} GB.", required_ram_gb));
    reasons.push(format!("Detected system RAM: {:.1} GB.", profile.ram_gb));

    if required_ram_gb <= profile.ram_gb * 0.65 {
        if profile.nvidia_gpu_detected {
            reasons.push("NVIDIA GPU detected; CPU-safe mode is still the default until GPU layers are raised.".to_string());
        } else {
            reasons.push("CPU-only operation appears possible; replies may still be slow.".to_string());
        }
        (
            Compatibility::GoodLocalFit,
            "Fits".to_string(),
            "Good local fit. Start with the recommended context and gpu_layers = 0.".to_string(),
            reasons,
        )
    } else if required_ram_gb <= profile.ram_gb * 0.9 {
        reasons.push("RAM margin is narrow; close other apps and reduce context if loading fails.".to_string());
        (
            Compatibility::MayRunSlowly,
            "May be slow".to_string(),
            "May run locally, but memory margin is tight. Prefer a smaller model for first setup.".to_string(),
            reasons,
        )
    } else if required_ram_gb <= profile.ram_gb * 1.15 {
        reasons.push("Estimated memory exceeds comfortable RAM budget.".to_string());
        (
            Compatibility::MayRunSlowly,
            "Risky".to_string(),
            "Risky local fit. Try only with low context and expect possible failures.".to_string(),
            reasons,
        )
    } else {
        reasons.push("Estimated memory is larger than this machine should attempt for beginner use.".to_string());
        (
            Compatibility::TooLarge,
            "Won’t fit".to_string(),
            "Likely too large for local mode on this computer. Use a smaller GGUF or remote endpoint.".to_string(),
            reasons,
        )
    }
}

fn recommended_gpu_layers(size_gb: f64, hardware: Option<&HardwareProfile>) -> u32 {
    let Some(profile) = hardware else {
        return 0;
    };

    if !profile.nvidia_gpu_detected {
        return 0;
    }

    let max_vram = profile
        .gpus
        .iter()
        .filter_map(|gpu| gpu.vram_gb)
        .fold(0.0_f64, f64::max);

    if max_vram < 4.0 {
        0
    } else if max_vram < 6.0 || size_gb > 5.0 {
        8
    } else if max_vram < 10.0 {
        16
    } else {
        24
    }
}

fn model_info_from_entry(entry: &fs::DirEntry, hardware: Option<&HardwareProfile>) -> Result<Option<ModelInfo>, String> {
    let path = entry.path();
    if path.extension().and_then(|ext| ext.to_str()).map(|ext| ext.eq_ignore_ascii_case("gguf")) != Some(true) {
        return Ok(None);
    }

    let metadata = entry.metadata().map_err(|err| err.to_string())?;
    if !metadata.is_file() {
        return Ok(None);
    }

    let file_name = entry
        .file_name()
        .to_string_lossy()
        .to_string();
    let size_bytes = metadata.len();
    let size_gb = bytes_to_gb(size_bytes);
    let context = recommended_context(size_gb, hardware);
    let required_ram = estimate_required_ram(size_gb, context);
    let (compatibility, label, recommendation, reasons) = advise(size_gb, required_ram, hardware);

    Ok(Some(ModelInfo {
        id: model_id_from_name(&file_name),
        display_name: Path::new(&file_name)
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or(&file_name)
            .replace('_', " ").replace('-', " "),
        file_name: file_name.clone(),
        path: path.to_string_lossy().replace('\\', "/"),
        size_bytes,
        size_gb,
        estimated_required_ram_gb: required_ram,
        estimated_tier: estimate_tier(size_gb, &file_name),
        quant_hint: detect_quant_hint(&file_name),
        family_hint: detect_family_hint(&file_name),
        compatibility,
        compatibility_label: label.clone(),
        fit_status: label,
        recommendation,
        recommended_context: context,
        recommended_gpu_layers: recommended_gpu_layers(size_gb, hardware),
        reasons,
        last_verified: now_string(),
    }))
}

pub fn scan(paths: &AppPaths) -> Result<Vec<ModelInfo>, String> {
    let hardware = load_saved(paths);
    let hardware_ref = hardware.as_ref();

    let dir = Path::new(&paths.models_dir);
    if !dir.exists() {
        fs::create_dir_all(dir).map_err(|err| err.to_string())?;
        return Ok(Vec::new());
    }

    let mut models = Vec::new();
    for entry in fs::read_dir(dir).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        if let Some(model) = model_info_from_entry(&entry, hardware_ref)? {
            models.push(model);
        }
    }

    models.sort_by(|a, b| {
        a.estimated_required_ram_gb
            .partial_cmp(&b.estimated_required_ram_gb)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.file_name.cmp(&b.file_name))
    });

    Ok(models)
}

pub fn scan_registry(paths: &AppPaths) -> Result<ModelRegistry, String> {
    let models = scan(paths)?;
    let registry = ModelRegistry {
        schema_version: 1,
        generated_at: now_string(),
        models,
        notes: vec![
            "This registry is regenerated from the models folder. User annotations are planned for a later release.".to_string(),
            "Fit estimates are conservative heuristics until a GGUF header parser is added.".to_string(),
        ],
    };

    if let Some(parent) = Path::new(&paths.model_registry).parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let json = serde_json::to_string_pretty(&registry).map_err(|err| err.to_string())?;
    fs::write(&paths.model_registry, json).map_err(|err| err.to_string())?;

    Ok(registry)
}
