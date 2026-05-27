import "./style.css";
import { api } from "./tauri";
import type {
  AppPaths,
  ChatMessage,
  DoctorReport,
  EngineManifest,
  HardwareProfile,
  LocalSettings,
  ModelInfo,
  ModelRegistry,
  RuntimeStatus,
  RuntimeStartRequest,
  StoredChat
} from "./types";

type TabName = "setup" | "chat" | "models" | "engine" | "settings" | "logs";
type ModeName = "local" | "remote";

const defaultSettings: LocalSettings = {
  port: 8080,
  context_length: 2048,
  gpu_layers: 0,
  preferred_runtime: "auto",
  runtime_mode: "auto",
  temperature: 0.7,
  max_tokens: 512,
  remote_base_url: "",
  remote_api_key: "",
  remote_model_name: "",
  first_run_complete: false
};

const stoppedStatus: RuntimeStatus = {
  state: "stopped",
  message: "Runtime stopped.",
  port: 8080,
  mode: null,
  runtime_mode: null,
  backend: null,
  router_supported: false,
  model_path: null,
  model_name: null,
  loaded_model_id: null,
  model_status: null,
  pid: null,
  started_at: null,
  generation: 0
};

const state: {
  paths: AppPaths | null;
  hardware: HardwareProfile | null;
  models: ModelInfo[];
  modelRegistry: ModelRegistry | null;
  engine: EngineManifest | null;
  doctor: DoctorReport | null;
  selectedModelPath: string;
  settings: LocalSettings;
  runtime: RuntimeStatus;
  chats: StoredChat[];
  activeChatId: string;
  activeTab: TabName;
  mode: ModeName;
  busy: boolean;
  notice: { kind: "ok" | "warn" | "error"; text: string } | null;
  runtimeLog: string;
  drafts: {
    chatInput: string;
    systemPrompt: string;
  };
} = {
  paths: null,
  hardware: null,
  models: [],
  modelRegistry: null,
  engine: null,
  doctor: null,
  selectedModelPath: "",
  settings: defaultSettings,
  runtime: stoppedStatus,
  chats: [],
  activeChatId: "",
  activeTab: "setup",
  mode: "local",
  busy: false,
  notice: null,
  runtimeLog: "",
  drafts: {
    chatInput: "",
    systemPrompt: "You are a helpful local AI assistant."
  }
};

const appRoot = document.querySelector<HTMLDivElement>("#app");
if (!appRoot) {
  throw new Error("Missing #app root");
}
const app: HTMLDivElement = appRoot;

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}

function gb(value: number | null | undefined): string {
  if (value === null || value === undefined || Number.isNaN(value)) return "Unknown";
  return `${value.toFixed(value >= 10 ? 0 : 1)} GB`;
}

