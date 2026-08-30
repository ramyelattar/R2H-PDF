import type { CommandItem } from "../types/shell";
import { R2H_FEATURE_OWNERSHIP } from "./r2hFeatureOwnership";

/**
 * Production command definitions are behavior, not user data.  User-facing
 * records (workspaces, recent files, reviews, handoffs, and exports) are
 * loaded from the typed backend library boundary instead.
 */
export const commandCatalog: CommandItem[] = [
  {
    id: "cmd.open.commandPalette",
    title: "Open Command Palette",
    description: "Search commands and document actions",
    category: "System",
    shortcut: "Ctrl+K",
  },
  {
    id: "cmd.open.preferences",
    title: "Open Preferences",
    description: "View workstation and workflow settings",
    category: "System",
    shortcut: "Ctrl+,",
  },
  {
    id: "cmd.open.diagnostics",
    title: "Toggle Diagnostics Viewer",
    description: "Inspect logs and system events",
    category: "System",
    shortcut: "Ctrl+Shift+D",
  },
  {
    id: "cmd.tab.next",
    title: "Next Tab",
    description: "Switch to the next open tab",
    category: "Documents",
    shortcut: "Ctrl+Tab",
    requiresDocument: true,
  },
  {
    id: "cmd.tab.close",
    title: "Close Current Tab",
    description: "Close the active document tab",
    category: "Documents",
    shortcut: "Ctrl+W",
    requiresDocument: true,
  },
  ...R2H_FEATURE_OWNERSHIP.map((feature) => ({
    id: feature.commandId,
    title: feature.commandTitle,
    description: feature.commandDescription,
    category: "Panels" as const,
    shortcut: "",
    requiresDocument: feature.requiresDocument,
  })),
];
