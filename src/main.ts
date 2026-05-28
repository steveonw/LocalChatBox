import "./style.css";
import { api } from "./tauri";
import type {
  ChatMessage,
  LocalSettings,
  ModelInfo,
  RequirementsReport,
  RuntimeStartRequest,
  RuntimeStatus,
  StoredChat
} from "./types";

type TabName = "setup" | "models" | "chat" | "settings" | "logs";
type ModeName = "local" | "remote";

const defaultSettings: LocalSettings = {
  port: 8080,
  context_length: 2048,
  temperature: 0.7,
  max_tokens: 512,
  remote_base_url: "",
  remote_api_key: "",
  remote_model_name: ""
};

const stoppedStatus: RuntimeStatus = {
  state: "stopped",
  message: "Runtime stopped.",
  port: 8080,
  model_path: null,
  model_name: null,
  pid: null,
  started_at: null,
  generation: 0
};

const state: {
  requirements: RequirementsReport | null;
  models: ModelInfo[];
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
  drafts: { chatInput: string; systemPrompt: string };
} = {
  requirements: null,
  models: [],
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
    systemPrompt: "You are a helpful assistant."
  }
};

const appRoot = document.querySelector<HTMLDivElement>("#app");
if (!appRoot) throw new Error("Missing #app root");
const app: HTMLDivElement = appRoot;

// ── Utilities ──────────────────────────────────────────────────────────────

function escapeHtml(v: string): string {
  return v
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}

function gbLabel(v: number): string {
  return v >= 10 ? `${v.toFixed(0)} GB` : `${v.toFixed(1)} GB`;
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
  const el = document.activeElement;
  if (!el) return false;
  const tag = el.tagName.toLowerCase();
  return tag === "input" || tag === "textarea" || tag === "select";
}

function nowIso(): string {
  return new Date().toISOString();
}

// ── Model / chat helpers ───────────────────────────────────────────────────

function selectedModel(): ModelInfo | null {
  return state.models.find((m) => m.path === state.selectedModelPath) ?? state.models[0] ?? null;
}

function activeChat(): StoredChat | null {
  return state.chats.find((c) => c.id === state.activeChatId) ?? null;
}

