export interface LocalAiAssetStatus {
  asset_id: string;
  name: string;
  category: string;
  path: string;
  required: boolean;
  exists: boolean;
  size_bytes: number;
  status: "ok" | "missing" | "invalid" | "optional_missing";
  message: string;
}

export interface LocalAiValidationResult {
  local_ai_root: string;
  ready: boolean;
  total_size_bytes: number;
  required_ok_count: number;
  required_missing_count: number;
  optional_missing_count: number;
  assets: LocalAiAssetStatus[];
  warnings: string[];
}
