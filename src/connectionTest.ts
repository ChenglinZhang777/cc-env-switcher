export type ConnectionResultKind = "success" | "authentication" | "request" | "unavailable" | "network";

export function missingConnectionFields(env: Record<string, string>): string[] {
  return [
    ["ANTHROPIC_BASE_URL", "API 地址"],
    ["ANTHROPIC_AUTH_TOKEN", "API Key"],
    ["ANTHROPIC_MODEL", "主模型"],
  ].filter(([key]) => !env[key]?.trim()).map(([, label]) => label);
}

export function presentConnectionResult(kind: ConnectionResultKind): string {
  return {
    success: "连接成功，可使用此模型。",
    authentication: "连接失败：请检查 API Key。",
    request: "连接失败：请检查 API 地址或模型名称。",
    unavailable: "连接失败：服务暂时不可用，请稍后重试。",
    network: "连接失败：请检查网络与 API 地址。",
  }[kind];
}
