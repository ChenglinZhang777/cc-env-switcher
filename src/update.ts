export type UpdateResult =
  | { kind: "available"; version: string; notes: string }
  | { kind: "current" }
  | { kind: "failed" };

export type UpdatePresentation = { title: string; detail: string; canInstall: boolean };

export function presentUpdateResult(result: UpdateResult): UpdatePresentation {
  if (result.kind === "available") {
    return { title: `发现新版本 ${result.version}`, detail: result.notes, canInstall: true };
  }
  if (result.kind === "current") {
    return { title: "已是最新版本", detail: "", canInstall: false };
  }
  return { title: "暂时无法检查更新", detail: "不影响当前使用，可稍后重试。", canInstall: false };
}

export type AvailableUpdate = { update: Update; version: string; notes: string };

export async function checkForUpdate(): Promise<AvailableUpdate | null> {
  const update = await check();
  return update ? { update, version: update.version, notes: update.body ?? "" } : null;
}

export async function installUpdate(update: Update, onProgress: (event: DownloadEvent) => void): Promise<void> {
  await update.downloadAndInstall(onProgress);
  await relaunch();
}
import { check, type DownloadEvent, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
