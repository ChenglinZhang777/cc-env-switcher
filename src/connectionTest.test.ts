import { describe, expect, it } from "vitest";
import { missingConnectionFields, presentConnectionResult } from "./connectionTest";

describe("connection test presentation", () => {
  it("lists only missing required values", () => {
    expect(missingConnectionFields({ ANTHROPIC_BASE_URL: " ", ANTHROPIC_AUTH_TOKEN: "token", ANTHROPIC_MODEL: "" }))
      .toEqual(["API 地址", "主模型"]);
  });

  it("does not expose server details", () => {
    expect(presentConnectionResult("authentication")).toBe("连接失败：请检查 API Key。");
  });
});
