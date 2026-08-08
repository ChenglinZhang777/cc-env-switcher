export type ActiveState =
  | { kind: "active"; providerId: string }
  | { kind: "stale"; providerId: string }
  | { kind: "unknown" }
  | { kind: "unreadable" };

type ProviderLike = { id: string; env: Record<string, string> };

/** 与写入 Claude 配置时相同的过滤规则：空字符串值的键视为未设置。 */
export function withoutEmptyValues(env: Record<string, string>): Record<string, string> {
  return Object.fromEntries(Object.entries(env).filter(([, value]) => value !== ""));
}

const sameEnv = (left: Record<string, string>, right: Record<string, string>) => {
  const leftKeys = Object.keys(left).sort();
  const rightKeys = Object.keys(right).sort();
  return leftKeys.length === rightKeys.length && leftKeys.every((key, index) => key === rightKeys[index] && left[key] === right[key]);
};

const sameCredentials = (left: Record<string, string>, right: Record<string, string>) =>
  Boolean(left.ANTHROPIC_BASE_URL) &&
  left.ANTHROPIC_BASE_URL === right.ANTHROPIC_BASE_URL &&
  left.ANTHROPIC_AUTH_TOKEN === right.ANTHROPIC_AUTH_TOKEN;

export function detectActiveState(activeEnv: Record<string, string> | null, providers: ProviderLike[]): ActiveState {
  if (!activeEnv) return { kind: "unreadable" };
  const active = withoutEmptyValues(activeEnv);
  const exact = providers.find(item => sameEnv(withoutEmptyValues(item.env), active));
  if (exact) return { kind: "active", providerId: exact.id };
  const stale = providers.find(item => sameCredentials(withoutEmptyValues(item.env), active));
  if (stale) return { kind: "stale", providerId: stale.id };
  return { kind: "unknown" };
}

export function presentActiveBadge(state: ActiveState, providerId: string): string {
  if (state.kind === "active" && state.providerId === providerId) return "已生效";
  if (state.kind === "stale" && state.providerId === providerId) return "已改动未生效";
  return "";
}
