use crate::engine::{probe_runtime_manifest, RuntimeProbeEntry};
use crate::hardware::load_saved;
use crate::models::{model_id_from_name, scan as scan_models};
use crate::paths::AppPaths;
use crate::storage::ChatMessage;
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeStatus {
    pub state: String,
    pub message: String,
    pub port: u16,
    pub mode: Option<String>,
    pub runtime_mode: Option<String>,
    pub backend: Option<String>,
    pub router_supported: bool,
    pub model_path: Option<String>,
    pub model_name: Option<String>,
    pub loaded_model_id: Option<String>,
    pub model_status: Option<String>,
    pub pid: Option<u32>,
    pub started_at: Option<String>,
    pub generation: u64,

    // The local llama-server API key is intentionally not sent to the frontend
    // or persisted to data/runtime-state.json. It stays in the Rust process and
    // is passed to llama-server through the child process environment variable
    // LLAMA_API_KEY instead of command-line args.
    #[serde(skip_serializing)]
    pub api_key: Option<String>,
}

impl Default for RuntimeStatus {
    fn default() -> Self {
        Self {
            state: "stopped".to_string(),
            message: "Runtime stopped.".to_string(),
            port: 8080,
            mode: None,
            runtime_mode: None,
            backend: None,
            router_supported: false,
            model_path: None,
            model_name: None,
            loaded_model_id: None,
            model_status: None,
            pid: None,
            started_at: None,
            generation: 0,
            api_key: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeStartRequest {
    pub model_path: String,
    pub port: u16,
    pub context_length: u32,
    pub gpu_layers: u32,
    pub preferred_runtime: String,
    #[serde(default)]
    pub runtime_mode: Option<String>,
    #[serde(default)]
    pub model_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatRequest {
    pub mode: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: f32,
    pub max_tokens: u32,
    pub model: String,
    pub remote_base_url: Option<String>,
    pub remote_api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatResponse {
    pub content: String,
    pub model: Option<String>,
    pub raw: serde_json::Value,
}

pub struct RuntimeManager {
    child: Option<Child>,
    status: RuntimeStatus,
    generation: u64,
}

#[derive(Debug, Clone)]
pub struct RuntimeCandidate {
    pub path: PathBuf,
    pub mode: String,
    pub ui_disable_flag: String,
    pub router_supported: bool,
}

#[derive(Debug, Clone, Copy)]
struct RuntimeCapabilities {
    supports_avx: bool,
    supports_avx2: bool,
    supports_fma: bool,
    nvidia_gpu_detected: bool,
}

#[derive(Debug)]
enum RuntimeWaitResult {
    Ready,
    Cancelled,
    Exited(String),
    TimedOut,
}

impl Default for RuntimeManager {
    fn default() -> Self {
        Self {
            child: None,
            status: RuntimeStatus::default(),
            generation: 0,
        }
    }
}

fn now_string() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!("{seconds}")
}

fn append_log(paths: &AppPaths, line: &str) {
    let _ = fs::create_dir_all(&paths.logs_dir);
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.runtime_log)
    {
        let _ = writeln!(file, "{line}");
    }
}

fn runtime_state_path(paths: &AppPaths) -> PathBuf {
    Path::new(&paths.data_dir).join("runtime-state.json")
}

fn persist_runtime_state(paths: &AppPaths, status: &RuntimeStatus) {
    let _ = fs::create_dir_all(&paths.data_dir);
    if let Ok(json) = serde_json::to_string_pretty(status) {
        let _ = fs::write(runtime_state_path(paths), json);
    }
}

fn pump_output<R>(reader: R, log_path: String, prefix: &'static str, secret: String)
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let lines = BufReader::new(reader).lines();
        let mut file = match OpenOptions::new().create(true).append(true).open(log_path) {
            Ok(file) => file,
            Err(_) => return,
        };

        for line in lines.map_while(Result::ok) {
            let safe_line = if !secret.is_empty() {
                line.replace(&secret, "<redacted>")
            } else {
                line
            };
            let _ = writeln!(file, "[{prefix}] {safe_line}");
        }
    });
}

fn model_name_from_path(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("model.gguf")
        .to_string()
}

fn model_id_from_request(request: &RuntimeStartRequest) -> String {
    request
        .model_id
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| model_id_from_name(&model_name_from_path(&request.model_path)))
}

