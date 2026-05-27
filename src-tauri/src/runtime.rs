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
    pub model_path: Option<String>,
    pub model_name: Option<String>,
    pub pid: Option<u32>,
    pub started_at: Option<String>,
    pub generation: u64,
    // The local API key is never serialized to the frontend or persisted to disk.
    // It is passed to llama-server via the LLAMA_API_KEY child process environment
    // variable and stays only in this Rust process.
    #[serde(skip_serializing)]
    pub api_key: Option<String>,
}

impl Default for RuntimeStatus {
    fn default() -> Self {
        Self {
            state: "stopped".to_string(),
            message: "Runtime stopped.".to_string(),
            port: 8080,
            model_path: None,
            model_name: None,
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
    pub status: RuntimeStatus,
    pub generation: u64,
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
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default()
}

fn append_log(paths: &AppPaths, line: &str) {
    if let Some(parent) = Path::new(&paths.runtime_log).parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.runtime_log)
    {
        let _ = writeln!(file, "{line}");
    }
}

fn pump_output<R>(reader: R, log_path: String, prefix: &'static str, secret: String)
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut file = match OpenOptions::new().create(true).append(true).open(&log_path) {
            Ok(f) => f,
            Err(_) => return,
        };
        for line in BufReader::new(reader).lines().map_while(Result::ok) {
            let safe = if !secret.is_empty() {
                line.replace(&secret, "<redacted>")
            } else {
                line
            };
            let _ = writeln!(file, "[{prefix}] {safe}");
        }
    });
}

fn model_name_from_path(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("model.gguf")
        .to_string()
}

fn generate_api_key() -> String {
    let suffix: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(48)
        .map(char::from)
        .collect();
    format!("lcb_{suffix}")
}