function bytesToLabel(bytes: number): string {
  const size = bytes / 1024 / 1024 / 1024;
  if (size >= 1) return `${size.toFixed(size >= 10 ? 1 : 2)} GB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function selectedModel(): ModelInfo | null {
  return state.models.find((model) => model.path === state.selectedModelPath) ?? state.models[0] ?? null;
}

function selectedModelId(): string {
  return selectedModel()?.id ?? state.runtime.loaded_model_id ?? state.runtime.model_name ?? "local-model";
}

function activeChat(): StoredChat | null {
  return state.chats.find((chat) => chat.id === state.activeChatId) ?? null;
}

function nowIso(): string {
  return new Date().toISOString();
}

function makeChat(title = "New local chat"): StoredChat {
  const model = state.mode === "local" ? selectedModelId() : state.settings.remote_model_name || "remote-model";
  return {
    id: crypto.randomUUID(),
    title,
    model,
    created_at: nowIso(),
    updated_at: nowIso(),
    messages: []
  };
}

function ensureChat(): StoredChat {
  let chat = activeChat();
  if (chat) return chat;

  chat = makeChat();
  state.chats.unshift(chat);
  state.activeChatId = chat.id;
  void api.saveChats(state.chats);
  return chat;
}

function setNotice(kind: "ok" | "warn" | "error", text: string): void {
  state.notice = { kind, text };
}

function clearNoticeSoon(): void {
  window.setTimeout(() => {
    state.notice = null;
    render();
  }, 5000);
}

function focusedEditable(): boolean {
  const active = document.activeElement;
  if (!active) return false;
  const tag = active.tagName.toLowerCase();
  return tag === "input" || tag === "textarea" || tag === "select";
}

function settingNumber<K extends keyof LocalSettings>(key: K, fallback: number): number {
  const value = state.settings[key];
  if (typeof value === "number" && Number.isFinite(value)) {
    return value;
  }
  return fallback;
}

function runtimeRequest(): RuntimeStartRequest | null {
  const model = selectedModel();
  if (!model) return null;

  return {
    model_path: model.path,
    model_id: model.id,
    port: settingNumber("port", 8080),
    context_length: settingNumber("context_length", model.recommended_context || 2048),
    gpu_layers: settingNumber("gpu_layers", 0),
    preferred_runtime: state.settings.preferred_runtime,
    runtime_mode: state.settings.runtime_mode
  };
}

function runtimePillClass(): string {
  if (state.runtime.state === "running") return "good";
  if (state.runtime.state === "loading") return "warn";
  if (state.runtime.state === "error") return "bad";
  return "";
}

function fitPillClass(model: ModelInfo): string {
  if (model.fit_status === "Fits") return "good";
  if (model.fit_status === "Won’t fit") return "bad";
  return "warn";
}

function setupStepStatus(): string {
  if (!state.hardware) return "Step 1 of 4: scan hardware.";
  if (!state.engine) return "Step 2 of 4: check the local AI engine.";
  if (state.models.length === 0) return "Step 3 of 4: add or import a GGUF model.";
  if (state.runtime.state !== "running") return "Step 4 of 4: start the runtime.";
  return "Ready: start chatting.";
}

function noticeHtml(): string {
  if (!state.notice) return "";
  return `<div class="notice ${state.notice.kind}">${escapeHtml(state.notice.text)}</div>`;
}

function hardwareMetricsHtml(): string {
  if (!state.hardware) {
    return `<div class="empty">No hardware scan yet. Click <strong>Scan Hardware</strong> to let LocalChatBox recommend the safest runtime.</div>`;
  }

  const gpuText = state.hardware.gpus.length
    ? state.hardware.gpus.map((gpu) => `${gpu.name}${gpu.vram_gb ? ` (${gb(gpu.vram_gb)})` : ""}`).join("<br>")
    : "No discrete GPU detected";

  return `
    <div class="hardware-grid">
      <div class="metric"><span>CPU</span><strong>${escapeHtml(state.hardware.cpu_name)}</strong></div>
      <div class="metric"><span>RAM</span><strong>${gb(state.hardware.ram_gb)}</strong></div>
      <div class="metric"><span>CPU features</span><strong>${[
        state.hardware.supports_avx2 ? "AVX2" : "",
        state.hardware.supports_avx ? "AVX" : "",
        state.hardware.supports_fma ? "FMA" : "",
        state.hardware.supports_sse2 ? "SSE2" : ""
      ].filter(Boolean).join(" / ") || "Unknown"}</strong></div>
      <div class="metric"><span>GPU</span><strong>${gpuText}</strong></div>
      <div class="metric"><span>Recommended tier</span><strong>${escapeHtml(state.hardware.recommended_tier)}</strong></div>
      <div class="metric"><span>Disk free</span><strong>${gb(state.hardware.disk_free_gb)}</strong></div>
    </div>
    ${state.hardware.notes.length ? `<ul>${state.hardware.notes.map((note) => `<li>${escapeHtml(note)}</li>`).join("")}</ul>` : ""}
  `;
}

function engineSummaryHtml(): string {
  if (!state.engine) {
    return `<div class="empty">No engine probe yet. Click <strong>Check Runtime</strong>.</div>`;
  }

  const verified = state.engine.runtimes.filter((runtime) => runtime.status === "verified");
  return `
    <div class="hardware-grid">
      <div class="metric"><span>Engine</span><strong>${escapeHtml(state.engine.engine)}</strong></div>
      <div class="metric"><span>Preferred backend</span><strong>${escapeHtml(state.engine.preferred_backend)}</strong></div>
      <div class="metric"><span>Known good backend</span><strong>${escapeHtml(state.engine.known_good_backend ?? "None yet")}</strong></div>
      <div class="metric"><span>Router mode</span><strong>${state.engine.router_supported ? "Supported" : "Not detected"}</strong></div>
      <div class="metric"><span>Verified runtimes</span><strong>${verified.length}</strong></div>
      <div class="metric"><span>Manifest</span><strong>${escapeHtml(state.paths?.engine_manifest ?? "Unknown")}</strong></div>
    </div>
  `;
}

function modelCardsHtml(limit?: number): string {
  const models = typeof limit === "number" ? state.models.slice(0, limit) : state.models;

  if (models.length === 0) {
    return `
      <div class="empty">
        <p>No GGUF models found yet.</p>
        <p>For v0.3 source builds, place a <code>.gguf</code> file in the models folder. Installer builds should add a beginner download/import flow.</p>
        <button class="secondary" data-action="reveal-models">Open Models Folder</button>
      </div>
    `;
  }

  return `
    <div class="model-list">
      ${models
        .map((model) => {
          const selected = model.path === state.selectedModelPath;
          return `
            <label class="model-card ${selected ? "selected" : ""}">
              <input type="radio" name="model" value="${escapeHtml(model.path)}" ${selected ? "checked" : ""}>
              <div>
                <div class="model-title">${escapeHtml(model.display_name || model.file_name)}</div>
                <div class="model-meta">${escapeHtml(model.file_name)} · ${bytesToLabel(model.size_bytes)} · ${escapeHtml(model.estimated_tier)} · ${escapeHtml(model.quant_hint ?? "quant unknown")}</div>
                <p>${escapeHtml(model.recommendation)}</p>
                <div class="pill-row">
                  <span class="pill ${fitPillClass(model)}">${escapeHtml(model.fit_status)}</span>
                  <span class="pill">RAM est. ${gb(model.estimated_required_ram_gb)}</span>
                  <span class="pill">ctx ${model.recommended_context}</span>
                  <span class="pill">GPU layers ${model.recommended_gpu_layers}</span>
                </div>
                ${model.reasons.length ? `<ul class="compact">${model.reasons.map((reason) => `<li>${escapeHtml(reason)}</li>`).join("")}</ul>` : ""}
              </div>
            </label>
          `;
        })
        .join("")}
    </div>
  `;
}

function setupTabHtml(): string {
  return `
    <div class="grid-two">
      <section class="panel">
        <div class="panel-title">
          <div>
            <h2>First-run setup</h2>
            <p>${escapeHtml(setupStepStatus())}</p>
          </div>
          <span class="pill ${state.settings.first_run_complete ? "good" : "warn"}">${state.settings.first_run_complete ? "Completed" : "Wizard"}</span>
        </div>

        <div class="notice ok">
          <strong>Privacy contract:</strong> local mode keeps messages on this computer. Remote mode sends messages to your configured endpoint. Telemetry is off in this source build.
        </div>

        <div class="button-row">
          <button data-action="scan-hardware">1. Scan Hardware</button>
          <button data-action="probe-runtime">2. Check Runtime</button>
          <button data-action="scan-models">3. Scan Models</button>
          <button data-action="run-doctor">Run Doctor</button>
        </div>

        <div class="advisor">
          <h3>Doctor result</h3>
          ${
            state.doctor
              ? `<p><strong>${escapeHtml(state.doctor.result)}</strong>: ${escapeHtml(state.doctor.next_step)}</p><ul>${state.doctor.findings.map((finding) => `<li>${escapeHtml(finding)}</li>`).join("")}</ul>`
              : `<p>Run Doctor after scanning to get a single plain-English readiness report.</p>`
          }
        </div>

        <div class="button-row">
          <button data-action="mark-first-run">Mark setup complete</button>
          <button class="secondary" data-tab="chat">Go to Chat</button>
        </div>
      </section>

      <section class="panel">
        <div class="panel-title">
          <div>
            <h2>Beginner model path</h2>
            <p>v0.3 reads local GGUF files and ranks them by fit. The future installer should add one-click starter model download after license review.</p>
          </div>
        </div>
        ${modelCardsHtml(3)}
      </section>
    </div>

    <div class="grid-two">
      <section class="panel">
        <h2>Hardware</h2>
        ${hardwareMetricsHtml()}
      </section>
      <section class="panel">
        <h2>Engine</h2>
        ${engineSummaryHtml()}
      </section>
    </div>
  `;
}

function chatMessagesHtml(chat: StoredChat | null): string {
  if (!chat || chat.messages.length === 0) {
    return `<div class="empty">No messages yet. Start the runtime, then send a short test prompt.</div>`;
  }

  return chat.messages
    .filter((message) => message.role !== "system")
    .map(
      (message) => `
        <div class="message ${message.role}">
          <div class="role">${escapeHtml(message.role)}</div>
          <div class="content">${escapeHtml(message.content)}</div>
        </div>
      `
    )
    .join("");
}

function chatTabHtml(): string {
  const chat = activeChat();
  const canSendLocal = state.mode === "remote" || state.runtime.state === "running";
  const model = selectedModel();

  return `
    <div class="chat-layout">
      <aside class="chat-sidebar">
        <div class="panel-title">
          <div>
            <h2>Chats</h2>
            <p>Stored locally in <code>data/chats.json</code>.</p>
          </div>
        </div>
        <button class="full" data-action="new-chat">New Chat</button>
        <div class="chat-list">
          ${state.chats
            .map(
              (item) => `
                <button class="chat-item ${item.id === state.activeChatId ? "active" : ""}" data-chat-id="${escapeHtml(item.id)}">
                  <strong>${escapeHtml(item.title)}</strong>
                  <span>${escapeHtml(item.model)}</span>
                </button>
              `
            )
            .join("")}
        </div>
      </aside>

      <main class="chat-main">
        <div class="panel-title">
          <div>
            <h2>Chat</h2>
            <p>${state.mode === "local" ? `Local model: ${escapeHtml(state.runtime.loaded_model_id ?? model?.id ?? "not loaded")}` : `Remote model: ${escapeHtml(state.settings.remote_model_name || "not configured")}`}</p>
          </div>
          <div class="button-row">
            <button class="secondary" data-mode="local">Local</button>
            <button class="secondary" data-mode="remote">Remote</button>
          </div>
        </div>

        <div class="runtime-strip">
          <span class="pill ${runtimePillClass()}">${escapeHtml(state.runtime.state)}</span>
          <span>${escapeHtml(state.runtime.message)}</span>
          <span>mode: ${escapeHtml(state.runtime.runtime_mode ?? "none")}</span>
          <span>backend: ${escapeHtml(state.runtime.backend ?? state.runtime.mode ?? "none")}</span>
          <span>port: ${state.runtime.port}</span>
        </div>

        <div class="button-row">
          <button data-action="start-runtime" ${!model || state.busy ? "disabled" : ""}>Start / Load Selected</button>
          <button class="secondary" data-action="switch-model" ${!model || state.busy ? "disabled" : ""}>Switch Model</button>
          <button class="danger" data-action="stop-runtime" ${state.runtime.state === "stopped" || state.busy ? "disabled" : ""}>Stop</button>
        </div>

        <div class="messages">${chatMessagesHtml(chat)}</div>

        <div class="composer">
          <textarea id="system-prompt" rows="2" placeholder="System prompt">${escapeHtml(state.drafts.systemPrompt)}</textarea>
          <textarea id="chat-input" rows="4" placeholder="Type a message...">${escapeHtml(state.drafts.chatInput)}</textarea>
          <button data-action="send-message" ${state.busy || !canSendLocal ? "disabled" : ""}>Send</button>
        </div>
      </main>
    </div>
  `;
}

function modelsTabHtml(): string {
  return `
    <section class="panel">
      <div class="panel-title">
        <div>
          <h2>Model Registry</h2>
          <p>LocalChatBox v0.3 turns the models folder into a registry with fit labels and reasons.</p>
        </div>
        <div class="button-row">
          <button data-action="scan-models">Refresh Registry</button>
          <button class="secondary" data-action="reveal-models">Open Folder</button>
        </div>
      </div>
      ${modelCardsHtml()}
      <div class="advisor">
        <h3>Registry file</h3>
        <p><code>${escapeHtml(state.paths?.model_registry ?? "Not initialized")}</code></p>
      </div>
    </section>
  `;
}

function engineTabHtml(): string {
  const rows = state.engine?.runtimes
    .map(
      (runtime) => `
        <tr>
          <td>${escapeHtml(runtime.id)}</td>
          <td>${escapeHtml(runtime.file_name)}</td>
          <td><span class="pill ${runtime.status === "verified" ? "good" : runtime.status === "missing" ? "" : "bad"}">${escapeHtml(runtime.status)}</span></td>
          <td>${runtime.router_supported ? "yes" : "no"}</td>
          <td>${escapeHtml(runtime.ui_disable_flag ?? "unknown")}</td>
          <td>${escapeHtml(runtime.version ?? "")}</td>
        </tr>
      `
    )
    .join("");

  return `
    <section class="panel">
      <div class="panel-title">
        <div>
          <h2>Engine Manager</h2>
          <p>Probe runtime binaries, verify supported flags, and cache a manifest before launch.</p>
        </div>
        <div class="button-row">
          <button data-action="probe-runtime">Probe Runtime</button>
          <button data-action="run-doctor">Run Doctor</button>
        </div>
      </div>
      ${engineSummaryHtml()}
      <table>
        <thead>
          <tr><th>ID</th><th>File</th><th>Status</th><th>Router</th><th>UI flag</th><th>Version</th></tr>
        </thead>
        <tbody>${rows || `<tr><td colspan="6">No manifest yet.</td></tr>`}</tbody>
      </table>
      <div class="advisor">
        <h3>Files</h3>
        <p>Manifest: <code>${escapeHtml(state.paths?.engine_manifest ?? "unknown")}</code></p>
        <p>Router preset: <code>${escapeHtml(state.paths?.router_preset ?? "unknown")}</code></p>
      </div>
    </section>
  `;
}

function settingsTabHtml(): string {
  return `
    <section class="panel">
      <div class="panel-title">
        <div>
          <h2>Settings</h2>
          <p>Advanced settings are available, but safe defaults are chosen for first-run use.</p>
        </div>
        <button data-action="save-settings">Save</button>
      </div>

      <div class="settings-grid">
        <label>Preferred runtime
          <select id="preferred-runtime">
            ${["auto", "cuda", "cpu-avx2", "cpu-avx", "cpu-basic"].map((value) => `<option value="${value}" ${state.settings.preferred_runtime === value ? "selected" : ""}>${value}</option>`).join("")}
          </select>
        </label>
        <label>Runtime mode
          <select id="runtime-mode">
            ${["auto", "router", "classic"].map((value) => `<option value="${value}" ${state.settings.runtime_mode === value ? "selected" : ""}>${value}</option>`).join("")}
          </select>
        </label>
        <label>Preferred port
          <input id="port" type="number" min="1024" max="65535" value="${state.settings.port}">
        </label>
        <label>Context length
          <input id="context-length" type="number" min="512" max="32768" step="512" value="${state.settings.context_length}">
        </label>
        <label>GPU layers
          <input id="gpu-layers" type="number" min="0" max="256" value="${state.settings.gpu_layers}">
        </label>
        <label>Temperature
          <input id="temperature" type="number" min="0" max="2" step="0.05" value="${state.settings.temperature}">
        </label>
        <label>Max tokens
          <input id="max-tokens" type="number" min="16" max="8192" value="${state.settings.max_tokens}">
        </label>
        <label>Remote base URL
          <input id="remote-base-url" type="text" value="${escapeHtml(state.settings.remote_base_url)}" placeholder="http://127.0.0.1:1234/v1">
        </label>
        <label>Remote model name
          <input id="remote-model-name" type="text" value="${escapeHtml(state.settings.remote_model_name)}" placeholder="model-id">
        </label>
        <label>Remote API key
          <input id="remote-api-key" type="password" value="${escapeHtml(state.settings.remote_api_key)}">
        </label>
      </div>
    </section>
  `;
}

function logsTabHtml(): string {
  return `
    <section class="panel">
      <div class="panel-title">
        <div>
          <h2>Logs</h2>
          <p>Runtime logs are parent-captured to avoid Windows file-contention from two writers.</p>
        </div>
        <button data-action="refresh-logs">Refresh Logs</button>
      </div>
      <pre class="log-view">${escapeHtml(state.runtimeLog || "No runtime log yet.")}</pre>
    </section>
  `;
}

function render(): void {
  app.innerHTML = `
    <div class="shell">
      <header class="hero">
        <div>
          <div class="eyebrow">LocalChatBox v0.3 · Switchboard Installer Preview</div>
          <h1>Private local chat, with a front door normal users can survive.</h1>
          <p>Scan hardware, verify the engine, rank models, and run llama.cpp behind a Rust IPC boundary.</p>
        </div>
        <div class="status-card">
          <div>
            <span>Status</span>
            <h2>${escapeHtml(state.runtime.state)}</h2>
            <small>${escapeHtml(state.runtime.message)}</small>
          </div>
          <div class="pill-row">
            <span class="pill ${runtimePillClass()}">${escapeHtml(state.runtime.runtime_mode ?? "not running")}</span>
            <span class="pill">${escapeHtml(state.engine?.known_good_backend ?? "no runtime")}</span>
          </div>
        </div>
      </header>

      ${noticeHtml()}

      <nav class="tabs">
        ${(["setup", "chat", "models", "engine", "settings", "logs"] as TabName[])
          .map((tab) => `<button class="${state.activeTab === tab ? "active" : ""}" data-tab="${tab}">${tab}</button>`)
          .join("")}
      </nav>

      ${state.activeTab === "setup" ? setupTabHtml() : ""}
      ${state.activeTab === "chat" ? chatTabHtml() : ""}
      ${state.activeTab === "models" ? modelsTabHtml() : ""}
      ${state.activeTab === "engine" ? engineTabHtml() : ""}
      ${state.activeTab === "settings" ? settingsTabHtml() : ""}
      ${state.activeTab === "logs" ? logsTabHtml() : ""}
    </div>
  `;

  bindEvents();
}

function readSettingsFromDom(): LocalSettings {
  const valueOf = (id: string): string => (document.querySelector<HTMLInputElement | HTMLSelectElement>(`#${id}`)?.value ?? "").trim();
  const intOf = (id: string, fallback: number): number => {
    const parsed = Number.parseInt(valueOf(id), 10);
    return Number.isFinite(parsed) ? parsed : fallback;
  };
  const floatOf = (id: string, fallback: number): number => {
    const parsed = Number.parseFloat(valueOf(id));
    return Number.isFinite(parsed) ? parsed : fallback;
  };

  return {
    ...state.settings,
    preferred_runtime: valueOf("preferred-runtime") as LocalSettings["preferred_runtime"],
    runtime_mode: valueOf("runtime-mode") as LocalSettings["runtime_mode"],
    port: intOf("port", state.settings.port),
    context_length: intOf("context-length", state.settings.context_length),
    gpu_layers: intOf("gpu-layers", state.settings.gpu_layers),
    temperature: floatOf("temperature", state.settings.temperature),
    max_tokens: intOf("max-tokens", state.settings.max_tokens),
    remote_base_url: valueOf("remote-base-url"),
    remote_model_name: valueOf("remote-model-name"),
    remote_api_key: valueOf("remote-api-key")
  };
}

