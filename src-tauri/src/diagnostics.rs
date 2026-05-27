use crate::engine::{probe_runtime_manifest, EngineManifest};
use crate::hardware::{scan as scan_hardware, HardwareProfile};
use crate::models::{scan_registry, ModelRegistry};
use crate::paths::AppPaths;
use serde::{Deserialize, Serialize};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub schema_version: u32,
    pub generated_at: String,
    pub result: String,
    pub next_step: String,
    pub hardware: Option<HardwareProfile>,
    pub engine: EngineManifest,
    pub model_registry: ModelRegistry,
    pub findings: Vec<String>,
}

fn now_string() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!("{seconds}")
}

pub fn run(paths: &AppPaths) -> Result<DoctorReport, String> {
    let mut findings = Vec::new();

    let hardware = match scan_hardware(paths) {
        Ok(profile) => {
            findings.push(format!(
                "Hardware scan complete: {:.1} GB RAM, recommended tier: {}.",
                profile.ram_gb, profile.recommended_tier
            ));
            Some(profile)
        }
        Err(err) => {
            findings.push(format!("Hardware scan failed: {err}"));
            None
        }
    };

    let engine = probe_runtime_manifest(paths)?;
    let verified_runtimes = engine
        .runtimes
        .iter()
        .filter(|entry| entry.status == "verified")
        .count();

    if verified_runtimes == 0 {
        findings.push("No verified llama-server runtime binary was found.".to_string());
    } else {
        findings.push(format!("{verified_runtimes} runtime candidate(s) verified."));
    }

    if engine.router_supported {
        findings.push("At least one verified runtime supports router mode.".to_string());
    } else {
        findings.push("Router mode was not detected; classic single-model mode will be used.".to_string());
    }

    let model_registry = scan_registry(paths)?;
    let model_count = model_registry.models.len();
    let fit_count = model_registry
        .models
        .iter()
        .filter(|model| model.fit_status == "Fits")
        .count();

    if model_count == 0 {
        findings.push("No GGUF models were found in the models folder.".to_string());
    } else {
        findings.push(format!("{model_count} model(s) found; {fit_count} marked as Fits."));
    }

    let (result, next_step) = if verified_runtimes == 0 && model_count == 0 {
        (
            "needs_runtime_and_model".to_string(),
            "Install or bundle a llama-server runtime, then import or download a small GGUF model.".to_string(),
        )
    } else if verified_runtimes == 0 {
        (
            "needs_runtime".to_string(),
            "Add a compatible llama-server runtime to runtime/win or build the installer with a bundled CPU runtime.".to_string(),
        )
    } else if model_count == 0 {
        (
            "needs_model".to_string(),
            "Import a GGUF model or enable the future starter-model download flow.".to_string(),
        )
    } else {
        (
            "ready".to_string(),
            "Select a model marked Fits, start the runtime, and send a test message.".to_string(),
        )
    };

    let report = DoctorReport {
        schema_version: 1,
        generated_at: now_string(),
        result,
        next_step,
        hardware,
        engine,
        model_registry,
        findings,
    };

    fs::create_dir_all(&paths.diagnostics_dir).map_err(|err| err.to_string())?;
    let json = serde_json::to_string_pretty(&report).map_err(|err| err.to_string())?;
    fs::write(&paths.doctor_report, json).map_err(|err| err.to_string())?;

    Ok(report)
}
