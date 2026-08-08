import { describe, expect, it } from "vitest";
import { defaultProviderEnv } from "./providerTemplate";

describe("defaultProviderEnv", () => {
  it("includes every Claude-compatible provider field", () => {
    expect(Object.keys(defaultProviderEnv)).toEqual([
      "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS",
      "ANTHROPIC_BASE_URL",
      "ANTHROPIC_AUTH_TOKEN",
      "ANTHROPIC_MODEL",
      "ANTHROPIC_DEFAULT_OPUS_MODEL",
      "ANTHROPIC_DEFAULT_SONNET_MODEL",
      "ANTHROPIC_DEFAULT_HAIKU_MODEL",
      "ANTHROPIC_DEFAULT_FABLE_MODEL",
      "CLAUDE_CODE_SUBAGENT_MODEL",
      "CLAUDE_CODE_EFFORT_LEVEL",
    ]);
  });
});