function bindEvents(): void {
  app.querySelectorAll<HTMLButtonElement>("[data-tab]").forEach((button) => {
    button.addEventListener("click", () => {
      const tab = button.dataset.tab as TabName | undefined;
      if (tab) {
        state.activeTab = tab;
        render();
      }
    });
  });

  app.querySelectorAll<HTMLInputElement>("input[name='model']").forEach((input) => {
    input.addEventListener("change", () => {
      state.selectedModelPath = input.value;
      const model = selectedModel();
      if (model) {
        state.settings.context_length = model.recommended_context;
        if (state.settings.gpu_layers === 0) {
          state.settings.gpu_layers = 0;
        }
      }
      render();
    });
  });

  app.querySelectorAll<HTMLButtonElement>("[data-chat-id]").forEach((button) => {
    button.addEventListener("click", () => {
      const chatId = button.dataset.chatId;
      if (chatId) {
        state.activeChatId = chatId;
        render();
      }
    });
  });

  app.querySelectorAll<HTMLButtonElement>("[data-mode]").forEach((button) => {
    button.addEventListener("click", () => {
      const mode = button.dataset.mode as ModeName | undefined;
      if (mode) {
        state.mode = mode;
        render();
      }
    });
  });

  const chatInput = app.querySelector<HTMLTextAreaElement>("#chat-input");
  if (chatInput) {
    chatInput.addEventListener("input", () => {
      state.drafts.chatInput = chatInput.value;
    });
  }

  const systemPrompt = app.querySelector<HTMLTextAreaElement>("#system-prompt");
  if (systemPrompt) {
    systemPrompt.addEventListener("input", () => {
      state.drafts.systemPrompt = systemPrompt.value;
    });
  }

  app.querySelectorAll<HTMLButtonElement>("[data-action]").forEach((button) => {
    button.addEventListener("click", () => {
      void handleAction(button.dataset.action ?? "");
    });
  });
}

