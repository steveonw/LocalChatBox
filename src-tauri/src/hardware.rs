use crate::paths::AppPaths;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process::Command;
use sysinfo::{CpuExt, DiskExt, System, SystemExt};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    pub name: String,
    pub vram_gb: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareProfile {
    pub cpu_name: String,
    pub ram_gb: f64,
    pub os_name: String,
    pub os_version: String,
    pub supports_sse2: bool,
    pub supports_avx: bool,
    pub supports_avx2: bool,
    #[serde(default)]
    pub supports_fma: bool,
    pub gpus: Vec<GpuInfo>,
    pub nvidia_gpu_detected: bool,
    pub cuda_maybe_usable: bool,
    pub disk_free_gb: Option<f64>,
    pub recommended_tier: String,
    pub notes: Vec<String>,
}

fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn bytes_to_gb(bytes: u64) -> f64 {
    round1(bytes as f64 / 1024.0 / 1024.0 / 1024.0)
}

fn kib_to_gb(kib: u64) -> f64 {
    round1(kib as f64 / 1024.0 / 1024.0)
}

fn feature_support() -> (bool, bool, bool, bool) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        (
            std::is_x86_feature_detected!("sse2"),
            std::is_x86_feature_detected!("avx"),
            std::is_x86_feature_detected!("avx2"),
            std::is_x86_feature_detected!("fma"),
        )
    }

    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        (false, false, false, false)
    }
}

#[cfg(target_os = "windows")]
fn query_windows_gpus() -> Vec<GpuInfo> {
    // Prefer PowerShell CIM because WMIC is absent on some newer Windows builds.
    let ps = Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            "Get-CimInstance Win32_VideoController | Select-Object Name,AdapterRAM | ConvertTo-Json -Compress",
        ])
        .output();

    if let Ok(output) = ps {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !text.is_empty() {
                if let Some(gpus) = parse_gpu_json(&text) {
                    return gpus;
                }
            }
        }
    }

    let wmic = Command::new("wmic")
        .args(["path", "win32_VideoController", "get", "Name,AdapterRAM", "/format:csv"])
        .output();

    if let Ok(output) = wmic {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            return parse_wmic_gpus(&text);
        }
    }

    Vec::new()
}

#[cfg(not(target_os = "windows"))]
fn query_windows_gpus() -> Vec<GpuInfo> {
    Vec::new()
}

fn parse_gpu_json(text: &str) -> Option<Vec<GpuInfo>> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let items = if let Some(array) = value.as_array() {
        array.clone()
    } else {
        vec![value]
    };

    let mut gpus = Vec::new();
    for item in items {
        let name = item
            .get("Name")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown GPU")
            .trim()
            .to_string();
        let vram_gb = item
            .get("AdapterRAM")
            .and_then(|v| v.as_u64())
            .filter(|bytes| *bytes > 0)
            .map(bytes_to_gb);
        if !name.is_empty() {
            gpus.push(GpuInfo { name, vram_gb });
        }
    }
    Some(gpus)
}

fn parse_wmic_gpus(text: &str) -> Vec<GpuInfo> {
    let mut gpus = Vec::new();

    for line in text.lines() {
        let clean = line.trim();
        if clean.is_empty() || clean.contains("Node,AdapterRAM,Name") {
            continue;
        }

        let parts: Vec<&str> = clean.split(',').collect();
        if parts.len() < 3 {
            continue;
        }

        let adapter_ram = parts[1].trim().parse::<u64>().ok();
        let name = parts[2..].join(",").trim().to_string();
        if name.is_empty() {
            continue;
        }

        gpus.push(GpuInfo {
            name,
            vram_gb: adapter_ram.filter(|bytes| *bytes > 0).map(bytes_to_gb),
        });
    }

    gpus
}