fn local_cpu_features() -> (bool, bool, bool, bool) {
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

fn runtime_capabilities(paths: &AppPaths) -> RuntimeCapabilities {
    let (_local_sse2, local_avx, local_avx2, local_fma) = local_cpu_features();

    if let Some(profile) = load_saved(paths) {
        RuntimeCapabilities {
            supports_avx: profile.supports_avx || local_avx,
            supports_avx2: profile.supports_avx2 || local_avx2,
            supports_fma: profile.supports_fma || local_fma,
            nvidia_gpu_detected: profile.nvidia_gpu_detected,
        }
    } else {
        RuntimeCapabilities {
            supports_avx: local_avx,
            supports_avx2: local_avx2,
            supports_fma: local_fma,
            nvidia_gpu_detected: false,
        }
    }
}

fn push_candidate_id(ordered: &mut Vec<&'static str>, id: &'static str) {
    if !ordered.iter().any(|existing| *existing == id) {
        ordered.push(id);
    }
}

fn ordered_runtime_ids(paths: &AppPaths, preferred_runtime: &str, requested_gpu_layers: u32) -> Vec<&'static str> {
    let caps = runtime_capabilities(paths);
    let mut ordered: Vec<&'static str> = Vec::new();

    let cuda_requested = preferred_runtime == "cuda";
    let cuda_auto_ok = requested_gpu_layers > 0 && caps.nvidia_gpu_detected;

    let add_cpu_fallbacks = |ordered: &mut Vec<&'static str>| {
        if caps.supports_avx2 && caps.supports_fma {
            push_candidate_id(ordered, "cpu-avx2");
        }
        if caps.supports_avx {
            push_candidate_id(ordered, "cpu-avx");
        }
        push_candidate_id(ordered, "cpu-basic");
        push_candidate_id(ordered, "generic");
    };

    match preferred_runtime {
        "cuda" => {
            push_candidate_id(&mut ordered, "cuda");
            add_cpu_fallbacks(&mut ordered);
        }
        "cpu-avx2" => {
            if caps.supports_avx2 && caps.supports_fma {
                push_candidate_id(&mut ordered, "cpu-avx2");
            }
            if caps.supports_avx {
                push_candidate_id(&mut ordered, "cpu-avx");
            }
            push_candidate_id(&mut ordered, "cpu-basic");
            push_candidate_id(&mut ordered, "generic");
        }
        "cpu-avx" => {
            if caps.supports_avx {
                push_candidate_id(&mut ordered, "cpu-avx");
            }
            push_candidate_id(&mut ordered, "cpu-basic");
            push_candidate_id(&mut ordered, "generic");
        }
        "cpu-basic" => {
            push_candidate_id(&mut ordered, "cpu-basic");
            push_candidate_id(&mut ordered, "generic");
        }
        _ => {
            if cuda_auto_ok || cuda_requested {
                push_candidate_id(&mut ordered, "cuda");
            }
            add_cpu_fallbacks(&mut ordered);
        }
    }

    ordered
}

fn file_for_runtime_id(id: &str) -> &'static str {
    match id {
        "cuda" => "llama-server-cuda.exe",
        "cpu-avx2" => "llama-server-cpu-avx2.exe",
        "cpu-avx" => "llama-server-cpu-avx.exe",
        "cpu-basic" => "llama-server-cpu-basic.exe",
        _ => "llama-server.exe",
    }
}

fn entry_to_candidate(entry: &RuntimeProbeEntry) -> Option<RuntimeCandidate> {
    if entry.status != "verified" || !entry.exists {
        return None;
    }

    let ui_disable_flag = entry
        .ui_disable_flag
        .clone()
        .unwrap_or_else(|| "--no-ui".to_string());

    Some(RuntimeCandidate {
        path: PathBuf::from(entry.path.clone()),
        mode: entry.id.clone(),
        ui_disable_flag,
        router_supported: entry.router_supported,
    })
}