async function handleAction(action: string): Promise<void> {
  try {
    switch (action) {
      case "scan-hardware":
        await scanHardware();
        break;
      case "probe-runtime":
        await probeRuntime();
        break;
      case "scan-models":
        await scanModels();
        break;
      case "run-doctor":
        await runDoctor();
        break;
      case "mark-first-run":
        state.settings.first_run_complete = true;
        await api.saveSettings(state.settings);
        setNotice("ok", "First-run setup marked complete.");
        clearNoticeSoon();
        render();
        break;
      case "reveal-models":
        await api.revealModelsFolder();
        break;
      case "start-runtime":
        await startRuntime(false);
        break;
      case "switch-model":
        await startRuntime(true);
        break;
      case "stop-runtime":
        await stopRuntime();
        break;
      case "send-message":
        await sendMessage();
        break;
      case "new-chat":
        {
          const chat = makeChat();
          state.chats.unshift(chat);
          state.activeChatId = chat.id;
          await api.saveChats(state.chats);
          render();
        }
        break;
      case "save-settings":
        state.settings = readSettingsFromDom();
        await api.saveSettings(state.settings);
        setNotice("ok", "Settings saved.");
        clearNoticeSoon();
        render();
        break;
      case "refresh-logs":
        state.runtimeLog = await api.readRuntimeLog();
        render();
        break;
      default:
        break;
    }
  } catch (err) {
    setNotice("error", err instanceof Error ? err.message : String(err));
    render();
  }
}