fn recommend_tier(ram_gb: f64, max_vram_gb: f64, supports_avx: bool) -> (String, Vec<String>) {
    let mut notes = Vec::new();

    if !supports_avx {
        notes.push("CPU AVX was not detected. Use the basic CPU runtime if CUDA fails.".to_string());
    }

    if max_vram_gb >= 8.0 && ram_gb >= 24.0 {
        notes.push("8 GB+ VRAM detected. 7B Q4 models should be reasonable; larger models still need testing.".to_string());
        return ("small-and-medium-local-models".to_string(), notes);
    }

    if max_vram_gb >= 4.0 && ram_gb >= 16.0 {
        notes.push("4 GB VRAM detected. Prefer 1B–3B Q4 models; 7B Q3/Q4 may run with partial offload.".to_string());
        return ("small-local-models".to_string(), notes);
    }

    if ram_gb >= 16.0 {
        notes.push("Enough system RAM for small CPU mode, but GPU acceleration may be limited or unavailable.".to_string());
        return ("cpu-small-models".to_string(), notes);
    }

    notes.push("Low memory profile. Use 1B–3B quantized GGUF models or remote endpoint mode.".to_string());
    ("remote-or-tiny-local-models".to_string(), notes)
}

pub fn scan(paths: &AppPaths) -> Result<HardwareProfile, String> {
    let mut system = System::new_all();
    system.refresh_all();

    let cpu_name = system
        .cpus()
        .first()
        .map(|cpu| cpu.brand().trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Unknown CPU".to_string());

    // sysinfo 0.29 returns KiB for total_memory.
    let ram_gb = kib_to_gb(system.total_memory());
    let os_name = System::name().unwrap_or_else(|| std::env::consts::OS.to_string());
    let os_version = System::os_version().unwrap_or_else(|| "unknown".to_string());

    let (supports_sse2, supports_avx, supports_avx2, supports_fma) = feature_support();
    let gpus = query_windows_gpus();
    let max_vram_gb = gpus
        .iter()
        .filter_map(|gpu| gpu.vram_gb)
        .fold(0.0_f64, f64::max);

    let nvidia_gpu_detected = gpus
        .iter()
        .any(|gpu| gpu.name.to_lowercase().contains("nvidia"));
    let cuda_maybe_usable = nvidia_gpu_detected;

    let disk_free_gb = system
        .disks()
        .iter()
        .find(|disk| {
            let mount = disk.mount_point().to_string_lossy();
            paths.root.starts_with(mount.as_ref())
        })
        .map(|disk| bytes_to_gb(disk.available_space()))
        .or_else(|| system.disks().first().map(|disk| bytes_to_gb(disk.available_space())));

    let (recommended_tier, mut notes) = recommend_tier(ram_gb, max_vram_gb, supports_avx);

    if supports_avx && !supports_avx2 {
        notes.push("AVX detected but AVX2 was not. Keep the AVX-only CPU runtime available for older CPUs.".to_string());
    }
    if supports_avx2 && !supports_fma {
        notes.push("AVX2 detected but FMA was not. Prefer the AVX-only or basic CPU runtime unless your AVX2 build is known to work.".to_string());
    }
    if cuda_maybe_usable {
        notes.push("NVIDIA GPU detected. CUDA may work if the bundled llama-server CUDA build matches the installed driver.".to_string());
    }

    let profile = HardwareProfile {
        cpu_name,
        ram_gb,
        os_name,
        os_version,
        supports_sse2,
        supports_avx,
        supports_avx2,
        supports_fma,
        gpus,
        nvidia_gpu_detected,
        cuda_maybe_usable,
        disk_free_gb,
        recommended_tier,
        notes,
    };

    let out_path = Path::new(&paths.data_dir).join("hardware-profile.json");
    let json = serde_json::to_string_pretty(&profile).map_err(|err| err.to_string())?;
    fs::write(out_path, json).map_err(|err| err.to_string())?;

    Ok(profile)
}

pub fn load_saved(paths: &AppPaths) -> Option<HardwareProfile> {
    let path = Path::new(&paths.data_dir).join("hardware-profile.json");
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}
