import { describe, expect, it } from "vitest";
import { commandName, needsSync, presentSyncFailure, presentSyncState, syncStateOf } from "./multicaSync";

describe("multica 接线", () => {
  it("命令名与 Rust 侧的 slug 规则保持一致", () => {
    expect(commandName("DS-v4-flash")).toBe("cc-ds-v4-flash");
    expect(commandName("Jiaming GPT")).toBe("cc-jiaming-gpt");
    expect(commandName("  官方 Claude  ")).toBe("cc-claude");
  });

  it("按已注册命令名判断接线状态", () => {
    const registered = ["cc-ds-v4-flash"];
    expect(syncStateOf("DS-v4-flash", registered, false)).toBe("synced");
    expect(syncStateOf("Jiaming-GPT", registered, false)).toBe("unsynced");
    expect(syncStateOf("DS-v4-flash", registered, true)).toBe("syncing");
    expect(presentSyncState("unsynced")).toBe("未接线");
  });

  it("只有配了 API 地址的方案才需要接线", () => {
    expect(needsSync({ ANTHROPIC_BASE_URL: "https://api.example.com" })).toBe(true);
    expect(needsSync({ ANTHROPIC_BASE_URL: "   " })).toBe(false);
    expect(needsSync({})).toBe(false);
  });

  it("把失败原因翻译成人能看懂的话", () => {
    expect(presentSyncFailure("找不到 multica 命令；请先安装 multica CLI。")).toContain("没装 multica CLI");
    expect(presentSyncFailure("体检未通过：跑不出任何回复。")).toContain("体检未通过");
    expect(presentSyncFailure("")).toContain("没有拿到失败原因");
    expect(presentSyncFailure("boom")).toBe("同步失败：boom");
  });
});