async function scanHardware(): Promise<void> {
  state.busy = true;
  render();
  try {
    state.hardware = await api.scanHardware();
    setNotice("ok", "Hardware scan complete.");
    clearNoticeSoon();
  } catch (err) {
    setNotice("error", err instanceof Error ? err.message : String(err));
    clearNoticeSoon();
  } finally {
    state.busy = false;
    render();
  }
}

async function probeRuntime(): Promise<void> {
  state.busy = true;
  render();
  try {
    const engine: EngineManifest = await api.probeRuntimeManifest();
    state.engine = engine;
    setNotice(engine.runtimes.some((runtime) => runtime.status === "verified") ? "ok" : "warn", "Runtime probe complete.");
    clearNoticeSoon();
  } catch (err) {
    setNotice("error", err instanceof Error ? err.message : String(err));
    clearNoticeSoon();
  } finally {
    state.busy = false;
    render();
  }
}

async function scanModels(): Promise<void> {
  state.busy = true;
  render();
  try {
    const registry: ModelRegistry = await api.scanModelRegistry();
    state.modelRegistry = registry;
    state.models = registry.models;
    if (!state.selectedModelPath && state.models[0]) {
      state.selectedModelPath = state.models[0].path;
    }
    setNotice(state.models.length ? "ok" : "warn", state.models.length ? `Found ${state.models.length} GGUF model(s).` : "No GGUF models found.");
    clearNoticeSoon();
  } catch (err) {
    setNotice("error", err instanceof Error ? err.message : String(err));
    clearNoticeSoon();
  } finally {
    state.busy = false;
    render();
  }
}

