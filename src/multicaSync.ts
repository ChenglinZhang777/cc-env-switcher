export type SyncState = "unsynced" | "synced" | "syncing";

/** 方案名转命令名，必须与 Rust 侧 multica_sync::command_name 一致。 */
export function commandName(profileName: string): string {
  const slug = profileName
    .split("")
    .map(ch => (/[a-zA-Z0-9]/.test(ch) ? ch.toLowerCase() : "-"))
    .join("")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "");
  return `cc-${slug}`;
}

export function syncStateOf(profileName: string, registered: string[], syncing: boolean): SyncState {
  if (syncing) return "syncing";
  return registered.includes(commandName(profileName)) ? "synced" : "unsynced";
}

export function presentSyncState(state: SyncState): string {
  return { unsynced: "未接线", synced: "已接线", syncing: "同步中…" }[state];
}

/** 只有本机有 API 地址的方案才需要接线；官方方案走 multica 内置运行时。 */
export function needsSync(env: Record<string, string>): boolean {
  return Boolean(env.ANTHROPIC_BASE_URL?.trim());
}

export function presentSyncFailure(message: string): string {
  const text = message.trim();
  if (!text) return "同步失败：没有拿到失败原因。";
  if (text.includes("找不到 multica")) return "同步失败：这台机器没装 multica CLI。";
  if (text.includes("体检未通过")) return text;
  return `同步失败：${text}`;
}
