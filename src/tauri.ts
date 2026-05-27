import { invoke } from "@tauri-apps/api/core";
import type {
  AppPaths,
  ChatRequest,
  ChatResponse,
  LocalSettings,
  ModelInfo,
  RequirementsReport,
  RuntimeStartRequest,
  RuntimeStatus,
  StoredChat
} from "./types";

export const api = {
  checkRequirements: () => invoke<RequirementsReport>("check_requirements"),
  openUrl: (url: string) => invoke<void>("open_url", { url }),
  revealRuntimeFolder: () => invoke<void>("reveal_runtime_folder"),
  initializeWorkspace: () => invoke<AppPaths>("initialize_workspace"),
  scanModels: () => invoke<ModelInfo[]>("scan_models"),
  loadSettings: () => invoke<LocalSettings>("load_settings"),
  saveSettings: (settings: LocalSettings) => invoke<void>("save_settings", { settings }),
  loadChats: () => invoke<StoredChat[]>("load_chats"),
  saveChats: (chats: StoredChat[]) => invoke<void>("save_chats", { chats }),
  startRuntime: (request: RuntimeStartRequest) => invoke<RuntimeStatus>("start_runtime", { request }),
  stopRuntime: () => invoke<RuntimeStatus>("stop_runtime"),
  runtimeStatus: () => invoke<RuntimeStatus>("runtime_status"),
  readRuntimeLog: () => invoke<string>("read_runtime_log"),
  sendChat: (request: ChatRequest) => invoke<ChatResponse>("send_chat", { request }),
  revealModelsFolder: () => invoke<void>("reveal_models_folder")
};
