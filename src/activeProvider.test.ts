import { describe, expect, it } from "vitest";
import { detectActiveState, presentActiveBadge, withoutEmptyValues } from "./activeProvider";

const provider = (id: string, env: Record<string, string>) => ({ id, env });

describe("withoutEmptyValues", () => {
  it("removes keys whose value is an empty string", () => {
    expect(withoutEmptyValues({ A: "1", B: "", C: "2" })).toEqual({ A: "1", C: "2" });
  });
});

describe("detectActiveState", () => {
  it("marks the provider whose filtered env matches exactly", () => {
    const providers = [
      provider("a", { ANTHROPIC_BASE_URL: "https://a.test", ANTHROPIC_AUTH_TOKEN: "k1", ANTHROPIC_MODEL: "" }),
      provider("b", { ANTHROPIC_BASE_URL: "https://b.test", ANTHROPIC_AUTH_TOKEN: "k2" }),
    ];
    expect(detectActiveState({ ANTHROPIC_BASE_URL: "https://a.test", ANTHROPIC_AUTH_TOKEN: "k1" }, providers))
      .toEqual({ kind: "active", providerId: "a" });
  });

  it("marks a provider stale when only base url and token match", () => {
    const providers = [provider("a", { ANTHROPIC_BASE_URL: "https://a.test", ANTHROPIC_AUTH_TOKEN: "k1", ANTHROPIC_MODEL: "new" })];
    expect(detectActiveState({ ANTHROPIC_BASE_URL: "https://a.test", ANTHROPIC_AUTH_TOKEN: "k1", ANTHROPIC_MODEL: "old" }, providers))
      .toEqual({ kind: "stale", providerId: "a" });
  });

  it("returns unknown when nothing matches", () => {
    const providers = [provider("a", { ANTHROPIC_BASE_URL: "https://a.test", ANTHROPIC_AUTH_TOKEN: "k1" })];
    expect(detectActiveState({ ANTHROPIC_BASE_URL: "https://other.test", ANTHROPIC_AUTH_TOKEN: "k9" }, providers))
      .toEqual({ kind: "unknown" });
  });

  it("returns unknown when there are no providers", () => {
    expect(detectActiveState({ ANTHROPIC_BASE_URL: "https://a.test" }, [])).toEqual({ kind: "unknown" });
  });

  it("returns unreadable when the active env could not be read", () => {
    expect(detectActiveState(null, [provider("a", {})])).toEqual({ kind: "unreadable" });
  });

  it("matches a provider whose provider fields are all empty against an env without them", () => {
    const providers = [provider("native", { ANTHROPIC_BASE_URL: "", ANTHROPIC_AUTH_TOKEN: "", CLAUDE_CODE_EFFORT_LEVEL: "max" })];
    expect(detectActiveState({ CLAUDE_CODE_EFFORT_LEVEL: "max" }, providers))
      .toEqual({ kind: "active", providerId: "native" });
  });
});

describe("presentActiveBadge", () => {
  it("labels the active provider", () => {
    expect(presentActiveBadge({ kind: "active", providerId: "a" }, "a")).toBe("已生效");
  });

  it("labels a stale provider", () => {
    expect(presentActiveBadge({ kind: "stale", providerId: "a" }, "a")).toBe("已改动未生效");
  });

  it("gives no badge to other providers", () => {
    expect(presentActiveBadge({ kind: "active", providerId: "a" }, "b")).toBe("");
  });

  it("gives no badge when the state is unreadable", () => {
    expect(presentActiveBadge({ kind: "unreadable" }, "a")).toBe("");
  });
});
