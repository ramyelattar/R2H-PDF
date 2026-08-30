import { invoke } from "@tauri-apps/api/core";

export interface BentoPdfStatus {
  installed: boolean;
  running: boolean;
  bundlePath: string | null;
  bundleSource: string | null;
  origin: string | null;
  checkedPaths: string[];
  validationError: string | null;
}

export interface BentoPdfOpenResult {
  reusedExistingWindow: boolean;
  windowLabel: string;
  url: string;
  bundlePath: string;
  bundleSource: string;
}

export const getBentoPdfStatus = (): Promise<BentoPdfStatus> =>
  invoke<BentoPdfStatus>("bentopdf_get_status");

export const openBentoPdfTools = (): Promise<BentoPdfOpenResult> =>
  invoke<BentoPdfOpenResult>("bentopdf_open");