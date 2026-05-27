use crate::hardware::load_saved;
use crate::paths::AppPaths;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeProbeEntry {
    pub id: String,
    pub file_name: String,
    pub path: String,
    pub exists: bool,
    pub status: String,
    pub version: Option<String>,
    pub supported_flags: Vec<String>,
    pub ui_disable_flag: Option<String>,
    pub router_supported: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineManifest {
    pub schema_version: u32,
    pub engine: String,
    pub manifest_version: String,
    pub generated_at: String,
    pub preferred_backend: String,
    pub known_good_backend: Option<String>,
    pub router_supported: bool,
    pub runtimes: Vec<RuntimeProbeEntry>,
    pub notes: Vec<String>,
}

fn now_string() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!("{seconds}")
}

fn runtime_table() -> Vec<(&'static str, &'static str)> {
    vec![
        ("cuda", "llama-server-cuda.exe"),
        ("cpu-avx2", "llama-server-cpu-avx2.exe"),
        ("cpu-avx", "llama-server-cpu-avx.exe"),
        ("cpu-basic", "llama-server-cpu-basic.exe"),
        ("generic", "llama-server.exe"),
    ]
}

fn output_to_string(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).to_string()
}

fn run_probe_command(path: &Path, arg: &str) -> Result<String, String> {
    let mut command = Command::new(path);
    command.arg(arg);

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    let output = command
        .output()
        .map_err(|err| format!("Could not run {} {arg}: {err}", path.display()))?;

    let mut text = output_to_string(&output.stdout);
    let stderr = output_to_string(&output.stderr);
    if !stderr.trim().is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&stderr);
    }

    Ok(text)
}

fn detect_supported_flags(help: &str) -> Vec<String> {
    let candidates = [
        "--api-key",
        "--no-ui",
        "--no-webui",
        "--ctx-size",
        "--n-gpu-layers",
        "--parallel",
        "--host",
        "--port",
        "--alias",
        "--models-dir",
        "--models-preset",
        "--models-max",
        "--models-autoload",
        "--no-models-autoload",
        "--version",
        "--help",
    ];

    candidates
        .iter()
        .filter(|flag| help.contains(*flag))
        .map(|flag| (*flag).to_string())
        .collect()
}

fn preferred_backend(paths: &AppPaths) -> String {
    if let Some(profile) = load_saved(paths) {
        if profile.nvidia_gpu_detected {
            "cuda".to_string()
        } else if profile.supports_avx2 && profile.supports_fma {
            "cpu-avx2".to_string()
        } else if profile.supports_avx {
            "cpu-avx".to_string()
        } else {
            "cpu-basic".to_string()
        }
    } else {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            if std::is_x86_feature_detected!("avx2") && std::is_x86_feature_detected!("fma") {
                "cpu-avx2".to_string()
            } else if std::is_x86_feature_detected!("avx") {
                "cpu-avx".to_string()
            } else {
                "cpu-basic".to_string()
            }
        }

        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        {
            "cpu-basic".to_string()
        }
    }
}

fn probe_one(paths: &AppPaths, id: &str, file_name: &str) -> RuntimeProbeEntry {
    let path = Path::new(&paths.runtime_win_dir).join(file_name);
    let path_string = path.to_string_lossy().replace('\\', "/");

    if !path.exists() {
        return RuntimeProbeEntry {
            id: id.to_string(),
            file_name: file_name.to_string(),
            path: path_string,
            exists: false,
            status: "missing".to_string(),
            version: None,
            supported_flags: Vec::new(),
            ui_disable_flag: None,
            router_supported: false,
            error: None,
        };
    }

    let help = match run_probe_command(&path, "--help") {
        Ok(text) => text,
        Err(err) => {
            return RuntimeProbeEntry {
                id: id.to_string(),
                file_name: file_name.to_string(),
                path: path_string,
                exists: true,
                status: "probe_failed".to_string(),
                version: None,
                supported_flags: Vec::new(),
                ui_disable_flag: None,
                router_supported: false,
                error: Some(err),
            };
        }
    };

    let version = run_probe_command(&path, "--version")
        .ok()
        .map(|text| text.lines().next().unwrap_or("").trim().to_string())
        .filter(|text| !text.is_empty());

    let supported_flags = detect_supported_flags(&help);
    let ui_disable_flag = if supported_flags.iter().any(|flag| flag == "--no-ui") {
        Some("--no-ui".to_string())
    } else if supported_flags.iter().any(|flag| flag == "--no-webui") {
        Some("--no-webui".to_string())
    } else {
        None
    };

    let router_supported = supported_flags.iter().any(|flag| flag == "--models-preset")
        && supported_flags.iter().any(|flag| flag == "--models-max");

    RuntimeProbeEntry {
        id: id.to_string(),
        file_name: file_name.to_string(),
        path: path_string,
        exists: true,
        status: "verified".to_string(),
        version,
        supported_flags,
        ui_disable_flag,
        router_supported,
        error: None,
    }
}

pub fn probe_runtime_manifest(paths: &AppPaths) -> Result<EngineManifest, String> {
    fs::create_dir_all(&paths.llama_engine_dir).map_err(|err| err.to_string())?;

    let runtimes: Vec<RuntimeProbeEntry> = runtime_table()
        .into_iter()
        .map(|(id, file_name)| probe_one(paths, id, file_name))
        .collect();

    let preferred_backend = preferred_backend(paths);
    let known_good_backend = runtimes
        .iter()
        .find(|entry| entry.status == "verified" && entry.id == preferred_backend)
        .or_else(|| runtimes.iter().find(|entry| entry.status == "verified"))
        .map(|entry| entry.id.clone());

    let router_supported = runtimes
        .iter()
        .any(|entry| entry.status == "verified" && entry.router_supported);

    let manifest = EngineManifest {
        schema_version: 1,
        engine: "llama.cpp".to_string(),
        manifest_version: "0.3".to_string(),
        generated_at: now_string(),
        preferred_backend,
        known_good_backend,
        router_supported,
        runtimes,
        notes: vec![
            "Generated by LocalChatBox v0.3 runtime probe.".to_string(),
            "Router mode is enabled only when a verified runtime exposes --models-preset and --models-max.".to_string(),
            "Installer builds should bundle at least one verified CPU runtime for first-run ease of use.".to_string(),
        ],
    };

    let json = serde_json::to_string_pretty(&manifest).map_err(|err| err.to_string())?;
    fs::write(&paths.engine_manifest, json).map_err(|err| err.to_string())?;

    Ok(manifest)
}
