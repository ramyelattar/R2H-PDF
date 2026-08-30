export interface LocalModelInfo {
  id: string;
  name: string;
  model_type: string;
  format: string;
  path: string;
  runtime: string;
  exists: boolean;
  size_bytes: number;
  default: boolean;
  context_window: number;
}

export interface LocalRuntimeStatus {
  available: boolean;
  runtime_path: string;
  runtime_version: string;
  default_model_id: string | null;
  errors: string[];
}

export interface LocalGenerateRequest {
  model_id: string;
  prompt: string;
  system_prompt: string | null;
  max_tokens: number | null;
  temperature: number | null;
  top_p: number | null;
  timeout_ms: number | null;
}

export interface LocalGenerateResult {
  model_id: string;
  text: string;
  tokens_generated: number | null;
  elapsed_ms: number;
  finish_reason: string;
  warnings: string[];
}
