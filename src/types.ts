export type RuntimeState = "stopped" | "loading" | "running" | "error";

export interface AppPaths {
  data_root: string;
  models_dir: string;
  settings_file: string;
  chats_file: string;
  runtime_log: string;
}

export interface ModelInfo {
  id: string;
  display_name: string;
  path: string;
  size_gb: number;
  compatibility: string;
  compatibility_label: string;
}

export interface LocalSettings {
  port: number;
  context_length: number;
  temperature: number;
  max_tokens: number;
  remote_base_url: string;
  remote_api_key: string;
  remote_model_name: string;
}

export interface RuntimeStatus {
  state: RuntimeState;
  message: string;
  port: number;
  model_path: string | null;
  model_name: string | null;
  pid: number | null;
  started_at: string | null;
  generation: number;
}

export interface RuntimeStartRequest {
  model_path: string;
  port: number;
  context_length: number;
}

export interface ChatMessage {
  role: "system" | "user" | "assistant";
  content: string;
}

export interface ChatRequest {
  mode: "local" | "remote";
  messages: ChatMessage[];
  temperature: number;
  max_tokens: number;
  model: string;
  remote_base_url?: string;
  remote_api_key?: string;
}

export interface ChatResponse {
  content: string;
  model: string | null;
  raw: unknown;
}

export interface StoredChat {
  id: string;
  title: string;
  model: string;
  created_at: string;
  updated_at: string;
  messages: ChatMessage[];
}
