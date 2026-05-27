import { invoke } from "@tauri-apps/api/core";
import type {
  AppPaths,
  ChatRequest,
  ChatResponse,
  DoctorReport,
  EngineManifest,
  HardwareProfile,
  LocalSettings,
  ModelInfo,
  ModelRegistry,
  RuntimeStartRequest,
  RuntimeStatus,
  StoredChat
} from "./types";

export const api = {
  initializeWorkspace: () => invoke<AppPaths>("initialize_workspace"),
  scanHardware: () => invoke<HardwareProfile>("scan_hardware"),
  scanModels: () => invoke<ModelInfo[]>("scan_models"),
  scanModelRegistry: () => invoke<ModelRegistry>("scan_model_registry"),
  probeRuntimeManifest: () => invoke<EngineManifest>("probe_runtime_manifest"),
  runDoctor: () => invoke<DoctorReport>("run_doctor"),
  loadSettings: () => invoke<LocalSettings>("load_settings"),
  saveSettings: (settings: LocalSettings) => invoke<void>("save_settings", { settings }),
  startRuntime: (request: RuntimeStartRequest) => invoke<RuntimeStatus>("start_runtime", { request }),
  switchLocalModel: (request: RuntimeStartRequest) => invoke<RuntimeStatus>("switch_local_model", { request }),
  stopRuntime: () => invoke<RuntimeStatus>("stop_runtime"),
  runtimeStatus: () => invoke<RuntimeStatus>("runtime_status"),
  readRuntimeLog: () => invoke<string>("read_runtime_log"),
  sendChat: (request: ChatRequest) => invoke<ChatResponse>("send_chat", { request }),
  loadChats: () => invoke<StoredChat[]>("load_chats"),
  saveChats: (chats: StoredChat[]) => invoke<void>("save_chats", { chats }),
  revealModelsFolder: () => invoke<void>("reveal_models_folder")
};