function makeChat(title = "New chat"): StoredChat {
  const model =
    state.mode === "local"
      ? (state.runtime.model_name ?? selectedModel()?.id ?? "local")
      : (state.settings.remote_model_name || "remote");
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

function runtimeRequest(): RuntimeStartRequest | null {
  const model = selectedModel();
  if (!model) return null;
  return {
    model_path: model.path,
    port: state.settings.port,
    context_length: state.settings.context_length
  };
}

// ── HTML builders ──────────────────────────────────────────────────────────

function runtimePillClass(): string {
  if (state.runtime.state === "running") return "good";
  if (state.runtime.state === "loading") return "warn";
  if (state.runtime.state === "error") return "bad";
  return "";
}

function compatPillClass(compat: string): string {
  if (compat === "good") return "good";
  if (compat === "too_large") return "bad";
  return "warn";
}

function noticeHtml(): string {
  if (!state.notice) return "";
  return `<div class="notice ${state.notice.kind}">${escapeHtml(state.notice.text)}</div>`;
}

function modelCardsHtml(): string {
  if (state.models.length === 0) {
    return `
      <div class="empty">
        <p>No GGUF models found in your models folder.</p>
        <p>Add a <code>.gguf</code> file, then click <strong>Refresh</strong>.</p>
        <button class="secondary" data-action="reveal-models">Open Models Folder</button>
      </div>
    `;
  }

  return `
    <div class="model-list">
      ${state.models
        .map((model) => {
          const selected = model.path === state.selectedModelPath;
          return `
            <label class="model-card ${selected ? "selected" : ""}">
              <input type="radio" name="model" value="${escapeHtml(model.path)}" ${selected ? "checked" : ""}>
              <div class="model-card-body">
                <div class="model-title">${escapeHtml(model.display_name)}</div>
                <div class="model-meta">${gbLabel(model.size_gb)}</div>
                <span class="pill ${compatPillClass(model.compatibility)}">${escapeHtml(model.compatibility_label)}</span>
              </div>
            </label>
          `;
        })
        .join("")}
    </div>
  `;
}

function reqRow(ok: boolean, label: string, detail: string, actions: string): string {
  return `
    <div class="req-row ${ok ? "req-ok" : "req-missing"}">
      <div class="req-icon">${ok ? "✅" : "❌"}</div>
      <div class="req-body">
        <div class="req-label">${label}</div>
        <div class="req-detail">${escapeHtml(detail)}</div>
        ${ok ? "" : `<div class="req-actions">${actions}</div>`}
      </div>
    </div>
  `;
}

function setupTabHtml(): string {
  const r = state.requirements;
  if (!r) {
    return `<section class="panel"><p>Checking requirements…</p></section>`;
  }

  const allGood = r.runtime_found && r.model_count > 0;

  const runtimeRow = reqRow(
    r.runtime_found,
    "AI runtime",
    r.runtime_found
      ? `Found: ${r.runtime_path}`
      : `Not found. Expected location: ${r.runtime_path}`,
    `<button data-action="open-runtime-releases">Download from llama.cpp releases ↗</button>
     <button class="secondary" data-action="open-runtime-folder">Open runtime folder</button>
     <p class="req-hint">Download a Windows CPU build (e.g. <code>llama-b…-bin-win-cpu-x64.zip</code>),
     extract <code>llama-server.exe</code>, and place it in the folder above.</p>`
  );

  const modelsRow = reqRow(
    r.model_count > 0,
    "GGUF model",
    r.model_count > 0
      ? `${r.model_count} model${r.model_count === 1 ? "" : "s"} found in ${r.models_dir}`
      : `No models found. Models folder: ${r.models_dir}`,
    `<button data-action="open-model-hub">Browse GGUF models on HuggingFace ↗</button>
     <button class="secondary" data-action="reveal-models">Open models folder</button>
     <p class="req-hint">Download any <code>.gguf</code> file and place it in the folder above.
     Smaller models (under 4 GB) work best for CPU-only inference.</p>`
  );

  return `
    <section class="panel setup-panel">
      <div class="panel-title">
        <div>
          <h2>${allGood ? "Ready to chat" : "Setup required"}</h2>
          <p>${allGood
            ? "Both requirements are met. Head to the Models tab to start a model."
            : "LocalChatBox needs a runtime and at least one model before it can chat locally."}</p>
        </div>
        <button data-action="check-requirements" ${state.busy ? "disabled" : ""}>
          Check again
        </button>
      </div>

      <div class="req-list">
        ${runtimeRow}
        ${modelsRow}
      </div>

      ${allGood ? `
        <div class="button-row">
          <button data-tab="models">Go to Models →</button>
        </div>
      ` : `
        <p class="req-footer">
          After placing the files, click <strong>Check again</strong>.
          <button class="link-btn" data-tab="models">Continue anyway</button>
        </p>
      `}
    </section>
  `;
}

function modelsTabHtml(): string {
  const model = selectedModel();
  const canStart = !!model && !state.busy && state.runtime.state !== "loading";
  return `
    <section class="panel">
      <div class="panel-title">
        <div>
          <h2>Models</h2>
          <p>GGUF models in <code>%LOCALAPPDATA%\\LocalChatBox\\models</code>. Select one and start the runtime.</p>
        </div>
        <div class="button-row">
          <button data-action="scan-models" ${state.busy ? "disabled" : ""}>Refresh</button>
          <button class="secondary" data-action="reveal-models">Open Folder</button>
        </div>
      </div>
      ${modelCardsHtml()}
    </section>

    <section class="panel runtime-panel">
      <div class="panel-title">
        <h2>Runtime</h2>
        <span class="pill ${runtimePillClass()}">${escapeHtml(state.runtime.state)}</span>
      </div>
      <p>${escapeHtml(state.runtime.message)}</p>
      <div class="button-row">
        <button data-action="start-runtime" ${!canStart ? "disabled" : ""}>
          ${state.runtime.state === "running" ? "Restart with selected" : "Start selected model"}
        </button>
        <button class="danger" data-action="stop-runtime"
          ${state.runtime.state === "stopped" || state.busy ? "disabled" : ""}>Stop</button>
        <button class="secondary" data-tab="chat">Go to Chat →</button>
      </div>
    </section>
  `;
}

function chatMessagesHtml(chat: StoredChat | null): string {
  if (!chat || chat.messages.length === 0) {
    return `<div class="empty">No messages yet. Start a model, then send a message.</div>`;
  }
  return chat.messages
    .filter((m) => m.role !== "system")
    .map(
      (m) => `
        <div class="message ${escapeHtml(m.role)}">
          <div class="role">${escapeHtml(m.role)}</div>
          <div class="content">${escapeHtml(m.content)}</div>
        </div>
      `
    )
    .join("");
}

function chatTabHtml(): string {
  const chat = activeChat();
  const canSend =
    !state.busy &&
    (state.mode === "remote" || state.runtime.state === "running");

  return `
    <div class="chat-layout">
      <aside class="chat-sidebar">
        <button class="full" data-action="new-chat">+ New Chat</button>
        <div class="chat-list">
          ${state.chats
            .map(
              (c) => `
                <button class="chat-item ${c.id === state.activeChatId ? "active" : ""}"
                  data-chat-id="${escapeHtml(c.id)}">
                  <strong>${escapeHtml(c.title)}</strong>
                  <span>${escapeHtml(c.model)}</span>
                </button>
              `
            )
            .join("")}
        </div>
      </aside>

      <main class="chat-main">
        <div class="chat-header">
          <div class="runtime-strip">
            <span class="pill ${runtimePillClass()}">${escapeHtml(state.runtime.state)}</span>
            <span>${escapeHtml(state.runtime.model_name ?? "no model loaded")}</span>
            ${state.runtime.state === "running" ? `<span>port ${state.runtime.port}</span>` : ""}
          </div>
          <div class="button-row">
            <button class="secondary${state.mode === "local" ? " active" : ""}" data-mode="local">Local</button>
            <button class="secondary${state.mode === "remote" ? " active" : ""}" data-mode="remote">Remote</button>
          </div>
        </div>

        <div class="messages" id="messages">${chatMessagesHtml(chat)}</div>

        <div class="composer">
          <textarea id="system-prompt" rows="2"
            placeholder="System prompt (optional)">${escapeHtml(state.drafts.systemPrompt)}</textarea>
          <textarea id="chat-input" rows="4"
            placeholder="Type a message… (Enter to send)">${escapeHtml(state.drafts.chatInput)}</textarea>
          <button data-action="send-message" ${!canSend ? "disabled" : ""}>
            ${state.busy ? "Thinking…" : "Send"}
          </button>
        </div>
      </main>
    </div>
  `;
}

function settingsTabHtml(): string {
  const s = state.settings;
  return `
    <section class="panel">
      <div class="panel-title">
        <div>
          <h2>Settings</h2>
          <p>Saved automatically on change.</p>
        </div>
        <button data-action="save-settings">Save</button>
      </div>
      <div class="settings-grid">
        <label>Preferred port
          <input id="port" type="number" min="1024" max="65535" value="${s.port}">
        </label>
        <label>Context length
          <input id="context-length" type="number" min="512" max="32768" step="512"
            value="${s.context_length}">
        </label>
        <label>Temperature
          <input id="temperature" type="number" min="0" max="2" step="0.05"
            value="${s.temperature}">
        </label>
        <label>Max tokens
          <input id="max-tokens" type="number" min="16" max="8192" value="${s.max_tokens}">
        </label>
        <label class="full-width">Remote base URL
          <input id="remote-base-url" type="text" value="${escapeHtml(s.remote_base_url)}"
            placeholder="http://127.0.0.1:1234/v1">
        </label>
        <label>Remote model name
          <input id="remote-model-name" type="text"
            value="${escapeHtml(s.remote_model_name)}" placeholder="model-id">
        </label>
        <label>Remote API key
          <input id="remote-api-key" type="password" value="${escapeHtml(s.remote_api_key)}">
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
          <h2>Runtime Log</h2>
          <p>Last 64 KB of llama-server output.</p>
        </div>
        <button data-action="refresh-logs">Refresh</button>
      </div>
      <pre class="log-view">${escapeHtml(state.runtimeLog || "No log yet.")}</pre>
    </section>
  `;
}

function render(): void {
  app.innerHTML = `
    <div class="shell">
      <header class="topbar">
        <span class="app-name">LocalChatBox</span>
        <div class="status-pill-row">
          <span class="pill ${runtimePillClass()}">${escapeHtml(state.runtime.state)}</span>
          ${state.runtime.model_name
            ? `<span class="model-tag">${escapeHtml(state.runtime.model_name)}</span>`
            : ""}
        </div>
      </header>

      ${noticeHtml()}

      <nav class="tabs">
        ${(["setup", "models", "chat", "settings", "logs"] as TabName[])
          .map((tab) => {
            const needsAttention =
              tab === "setup" &&
              state.requirements !== null &&
              (!state.requirements.runtime_found || state.requirements.model_count === 0);
            return `<button class="${state.activeTab === tab ? "active" : ""}${needsAttention ? " tab-warn" : ""}"
              data-tab="${tab}">${tab}${needsAttention ? " ⚠" : ""}</button>`;
          })
          .join("")}
      </nav>

      <div class="tab-content">
        ${state.activeTab === "setup" ? setupTabHtml() : ""}
        ${state.activeTab === "models" ? modelsTabHtml() : ""}
        ${state.activeTab === "chat" ? chatTabHtml() : ""}
        ${state.activeTab === "settings" ? settingsTabHtml() : ""}
        ${state.activeTab === "logs" ? logsTabHtml() : ""}
      </div>
    </div>
  `;

  bindEvents();

  if (state.activeTab === "chat") {
    const messages = document.getElementById("messages");
    if (messages) messages.scrollTop = messages.scrollHeight;
  }
}

// ── Event binding ──────────────────────────────────────────────────────────

function readSettingsFromDom(): LocalSettings {
  const val = (id: string) =>
    (document.querySelector<HTMLInputElement | HTMLSelectElement>(`#${id}`)?.value ?? "").trim();
  const int = (id: string, fb: number) => {
    const n = Number.parseInt(val(id), 10);
    return Number.isFinite(n) ? n : fb;
  };
  const float = (id: string, fb: number) => {
    const n = Number.parseFloat(val(id));
    return Number.isFinite(n) ? n : fb;
  };
  return {
    port: int("port", state.settings.port),
    context_length: int("context-length", state.settings.context_length),
    temperature: float("temperature", state.settings.temperature),
    max_tokens: int("max-tokens", state.settings.max_tokens),
    remote_base_url: val("remote-base-url"),
    remote_model_name: val("remote-model-name"),
    remote_api_key: val("remote-api-key")
  };
}

function bindEvents(): void {
  app.querySelectorAll<HTMLButtonElement>("[data-tab]").forEach((btn) => {
    btn.addEventListener("click", () => {
      const tab = btn.dataset.tab as TabName | undefined;
      if (tab) {
        state.activeTab = tab;
        render();
      }
    });
  });

  app.querySelectorAll<HTMLInputElement>("input[name='model']").forEach((input) => {
    input.addEventListener("change", () => {
      state.selectedModelPath = input.value;
      render();
    });
  });

  app.querySelectorAll<HTMLButtonElement>("[data-chat-id]").forEach((btn) => {
    btn.addEventListener("click", () => {
      const chatId = btn.dataset.chatId;
      if (chatId) {
        state.activeChatId = chatId;
        render();
      }
    });
  });

  app.querySelectorAll<HTMLButtonElement>("[data-mode]").forEach((btn) => {
    btn.addEventListener("click", () => {
      const mode = btn.dataset.mode as ModeName | undefined;
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
    chatInput.addEventListener("keydown", (e) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        void handleAction("send-message");
      }
    });
  }

  const systemPrompt = app.querySelector<HTMLTextAreaElement>("#system-prompt");
  if (systemPrompt) {
    systemPrompt.addEventListener("input", () => {
      state.drafts.systemPrompt = systemPrompt.value;
    });
  }

  app.querySelectorAll<HTMLButtonElement>("[data-action]").forEach((btn) => {
    btn.addEventListener("click", () => {
      void handleAction(btn.dataset.action ?? "");
    });
  });
}

// ── Action handlers ────────────────────────────────────────────────────────

async function handleAction(action: string): Promise<void> {
  switch (action) {
    case "check-requirements":
      await checkRequirements();
      break;
    case "open-runtime-releases":
      await api.openUrl("https://github.com/ggerganov/llama.cpp/releases");
      break;
    case "open-runtime-folder":
      await api.revealRuntimeFolder();
      break;
    case "open-model-hub":
      await api.openUrl("https://huggingface.co/models?library=gguf&sort=downloads");
      break;
    case "scan-models":
      await scanModels();
      break;
    case "reveal-models":
      await api.revealModelsFolder();
      break;
    case "start-runtime":
      await startRuntime();
      break;
    case "stop-runtime":
      await stopRuntime();
      break;
    case "send-message":
      await sendMessage();
      break;
    case "new-chat": {
      const chat = makeChat();
      state.chats.unshift(chat);
      state.activeChatId = chat.id;
      await api.saveChats(state.chats);
      render();
      break;
    }
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
}

async function checkRequirements(): Promise<void> {
  state.busy = true;
  render();
  try {
    state.requirements = await api.checkRequirements();
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
    state.models = await api.scanModels();
    if (!state.selectedModelPath && state.models[0]) {
      state.selectedModelPath = state.models[0].path;
    }
    setNotice(
      state.models.length ? "ok" : "warn",
      state.models.length
        ? `Found ${state.models.length} model${state.models.length === 1 ? "" : "s"}.`
        : "No GGUF models found. Add a .gguf file to the models folder."
    );
    clearNoticeSoon();
  } catch (err) {
    setNotice("error", err instanceof Error ? err.message : String(err));
    clearNoticeSoon();
  } finally {
    state.busy = false;
    render();
  }
}

async function startRuntime(): Promise<void> {
  const request = runtimeRequest();
  if (!request) {
    setNotice("warn", "Select a GGUF model first.");
    render();
    return;
  }

  state.busy = true;
  render();
  try {
    state.runtime = await api.startRuntime(request);
    if (state.runtime.state === "running") {
      setNotice("ok", `${state.runtime.model_name ?? "Model"} ready.`);
      state.activeTab = "chat";
    } else {
      setNotice("warn", state.runtime.message);
    }
    clearNoticeSoon();
  } catch (err) {
    state.runtime = { ...stoppedStatus, state: "error", message: err instanceof Error ? err.message : String(err) };
    setNotice("error", state.runtime.message);
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
  const systemAdded =
    chat.messages.length === 0 && state.drafts.systemPrompt.trim() !== "";
  if (systemAdded) {
    chat.messages.push({ role: "system", content: state.drafts.systemPrompt.trim() });
  }
  chat.messages.push({ role: "user", content: text });
  chat.updated_at = nowIso();
  if (chat.title === "New chat") chat.title = text.slice(0, 50);
  state.drafts.chatInput = "";
  state.busy = true;
  render();

  try {
    const response = await api.sendChat({
      mode: state.mode,
      messages: chat.messages,
      temperature: state.settings.temperature,
      max_tokens: state.settings.max_tokens,
      model:
        state.mode === "local"
          ? (state.runtime.model_name ?? "local-model")
          : state.settings.remote_model_name,
      remote_base_url: state.settings.remote_base_url,
      remote_api_key: state.settings.remote_api_key
    });

    chat.messages.push({ role: "assistant", content: response.content });
    chat.model =
      response.model ??
      (state.mode === "local" ? (state.runtime.model_name ?? "local") : state.settings.remote_model_name);
    chat.updated_at = nowIso();
    await api.saveChats(state.chats);
  } catch (err) {
    // Restore draft so the user can retry without retyping
    state.drafts.chatInput = text;
    chat.messages.pop();
    if (systemAdded) chat.messages.pop();
    setNotice("error", err instanceof Error ? err.message : String(err));
    clearNoticeSoon();
  } finally {
    state.busy = false;
    render();
  }
}

// ── Poll runtime state ─────────────────────────────────────────────────────

async function pollRuntime(): Promise<void> {
  try {
    const prev = state.runtime.state;
    state.runtime = await api.runtimeStatus();
    if (state.activeTab === "logs") {
      state.runtimeLog = await api.readRuntimeLog();
    }
    if (state.runtime.state !== prev && !focusedEditable()) {
      render();
    }
  } catch {
    // Polling should never interrupt typing
  }
}

// ── Init ───────────────────────────────────────────────────────────────────

async function init(): Promise<void> {
  try {
    await api.initializeWorkspace();

    try {
      state.settings = { ...defaultSettings, ...(await api.loadSettings()) };
    } catch {
      // Keep defaults; non-fatal on first run
    }

    state.chats = await api.loadChats();
    if (state.chats[0]) state.activeChatId = state.chats[0].id;

    state.requirements = await api.checkRequirements();
    const ready = state.requirements.runtime_found && state.requirements.model_count > 0;

    state.models = await api.scanModels();
    if (state.models[0]) state.selectedModelPath = state.models[0].path;

    state.runtime = await api.runtimeStatus();

    if (!ready) {
      state.activeTab = "setup";
    } else if (state.runtime.state === "running" || state.chats.length > 0) {
      state.activeTab = "chat";
    } else {
      state.activeTab = "models";
    }

    render();
    window.setInterval(() => void pollRuntime(), 4000);
  } catch (err) {
    app.innerHTML = `<div class="shell"><div class="notice error">${escapeHtml(
      err instanceof Error ? err.message : String(err)
    )}</div></div>`;
  }
}

void init();