fn port_available(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

fn choose_free_port(preferred: u16) -> Result<u16, String> {
    if preferred >= 1024 && port_available(preferred) {
        return Ok(preferred);
    }
    for _ in 0..128 {
        let p: u16 = rand::thread_rng().gen_range(49152..=65535);
        if port_available(p) {
            return Ok(p);
        }
    }
    TcpListener::bind(("127.0.0.1", 0))
        .map_err(|e| format!("Could not reserve a local port: {e}"))?
        .local_addr()
        .map(|a| a.port())
        .map_err(|e| format!("Could not inspect reserved port: {e}"))
}

impl RuntimeManager {
    pub fn status(&mut self, paths: &AppPaths) -> RuntimeStatus {
        self.refresh(paths);
        self.status.clone()
    }

    pub fn stop(&mut self, paths: &AppPaths) -> RuntimeStatus {
        self.generation = self.generation.saturating_add(1);
        self.kill(paths);
        self.status = RuntimeStatus {
            port: self.status.port,
            generation: self.generation,
            ..RuntimeStatus::default()
        };
        self.status.clone()
    }

    fn kill(&mut self, paths: &AppPaths) {
        if let Some(mut child) = self.child.take() {
            append_log(paths, "[LocalChatBox] stopping runtime process");
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    fn refresh(&mut self, paths: &AppPaths) {
        if let Some(child) = self.child.as_mut() {
            match child.try_wait() {
                Ok(Some(code)) => {
                    self.child = None;
                    self.status.state = "error".to_string();
                    self.status.message =
                        format!("Runtime exited unexpectedly (status {code}).");
                    self.status.pid = None;
                    self.status.api_key = None;
                }
                Ok(None) => {}
                Err(e) => {
                    self.status.state = "error".to_string();
                    self.status.message = format!("Could not query runtime process: {e}");
                }
            }
        }
    }

    pub fn launch(
        &mut self,
        paths: &AppPaths,
        sidecar: &Path,
        request: &RuntimeStartRequest,
        generation: u64,
    ) -> Result<RuntimeStatus, String> {
        self.kill(paths);

        if !sidecar.exists() {
            return Err(format!(
                "llama-server not found at {}. \
                 For installer builds the runtime is bundled automatically. \
                 For developer builds place llama-server.exe in runtime/win/.",
                sidecar.display()
            ));
        }

        let model_path = Path::new(&request.model_path);
        if !model_path.exists() {
            return Err(format!("Model file not found: {}", request.model_path));
        }

        let api_key = generate_api_key();
        let model_name = model_name_from_path(&request.model_path);

        let args = [
            "--host", "127.0.0.1",
            "--port", &request.port.to_string(),
            "--model", &request.model_path,
            "--ctx-size", &request.context_length.to_string(),
            "--parallel", "1",
            "--alias", &model_name,
            "--no-webui",
        ];

        append_log(
            paths,
            &format!(
                "[LocalChatBox] launching {} with model {}",
                sidecar.display(),
                model_name
            ),
        );

        let mut command = Command::new(sidecar);
        command
            .args(args)
            .env("LLAMA_API_KEY", &api_key)
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
            .map_err(|e| format!("Could not launch llama-server: {e}"))?;

        let pid = child.id();
        if let Some(stdout) = child.stdout.take() {
            pump_output(stdout, paths.runtime_log.clone(), "llama.stdout", api_key.clone());
        }
        if let Some(stderr) = child.stderr.take() {
            pump_output(stderr, paths.runtime_log.clone(), "llama.stderr", api_key.clone());
        }

        self.status = RuntimeStatus {
            state: "loading".to_string(),
            message: format!("Loading {}…", model_name),
            port: request.port,
            model_path: Some(request.model_path.clone()),
            model_name: Some(model_name),
            pid: Some(pid),
            started_at: Some(now_string()),
            generation,
            api_key: Some(api_key),
        };
        self.child = Some(child);
        Ok(self.status.clone())
    }

    pub fn mark_running(&mut self) -> RuntimeStatus {
        self.status.state = "running".to_string();
        self.status.message = "Model loaded and ready.".to_string();
        self.status.clone()
    }

    pub fn mark_error(&mut self, message: String) -> RuntimeStatus {
        self.status.state = "error".to_string();
        self.status.message = message;
        self.status.pid = None;
        self.status.api_key = None;
        self.status.clone()
    }
}

async fn http_client(timeout_secs: u64) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| format!("Could not build HTTP client: {e}"))
}

enum WaitResult {
    Ready,
    Cancelled,
    Exited(String),
    TimedOut,
}

async fn wait_for_ready(
    manager: &std::sync::Mutex<RuntimeManager>,
    paths: &AppPaths,
    port: u16,
    generation: u64,
) -> WaitResult {
    let client = match http_client(20).await {
        Ok(c) => c,
        Err(e) => return WaitResult::Exited(e),
    };
    let health_url = format!("http://127.0.0.1:{port}/health");
    // 7 minutes: CPU-only inference of large models can be very slow to load
    let max_polls = 420u64 * 2;

    for _ in 0..max_polls {
        if let Ok(r) = client.get(&health_url).send().await {
            if r.status().is_success() {
                return WaitResult::Ready;
            }
        }

        match manager.lock() {
            Ok(mut locked) => {
                if locked.generation != generation {
                    return WaitResult::Cancelled;
                }
                let snapshot = locked.status(paths);
                if snapshot.state == "error" {
                    return WaitResult::Exited(snapshot.message);
                }
            }
            Err(_) => return WaitResult::Exited("Runtime manager lock poisoned.".to_string()),
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    WaitResult::TimedOut
}

pub async fn start(
    manager: &std::sync::Mutex<RuntimeManager>,
    paths: &AppPaths,
    sidecar: &PathBuf,
    mut request: RuntimeStartRequest,
) -> Result<RuntimeStatus, String> {
    request.port = choose_free_port(request.port)?;
    request.context_length = request.context_length.clamp(512, 32768);

    let generation = {
        let mut locked = manager.lock().map_err(|_| "Lock poisoned.".to_string())?;
        locked.generation = locked.generation.saturating_add(1);
        locked.kill(paths);
        locked.generation
    };

    let api_key = {
        let mut locked = manager.lock().map_err(|_| "Lock poisoned.".to_string())?;
        locked.launch(paths, sidecar, &request, generation)?;
        locked.status.api_key.clone().unwrap_or_default()
    };

    let _ = api_key; // held in RuntimeManager.status.api_key; used by send_chat via status()

    match wait_for_ready(manager, paths, request.port, generation).await {
        WaitResult::Ready => {
            let mut locked = manager.lock().map_err(|_| "Lock poisoned.".to_string())?;
            Ok(locked.mark_running())
        }
        WaitResult::Cancelled => {
            let mut locked = manager.lock().map_err(|_| "Lock poisoned.".to_string())?;
            Ok(locked.status(paths))
        }
        WaitResult::Exited(msg) => {
            let mut locked = manager.lock().map_err(|_| "Lock poisoned.".to_string())?;
            Err(locked.mark_error(format!("Runtime exited before becoming ready: {msg}")).message)
        }
        WaitResult::TimedOut => {
            let mut locked = manager.lock().map_err(|_| "Lock poisoned.".to_string())?;
            Err(locked
                .mark_error("Runtime did not respond within 7 minutes.".to_string())
                .message)
        }
    }
}

fn chat_url(base: &str) -> String {
    let t = base.trim().trim_end_matches('/');
    if t.ends_with("/chat/completions") {
        t.to_string()
    } else if t.ends_with("/v1") {
        format!("{t}/chat/completions")
    } else {
        format!("{t}/v1/chat/completions")
    }
}

pub async fn send_chat(status: RuntimeStatus, request: ChatRequest) -> Result<ChatResponse, String> {
    let (base_url, api_key, model) = if request.mode == "remote" {
        let base = request
            .remote_base_url
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| "Remote base URL is empty.".to_string())?;
        if request.model.trim().is_empty() {
            return Err("Remote model name is empty.".to_string());
        }
        (base, request.remote_api_key.unwrap_or_default(), request.model.clone())
    } else {
        if status.state != "running" {
            return Err("Local runtime is not running. Start a model first.".to_string());
        }
        let api_key = status
            .api_key
            .ok_or_else(|| "Local runtime auth token missing. Restart the runtime.".to_string())?;
        let model = status
            .model_name
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| request.model.clone());
        (format!("http://127.0.0.1:{}", status.port), api_key, model)
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
        .map_err(|e| format!("Chat request failed: {e}"))?;

    let status_code = response.status();
    let text = response
        .text()
        .await
        .map_err(|e| format!("Could not read chat response: {e}"))?;

    if !status_code.is_success() {
        return Err(format!("Chat endpoint returned {status_code}: {text}"));
    }

    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("Invalid JSON from chat endpoint: {e}. Body: {text}"))?;

    let content = value
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .or_else(|| {
            value
                .get("choices")
                .and_then(|c| c.get(0))
                .and_then(|c| c.get("text"))
                .and_then(|t| t.as_str())
        })
        .map(|s| s.to_string())
        .ok_or_else(|| {
            format!("Chat response missing choices[0].message.content: {value}")
        })?;

    let model = value
        .get("model")
        .and_then(|m| m.as_str())
        .map(|s| s.to_string());

    Ok(ChatResponse {
        content,
        model,
        raw: value,
    })
}

pub fn read_log(paths: &AppPaths) -> Result<String, String> {
    let path = Path::new(&paths.runtime_log);
    if !path.exists() {
        File::create(path).map_err(|e| e.to_string())?;
        return Ok(String::new());
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let max = 64 * 1024;
    let slice = if bytes.len() > max {
        &bytes[bytes.len() - max..]
    } else {
        &bytes[..]
    };
    Ok(String::from_utf8_lossy(slice).to_string())
}