async function runDoctor(): Promise<void> {
  state.busy = true;
  render();
  try {
    const doctor: DoctorReport = await api.runDoctor();
    state.doctor = doctor;
    state.hardware = doctor.hardware;
    state.engine = doctor.engine;
    state.modelRegistry = doctor.model_registry;
    state.models = doctor.model_registry.models;
    if (!state.selectedModelPath && state.models[0]) {
      state.selectedModelPath = state.models[0].path;
    }
    setNotice(doctor.result === "ready" ? "ok" : "warn", doctor.next_step);
    clearNoticeSoon();
  } catch (err) {
    setNotice("error", err instanceof Error ? err.message : String(err));
    clearNoticeSoon();
  } finally {
    state.busy = false;
    render();
  }
}

async function startRuntime(switchOnly: boolean): Promise<void> {
  const request = runtimeRequest();
  if (!request) {
    setNotice("warn", "Add or select a GGUF model first.");
    render();
    return;
  }

  state.busy = true;
  render();
  try {
    state.runtime = switchOnly ? await api.switchLocalModel(request) : await api.startRuntime(request);
    setNotice(state.runtime.state === "running" ? "ok" : "warn", state.runtime.message);
    state.activeTab = "chat";
    clearNoticeSoon();
  } catch (err) {
    setNotice("error", err instanceof Error ? err.message : String(err));
    clearNoticeSoon();
  } finally {
    state.busy = false;
    render();
  }
}