fn candidate_executables(
    paths: &AppPaths,
    preferred_runtime: &str,
    requested_gpu_layers: u32,
) -> Vec<RuntimeCandidate> {
    let ordered_ids = ordered_runtime_ids(paths, preferred_runtime, requested_gpu_layers);
    let mut candidates = Vec::new();

    if let Ok(manifest) = probe_runtime_manifest(paths) {
        for id in &ordered_ids {
            if let Some(entry) = manifest.runtimes.iter().find(|entry| entry.id == *id) {
                if let Some(candidate) = entry_to_candidate(entry) {
                    candidates.push(candidate);
                }
            }
        }
    }

    if !candidates.is_empty() {
        return candidates;
    }

    // Last-resort fallback if probing failed but files are present. This keeps older
    // runtime builds usable while still preferring probed launch profiles.
    let runtime_dir = Path::new(&paths.runtime_win_dir);
    ordered_ids
        .into_iter()
        .flat_map(|id| {
            let path = runtime_dir.join(file_for_runtime_id(id));
            [
                RuntimeCandidate {
                    path: path.clone(),
                    mode: id.to_string(),
                    ui_disable_flag: "--no-ui".to_string(),
                    router_supported: false,
                },
                RuntimeCandidate {
                    path,
                    mode: id.to_string(),
                    ui_disable_flag: "--no-webui".to_string(),
                    router_supported: false,
                },
            ]
        })
        .filter(|candidate| candidate.path.exists())
        .collect()
}

fn generate_local_api_key() -> String {
    let suffix: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(48)
        .map(char::from)
        .collect();
    format!("lcb_{suffix}")
}

fn redacted_args(args: &[String]) -> String {
    args.join(" ")
}

fn port_available(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

fn choose_free_port(preferred_port: u16) -> Result<u16, String> {
    if preferred_port >= 1024 && port_available(preferred_port) {
        return Ok(preferred_port);
    }

    for _ in 0..128 {
        let candidate = rand::thread_rng().gen_range(49152..=65535);
        if port_available(candidate) {
            return Ok(candidate);
        }
    }

    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|err| format!("Could not reserve a local port: {err}"))?;
    let port = listener
        .local_addr()
        .map_err(|err| format!("Could not inspect reserved local port: {err}"))?
        .port();
    Ok(port)
}

fn should_use_router(candidate: &RuntimeCandidate, request: &RuntimeStartRequest) -> bool {
    match request.runtime_mode.as_deref().unwrap_or("auto") {
        "router" => candidate.router_supported,
        "classic" => false,
        _ => candidate.router_supported,
    }
}

fn write_router_preset(paths: &AppPaths, request: &RuntimeStartRequest, selected_model_id: &str) -> Result<String, String> {
    fs::create_dir_all(&paths.llama_engine_dir).map_err(|err| err.to_string())?;

    let mut models = scan_models(paths).unwrap_or_default();

    if !models.iter().any(|model| model.path == request.model_path) {
        models.push(crate::models::ModelInfo {
            id: selected_model_id.to_string(),
            file_name: model_name_from_path(&request.model_path),
            display_name: model_name_from_path(&request.model_path),
            path: request.model_path.clone(),
            size_bytes: 0,
            size_gb: 0.0,
            estimated_required_ram_gb: 0.0,
            estimated_tier: "unknown".to_string(),
            quant_hint: None,
            family_hint: None,
            compatibility: crate::models::Compatibility::MayRunSlowly,
            compatibility_label: "Unknown".to_string(),
            fit_status: "Unknown".to_string(),
            recommendation: "Imported as selected model.".to_string(),
            recommended_context: request.context_length,
            recommended_gpu_layers: request.gpu_layers,
            reasons: Vec::new(),
            last_verified: now_string(),
        });
    }

    let mut text = String::new();
    text.push_str("version = 1\n\n");
    text.push_str("[*]\n");
    text.push_str(&format!("c = {}\n", request.context_length));
    text.push_str(&format!("n-gpu-layers = {}\n", request.gpu_layers));
    text.push_str("parallel = 1\n\n");

    for model in models {
        let id = if model.id.trim().is_empty() {
            model_id_from_name(&model.file_name)
        } else {
            model.id.clone()
        };

        text.push_str(&format!("[{}]\n", id));
        text.push_str(&format!("model = {}\n", model.path.replace('\\', "/")));
        text.push_str(&format!("c = {}\n", request.context_length));
        text.push_str(&format!("n-gpu-layers = {}\n", request.gpu_layers));
        text.push_str(&format!(
            "load-on-startup = {}\n",
            if id == selected_model_id { "true" } else { "false" }
        ));
        text.push_str("stop-timeout = 10\n\n");
    }

    fs::write(&paths.router_preset, text).map_err(|err| err.to_string())?;
    Ok(paths.router_preset.clone())
}

impl RuntimeManager {
    pub fn status(&mut self, paths: &AppPaths) -> RuntimeStatus {
        self.refresh_process_state(paths);
        self.status.clone()
    }

