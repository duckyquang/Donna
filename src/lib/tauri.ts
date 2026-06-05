import { isTauri } from "@tauri-apps/api/core";

export const DESKTOP_REQUIRED_MESSAGE =
  "Donna must run as the desktop app, not in a browser preview. From the project folder, run: npm run tauri:dev";

export function isDesktopApp(): boolean {
  return isTauri();
}

export function ensureDesktopApp(): void {
  if (!isDesktopApp()) {
    throw new Error(DESKTOP_REQUIRED_MESSAGE);
  }
}