async function stopRuntime(): Promise<void> {
  state.busy = true;
  render();
  try {
    state.runtime = await api.stopRuntime();
    setNotice("ok", "Runtime stopped.");
    clearNoticeSoon();
  } catch (err) {
    setNotice("error", err instanceof Error ? err.message : String(err));
    clearNoticeSoon();
  } finally {
    state.busy = false;
    render();
  }
}

async function sendMessage(): Promise<void> {
  const text = state.drafts.chatInput.trim();
  if (!text) return;

  const chat = ensureChat();
  const systemAdded = chat.messages.length === 0 && state.drafts.systemPrompt.trim() !== "";
  if (systemAdded) {
    chat.messages.push({ role: "system", content: state.drafts.systemPrompt.trim() });
  }

  chat.messages.push({ role: "user", content: text });
  chat.updated_at = nowIso();
  if (chat.title === "New local chat" || chat.title === "New chat") {
    chat.title = text.slice(0, 50);
  }
  state.drafts.chatInput = "";

  state.busy = true;
  render();

  try {
    const response = await api.sendChat({
      mode: state.mode,
      messages: chat.messages,
      temperature: state.settings.temperature,
      max_tokens: state.settings.max_tokens,
      model: state.mode === "local" ? selectedModelId() : state.settings.remote_model_name,
      remote_base_url: state.settings.remote_base_url,
      remote_api_key: state.settings.remote_api_key
    });

    chat.messages.push({ role: "assistant", content: response.content });
    chat.model = response.model ?? (state.mode === "local" ? selectedModelId() : state.settings.remote_model_name);
    chat.updated_at = nowIso();
    await api.saveChats(state.chats);
  } catch (err) {
    // Restore user's draft so they can retry without retyping
    state.drafts.chatInput = text;
    // Remove the user message (and system message if we just added it) from the chat
    chat.messages.pop();
    if (systemAdded) chat.messages.pop();
    setNotice("error", err instanceof Error ? err.message : String(err));
    clearNoticeSoon();
  } finally {
    state.busy = false;
    render();
  }
}