    fn kill_child(&mut self, paths: &AppPaths) {
        if let Some(mut child) = self.child.take() {
            append_log(paths, "[LocalChatBox] stopping runtime process");
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    pub fn stop(&mut self, paths: &AppPaths) -> RuntimeStatus {
        self.generation = self.generation.saturating_add(1);
        self.kill_child(paths);

        self.status = RuntimeStatus {
            port: self.status.port,
            generation: self.generation,
            ..RuntimeStatus::default()
        };
        persist_runtime_state(paths, &self.status);
        self.status.clone()
    }

    fn refresh_process_state(&mut self, paths: &AppPaths) {
        let mut changed = false;

        if let Some(child) = self.child.as_mut() {
            match child.try_wait() {
                Ok(Some(exit_status)) => {
                    self.child = None;
                    self.status.state = "error".to_string();
                    self.status.message = format!("Runtime exited with status {exit_status}.");
                    self.status.pid = None;
                    self.status.api_key = None;
                    self.status.model_status = Some("failed".to_string());
                    changed = true;
                }
                Ok(None) => {}
                Err(err) => {
                    self.status.state = "error".to_string();
                    self.status.message = format!("Could not query runtime process: {err}");
                    changed = true;
                }
            }
        }

        if changed {
            persist_runtime_state(paths, &self.status);
        }
    }

    fn begin_start(&mut self, paths: &AppPaths) -> u64 {
        self.generation = self.generation.saturating_add(1);
        self.kill_child(paths);
        self.status.generation = self.generation;
        self.generation
    }

    pub fn launch_candidate(
        &mut self,
        paths: &AppPaths,
        candidate: &RuntimeCandidate,
        request: &RuntimeStartRequest,
        generation: u64,
    ) -> Result<RuntimeStatus, String> {
        self.kill_child(paths);

        let model_path = Path::new(&request.model_path);
        if !model_path.exists() {
            return Err(format!("Model file not found: {}", request.model_path));
        }

        let api_key = generate_local_api_key();
        let selected_model_id = model_id_from_request(request);
        let use_router = should_use_router(candidate, request);

        let mut args = vec![
            "--host".to_string(),
            "127.0.0.1".to_string(),
            "--port".to_string(),
            request.port.to_string(),
            "--parallel".to_string(),
            "1".to_string(),
            candidate.ui_disable_flag.clone(),
        ];

        let runtime_mode = if use_router {
            let preset = write_router_preset(paths, request, &selected_model_id)?;
            args.extend([
                "--models-preset".to_string(),
                preset,
                "--models-max".to_string(),
                "1".to_string(),
            ]);
            "router".to_string()
        } else {
            args.extend([
                "--model".to_string(),
                request.model_path.clone(),
                "--ctx-size".to_string(),
                request.context_length.to_string(),
                "--n-gpu-layers".to_string(),
                request.gpu_layers.to_string(),
                "--alias".to_string(),
                selected_model_id.clone(),
            ]);
            "classic".to_string()
        };

        append_log(
            paths,
            &format!(
                "[LocalChatBox] launching {} in {} mode with args: {}",
                candidate.path.display(),
                runtime_mode,
                redacted_args(&args)
            ),
        );

        let mut command = Command::new(&candidate.path);
        command
            .args(&args)
            .env("LLAMA_API_KEY", &api_key)
            .current_dir(Path::new(&paths.root))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            command.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = command
            .spawn()
            .map_err(|err| format!("Could not launch {}: {err}", candidate.path.display()))?;

        let pid = child.id();
        if let Some(stdout) = child.stdout.take() {
            pump_output(stdout, paths.runtime_log.clone(), "llama.stdout", api_key.clone());
        }
        if let Some(stderr) = child.stderr.take() {
            pump_output(stderr, paths.runtime_log.clone(), "llama.stderr", api_key.clone());
        }

        self.status = RuntimeStatus {
            state: "loading".to_string(),
            message: format!("Loading model with {} runtime in {} mode…", candidate.mode, runtime_mode),
            port: request.port,
            mode: Some(candidate.mode.clone()),
            runtime_mode: Some(runtime_mode),
            backend: Some(candidate.mode.clone()),
            router_supported: candidate.router_supported,
            model_path: Some(request.model_path.clone()),
            model_name: Some(model_name_from_path(&request.model_path)),
            loaded_model_id: Some(selected_model_id),
            model_status: Some("loading".to_string()),
            pid: Some(pid),
            started_at: Some(now_string()),
            generation,
            api_key: Some(api_key),
        };

        self.child = Some(child);
        persist_runtime_state(paths, &self.status);
        Ok(self.status.clone())
    }

    pub fn mark_running(&mut self, paths: &AppPaths) -> RuntimeStatus {
        self.status.state = "running".to_string();
        self.status.message = if self.status.runtime_mode.as_deref() == Some("router") {
            "Router runtime is ready and selected model is loaded.".to_string()
        } else {
            "Local model server is ready.".to_string()
        };
        self.status.model_status = Some("ready".to_string());
        persist_runtime_state(paths, &self.status);
        self.status.clone()
    }

    pub fn mark_error(&mut self, paths: &AppPaths, message: String) -> RuntimeStatus {
        self.status.state = "error".to_string();
        self.status.message = message;
        self.status.pid = None;
        self.status.api_key = None;
        self.status.model_status = Some("failed".to_string());
        persist_runtime_state(paths, &self.status);
        self.status.clone()
    }
}

async fn http_client(timeout_secs: u64) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(timeout_secs))
        .build()
        .map_err(|err| format!("Could not build HTTP client: {err}"))
}

