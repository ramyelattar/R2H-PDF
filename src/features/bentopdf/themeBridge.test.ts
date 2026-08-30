import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const here = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(here, "../../../");

const runtime = fs.readFileSync(
  path.join(projectRoot, "src-tauri", "src", "bentopdf_runtime.rs"),
  "utf8",
);
const theme = fs.readFileSync(
  path.join(
    projectRoot,
    "src-tauri",
    "bentopdf-overlay",
    "r2h-theme.css",
  ),
  "utf8",
);
const shell = fs.readFileSync(
  path.join(
    projectRoot,
    "src-tauri",
    "bentopdf-overlay",
    "r2h-shell.js",
  ),
  "utf8",
);
const manifest = JSON.parse(
  fs
    .readFileSync(
      path.join(
        projectRoot,
        "scripts",
        "bentopdf",
        "integration-manifest.json",
      ),
      "utf8",
    )
    .replace(/^\uFEFF/, ""),
);

const requiredTokens = [
  "--r2h-bg-app",
  "--r2h-bg-surface",
  "--r2h-bg-elevated",
  "--r2h-border",
  "--r2h-border-strong",
  "--r2h-text-primary",
  "--r2h-text-secondary",
  "--r2h-text-muted",
  "--r2h-accent",
  "--r2h-accent-hover",
  "--r2h-focus",
  "--r2h-success",
  "--r2h-warning",
  "--r2h-danger",
  "--r2h-shadow",
  "--r2h-radius-sm",
  "--r2h-radius-md",
  "--r2h-radius-lg",
];

describe("BentoPDF R2H desktop theme bridge", () => {
  it("defines the approved centralized R2H token contract", () => {
    for (const token of requiredTokens) {
      expect(theme).toContain(token);
    }
  });

  it("injects local bridge assets through the loopback runtime", () => {
    expect(runtime).toContain("R2H_THEME_CSS_PATH");
    expect(runtime).toContain("R2H_SHELL_JS_PATH");
    expect(runtime).toContain("inject_r2h_shell");
    expect(runtime).toContain("embedded_r2h_asset");
    expect(runtime).toContain("data-r2h-theme-bridge");
    expect(runtime).toContain("data-r2h-shell-bridge");
  });

  it("removes website chrome while preserving a local legal route", () => {
    expect(shell).toContain("#donation-ribbon");
    expect(shell).toContain("#hero-section");
    expect(shell).toContain("data-r2h-external-hidden");
    expect(shell).toContain("licensing.html");
    expect(shell).toContain("BentoPDF 2.8.6 · AGPL-3.0");
  });

  it("keeps the overlay fully local and network-free", () => {
    for (const forbidden of [
      "fetch(",
      "XMLHttpRequest",
      "WebSocket",
      "EventSource",
    ]) {
      expect(theme).not.toContain(forbidden);
      expect(shell).not.toContain(forbidden);
    }
  });

  it("records the bridge without modifying the pinned upstream submodule", () => {
    expect(manifest.themeBridgeEnabled).toBe(true);
    expect(manifest.themeBridgeImplementation).toBe(
      "tauri-runtime-html-injection",
    );
    expect(manifest.desktopChromeSuppressionEnabled).toBe(true);
    expect(manifest.upstreamSubmoduleModified).toBe(false);
    expect(manifest.upstreamCommit).toBe(
      "21c924a3e6a7ce28740535a5bc6b74f872fcdcb5",
    );
  });
});
