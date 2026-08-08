import { describe, expect, it } from "vitest";
import { errorFeedback, FEEDBACK_DISMISS_MS, successFeedback } from "./feedback";

describe("feedback", () => {
  it("makes success messages auto-dismiss", () => {
    expect(successFeedback("方案已保存")).toEqual({ text: "方案已保存", tone: "success", sticky: false });
  });

  it("keeps error messages until dismissed", () => {
    expect(errorFeedback("保存失败：磁盘只读")).toEqual({ text: "保存失败：磁盘只读", tone: "error", sticky: true });
  });

  it("dismisses success messages after two seconds", () => {
    expect(FEEDBACK_DISMISS_MS).toBe(2000);
  });
});