async fn wait_for_ready_or_exit(
    manager: &std::sync::Mutex<RuntimeManager>,
    paths: &AppPaths,
    port: u16,
    api_key: &str,
    generation: u64,
    timeout_secs: u64,
) -> RuntimeWaitResult {
    let client = match http_client(20).await {
        Ok(client) => client,
        Err(err) => return RuntimeWaitResult::Exited(err),
    };
    let health_url = format!("http://127.0.0.1:{port}/health");
    let models_url = format!("http://127.0.0.1:{port}/v1/models");
    let attempts = timeout_secs.saturating_mul(2).max(40);

    for _ in 0..attempts {
        if let Ok(response) = client.get(&health_url).send().await {
            if response.status().is_success() {
                return RuntimeWaitResult::Ready;
            }
        }

        let mut builder = client.get(&models_url);
        if !api_key.trim().is_empty() {
            builder = builder.header("Authorization", format!("Bearer {api_key}"));
        }

        if let Ok(response) = builder.send().await {
            if response.status().is_success() {
                return RuntimeWaitResult::Ready;
            }
        }

        match manager.lock() {
            Ok(mut locked) => {
                if locked.generation != generation {
                    return RuntimeWaitResult::Cancelled;
                }

                let status = locked.status(paths);
                if status.state == "error" {
                    return RuntimeWaitResult::Exited(status.message);
                }
            }
            Err(_) => {
                return RuntimeWaitResult::Exited(
                    "Runtime manager lock was poisoned while waiting for server readiness.".to_string(),
                );
            }
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    RuntimeWaitResult::TimedOut
}

fn startup_timeout_for(candidate: &RuntimeCandidate) -> u64 {
    match candidate.mode.as_str() {
        "cuda" => 180,
        "cpu-avx2" => 300,
        "cpu-avx" => 360,
        _ => 420,
    }
}

pub async fn start_with_fallbacks(
    manager: &std::sync::Mutex<RuntimeManager>,
    paths: &AppPaths,
    mut request: RuntimeStartRequest,
) -> Result<RuntimeStatus, String> {
    request.port = choose_free_port(request.port)?;
    request.context_length = request.context_length.clamp(512, 32768);
    request.gpu_layers = request.gpu_layers.min(256);

    let candidates = candidate_executables(paths, &request.preferred_runtime, request.gpu_layers);

    if candidates.is_empty() {
        let message = format!(
            "No compatible llama-server runtime binary found in {}. Installer builds should bundle a CPU runtime; developer builds can add llama-server-cuda.exe, llama-server-cpu-avx2.exe, llama-server-cpu-avx.exe, llama-server-cpu-basic.exe, or llama-server.exe.",
            paths.runtime_win_dir
        );

        if let Ok(mut locked) = manager.lock() {
            locked.mark_error(paths, message.clone());
        }

        return Err(message);
    }

    let generation = {
        let mut locked = manager
            .lock()
            .map_err(|_| "Runtime manager lock was poisoned.".to_string())?;
        locked.begin_start(paths)
    };

    let mut last_error = String::new();

    for candidate in candidates {
        let launch_result = {
            let mut locked = manager
                .lock()
                .map_err(|_| "Runtime manager lock was poisoned.".to_string())?;
            locked.launch_candidate(paths, &candidate, &request, generation)
        };

        let status_after_launch = match launch_result {
            Ok(status) => status,
            Err(err) => {
                last_error = err;
                append_log(paths, &format!("[LocalChatBox] {last_error} Trying fallback if available."));
                continue;
            }
        };

        let api_key = status_after_launch.api_key.clone().unwrap_or_default();
        let timeout_secs = startup_timeout_for(&candidate);
        match wait_for_ready_or_exit(manager, paths, request.port, &api_key, generation, timeout_secs).await {
            RuntimeWaitResult::Ready => {
                let mut locked = manager
                    .lock()
                    .map_err(|_| "Runtime manager lock was poisoned.".to_string())?;
                return Ok(locked.mark_running(paths));
            }
            RuntimeWaitResult::Cancelled => {
                let mut locked = manager
                    .lock()
                    .map_err(|_| "Runtime manager lock was poisoned.".to_string())?;
                return Ok(locked.status(paths));
            }
            RuntimeWaitResult::Exited(message) => {
                last_error = format!(
                    "{} runtime exited before it became healthy on port {}: {}",
                    candidate.mode, request.port, message
                );
            }
            RuntimeWaitResult::TimedOut => {
                last_error = format!(
                    "{} runtime did not become healthy on port {} within {} seconds.",
                    candidate.mode, request.port, timeout_secs
                );
            }
        }

        {
            let mut locked = manager
                .lock()
                .map_err(|_| "Runtime manager lock was poisoned.".to_string())?;
            append_log(paths, &format!("[LocalChatBox] {last_error} Trying fallback if available."));
            locked.kill_child(paths);
        }
    }

    let message = format!("Local mode failed. {last_error} Use a smaller model, CPU safe mode, or remote endpoint mode.");
    let mut locked = manager
        .lock()
        .map_err(|_| "Runtime manager lock was poisoned.".to_string())?;
    locked.mark_error(paths, message.clone());
    Err(message)
}

async fn post_router_action(status: &RuntimeStatus, endpoint: &str, model_id: &str) -> Result<(), String> {
    let api_key = status
        .api_key
        .clone()
        .ok_or_else(|| "Local runtime auth token is missing. Restart the runtime.".to_string())?;
    let client = http_client(120).await?;
    let url = format!("http://127.0.0.1:{}{}", status.port, endpoint);

    let response = client
        .post(url)
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&json!({ "model": model_id }))
        .send()
        .await
        .map_err(|err| format!("Router request failed: {err}"))?;

    if response.status().is_success() {
        Ok(())
    } else {
        let code = response.status();
        let text = response.text().await.unwrap_or_default();
        Err(format!("Router endpoint returned {code}: {text}"))
    }
}

pub async fn switch_model(
    manager: &std::sync::Mutex<RuntimeManager>,
    paths: &AppPaths,
    mut request: RuntimeStartRequest,
) -> Result<RuntimeStatus, String> {
    request.port = {
        let mut locked = manager
            .lock()
            .map_err(|_| "Runtime manager lock was poisoned.".to_string())?;
        let status = locked.status(paths);
        if status.port >= 1024 {
            status.port
        } else {
            request.port
        }
    };

    let target_model_id = model_id_from_request(&request);

    let current_status = {
        let mut locked = manager
            .lock()
            .map_err(|_| "Runtime manager lock was poisoned.".to_string())?;
        locked.status(paths)
    };

    if current_status.state == "running"
        && current_status.runtime_mode.as_deref() == Some("router")
        && current_status.router_supported
    {
        if let Some(current_model_id) = current_status.loaded_model_id.clone() {
            if current_model_id != target_model_id {
                let _ = post_router_action(&current_status, "/models/unload", &current_model_id).await;
            }
        }

        {
            let mut locked = manager
                .lock()
                .map_err(|_| "Runtime manager lock was poisoned.".to_string())?;
            locked.status.state = "loading".to_string();
            locked.status.message = format!("Loading {target_model_id} through router…");
            locked.status.model_path = Some(request.model_path.clone());
            locked.status.model_name = Some(model_name_from_path(&request.model_path));
            locked.status.loaded_model_id = Some(target_model_id.clone());
            locked.status.model_status = Some("loading".to_string());
            persist_runtime_state(paths, &locked.status);
        }

        post_router_action(&current_status, "/models/load", &target_model_id).await?;

        let result = wait_for_ready_or_exit(
            manager,
            paths,
            current_status.port,
            current_status.api_key.as_deref().unwrap_or_default(),
            current_status.generation,
            300,
        )
        .await;

        match result {
            RuntimeWaitResult::Ready => {
                let mut locked = manager
                    .lock()
                    .map_err(|_| "Runtime manager lock was poisoned.".to_string())?;
                return Ok(locked.mark_running(paths));
            }
            RuntimeWaitResult::Cancelled => {
                let mut locked = manager
                    .lock()
                    .map_err(|_| "Runtime manager lock was poisoned.".to_string())?;
                return Ok(locked.status(paths));
            }
            RuntimeWaitResult::Exited(message) => return Err(message),
            RuntimeWaitResult::TimedOut => return Err("Router model load timed out.".to_string()),
        }
    }

    // Classic fallback: if router mode is unavailable, switch by restarting the runtime.
    start_with_fallbacks(manager, paths, request).await
}

fn chat_url(base: &str) -> String {
    let trimmed = base.trim().trim_end_matches('/');
    if trimmed.ends_with("/chat/completions") {
        trimmed.to_string()
    } else if trimmed.ends_with("/v1") {
        format!("{trimmed}/chat/completions")
    } else {
        format!("{trimmed}/v1/chat/completions")
    }
}

pub async fn send_chat_request(status: RuntimeStatus, request: ChatRequest) -> Result<ChatResponse, String> {
    let (base_url, api_key, model) = if request.mode == "remote" {
        let base = request
            .remote_base_url
            .clone()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "Remote base URL is empty.".to_string())?;
        let model = if request.model.trim().is_empty() {
            return Err("Remote model name is empty.".to_string());
        } else {
            request.model.clone()
        };
        (base, request.remote_api_key.clone().unwrap_or_default(), model)
    } else {
        if status.state != "running" {
            return Err("Local runtime is not running.".to_string());
        }

        let api_key = status
            .api_key
            .clone()
            .ok_or_else(|| "Local runtime auth token is missing. Restart the local runtime.".to_string())?;

        let model = status
            .loaded_model_id
            .clone()
            .or(status.model_name.clone())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| request.model.clone());

        (
            format!("http://127.0.0.1:{}", status.port),
            api_key,
            model,
        )
    };

    let payload = json!({
        "model": model,
        "messages": request.messages,
        "temperature": request.temperature,
        "max_tokens": request.max_tokens,
        "stream": false
    });

    let client = http_client(600).await?;
    let mut builder = client
        .post(chat_url(&base_url))
        .header("Content-Type", "application/json")
        .json(&payload);

    if !api_key.trim().is_empty() {
        builder = builder.header("Authorization", format!("Bearer {}", api_key.trim()));
    }

    let response = builder
        .send()
        .await
        .map_err(|err| format!("Chat request failed: {err}"))?;

    let status_code = response.status();
    let text = response
        .text()
        .await
        .map_err(|err| format!("Could not read chat response: {err}"))?;

    if !status_code.is_success() {
        return Err(format!("Chat endpoint returned {status_code}: {text}"));
    }

    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|err| format!("Chat endpoint returned invalid JSON: {err}. Body: {text}"))?;

    let content = value
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(|content| content.as_str())
        .or_else(|| {
            value
                .get("choices")
                .and_then(|choices| choices.get(0))
                .and_then(|choice| choice.get("text"))
                .and_then(|content| content.as_str())
        })
        .map(|content| content.to_string())
        .ok_or_else(|| format!("Chat response did not contain choices[0].message.content: {value}"))?;

    let model = value
        .get("model")
        .and_then(|model| model.as_str())
        .map(|model| model.to_string());

    Ok(ChatResponse {
        content,
        model,
        raw: value,
    })
}

pub fn read_log(paths: &AppPaths) -> Result<String, String> {
    let path = Path::new(&paths.runtime_log);
    if !path.exists() {
        File::create(path).map_err(|err| err.to_string())?;
        return Ok(String::new());
    }

    let bytes = fs::read(path).map_err(|err| err.to_string())?;
    let max_len = 64 * 1024;
    let slice = if bytes.len() > max_len {
        &bytes[bytes.len() - max_len..]
    } else {
        &bytes[..]
    };

    Ok(String::from_utf8_lossy(slice).to_string())
}