async function pollRuntime(): Promise<void> {
  try {
    const previous = JSON.stringify(state.runtime);
    state.runtime = await api.runtimeStatus();
    if (state.activeTab === "logs") {
      state.runtimeLog = await api.readRuntimeLog();
    }
    const changed = previous !== JSON.stringify(state.runtime);
    if (changed && !focusedEditable()) {
      render();
    }
  } catch {
    // Polling should never interrupt typing or first-run setup.
  }
}

async function init(): Promise<void> {
  try {
    state.paths = await api.initializeWorkspace();
    try {
      state.settings = { ...defaultSettings, ...(await api.loadSettings()) };
    } catch (err) {
      setNotice("warn", err instanceof Error ? err.message : String(err));
    }

    state.chats = await api.loadChats();
    if (state.chats[0]) {
      state.activeChatId = state.chats[0].id;
    }

    try {
      state.hardware = await api.scanHardware();
    } catch {
      // First-run screen will show a scan button.
    }

    try {
      const registry: ModelRegistry = await api.scanModelRegistry();
      state.modelRegistry = registry;
      state.models = registry.models;
      if (state.models[0]) state.selectedModelPath = state.models[0].path;
    } catch {
      state.models = await api.scanModels();
      if (state.models[0]) state.selectedModelPath = state.models[0].path;
    }

    try {
      state.engine = await api.probeRuntimeManifest();
    } catch {
      // Runtime may not exist yet in developer source builds.
    }

    state.runtime = await api.runtimeStatus();

    if (state.settings.first_run_complete && state.models.length > 0) {
      state.activeTab = "chat";
    } else {
      state.activeTab = "setup";
    }

    render();
    window.setInterval(() => {
      void pollRuntime();
    }, 4000);
  } catch (err) {
    app.innerHTML = `<div class="shell"><div class="notice error">${escapeHtml(err instanceof Error ? err.message : String(err))}</div></div>`;
  }
}

void init();
