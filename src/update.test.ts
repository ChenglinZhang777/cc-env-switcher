import { describe, expect, it } from "vitest";
import { presentUpdateResult } from "./update";

describe("presentUpdateResult", () => {
  it("describes an available signed release", () => {
    expect(presentUpdateResult({ kind: "available", version: "0.2.0", notes: "修复切换体验" }))
      .toEqual({ title: "发现新版本 0.2.0", detail: "修复切换体验", canInstall: true });
  });

  it("does not expose transport errors", () => {
    expect(presentUpdateResult({ kind: "failed" }).detail).toBe("不影响当前使用，可稍后重试。");
  });
});
