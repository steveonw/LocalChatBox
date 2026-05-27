export type RuntimeState = "stopped" | "loading" | "running" | "error";

export interface AppPaths {
  root: string;
  models_dir: string;
  runtime_win_dir: string;
  data_dir: string;
  logs_dir: string;
  runtime_log: string;
  engines_dir: string;
  llama_engine_dir: string;
  engine_manifest: string;
  router_preset: string;
  model_registry: string;
  diagnostics_dir: string;
  doctor_report: string;
}

export interface GpuInfo {
  name: string;
  vram_gb: number | null;
}

export interface HardwareProfile {
  cpu_name: string;
  ram_gb: number;
  os_name: string;
  os_version: string;
  supports_sse2: boolean;
  supports_avx: boolean;
  supports_avx2: boolean;
  supports_fma: boolean;
  gpus: GpuInfo[];
  nvidia_gpu_detected: boolean;
  cuda_maybe_usable: boolean;
  disk_free_gb: number | null;
  recommended_tier: string;
  notes: string[];
}

export interface ModelInfo {
  id: string;
  file_name: string;
  display_name: string;
  path: string;
  size_bytes: number;
  size_gb: number;
  estimated_required_ram_gb: number;
  estimated_tier: string;
  quant_hint: string | null;
  family_hint: string | null;
  compatibility: "good_local_fit" | "may_run_slowly" | "too_large" | "remote_recommended";
  compatibility_label: string;
  fit_status: string;
  recommendation: string;
  recommended_context: number;
  recommended_gpu_layers: number;
  reasons: string[];
  last_verified: string;
}

export interface ModelRegistry {
  schema_version: number;
  generated_at: string;
  models: ModelInfo[];
  notes: string[];
}

export interface RuntimeProbeEntry {
  id: string;
  file_name: string;
  path: string;
  exists: boolean;
  status: string;
  version: string | null;
  supported_flags: string[];
  ui_disable_flag: string | null;
  router_supported: boolean;
  error: string | null;
}

export interface EngineManifest {
  schema_version: number;
  engine: string;
  manifest_version: string;
  generated_at: string;
  preferred_backend: string;
  known_good_backend: string | null;
  router_supported: boolean;
  runtimes: RuntimeProbeEntry[];
  notes: string[];
}

export interface DoctorReport {
  schema_version: number;
  generated_at: string;
  result: string;
  next_step: string;
  hardware: HardwareProfile | null;
  engine: EngineManifest;
  model_registry: ModelRegistry;
  findings: string[];
}

export interface LocalSettings {
  port: number;
  context_length: number;
  gpu_layers: number;
  preferred_runtime: "auto" | "cuda" | "cpu-avx2" | "cpu-avx" | "cpu-basic";
  runtime_mode: "auto" | "router" | "classic";
  temperature: number;
  max_tokens: number;
  remote_base_url: string;
  remote_api_key: string;
  remote_model_name: string;
  first_run_complete: boolean;
}

export interface RuntimeStatus {
  state: RuntimeState;
  message: string;
  port: number;
  mode: string | null;
  runtime_mode: string | null;
  backend: string | null;
  router_supported: boolean;
  model_path: string | null;
  model_name: string | null;
  loaded_model_id: string | null;
  model_status: string | null;
  pid: number | null;
  started_at: string | null;
  generation: number;
}

export interface RuntimeStartRequest {
  model_path: string;
  port: number;
  context_length: number;
  gpu_layers: number;
  preferred_runtime: "auto" | "cuda" | "cpu-avx2" | "cpu-avx" | "cpu-basic";
  runtime_mode?: "auto" | "router" | "classic";
  model_id?: string;
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
