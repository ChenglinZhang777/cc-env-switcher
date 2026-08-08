# Active State and Operation Feedback Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让用户能确认切换操作是否成功、当前真正生效的是哪个方案，并有一个预置的「官方 Claude」方案作为回退入口。

**Architecture:** 后端在写入 Claude 配置前过滤空值环境变量，并新增一个读取当前生效环境变量的命令；前端用一个纯函数比对生效环境变量与已存方案，得出生效状态；操作反馈改为按钮就地变化加右上角自动消失的消息。

**Tech Stack:** Rust（Tauri 2）、TypeScript、React 19、Vitest 3、cargo test

## Global Constraints

- 所有面向用户的文案使用中文。
- 供应商字段共 8 个键：`ANTHROPIC_BASE_URL`、`ANTHROPIC_AUTH_TOKEN`、`ANTHROPIC_MODEL`、`ANTHROPIC_DEFAULT_OPUS_MODEL`、`ANTHROPIC_DEFAULT_SONNET_MODEL`、`ANTHROPIC_DEFAULT_HAIKU_MODEL`、`ANTHROPIC_DEFAULT_FABLE_MODEL`、`CLAUDE_CODE_SUBAGENT_MODEL`。
- 预置方案名称为「官方 Claude」。
- 空值过滤规则：值为空字符串的键不写入 Claude 配置；`providers.json` 保存的数据形状不变。
- 不修改 `switch_env` 的备份、原子写入、回读校验逻辑。
- 前端测试命令：`npm test`。Rust 测试命令：在 `src-tauri/` 目录下 `cargo test --lib`。
- 现有测试基线（2026-08-08 实测）：前端 3 个文件 6 个测试全通过；Rust lib 4 个测试全通过（`claude_settings` 1、`providers` 1、`connection_test` 2）。任何任务结束时这些必须仍然通过。
- 若 `cargo test` 报 `failed to read plugin permissions` 并指向 `/Users/xiaogouzi/workspace/app/claude-env-switcher`（旧目录名），这是目录改名遗留的陈旧构建产物，不是代码问题。在 `src-tauri/` 下执行 `cargo clean -p tauri -p cc-env-switcher` 后重试。

---

## File Structure

| 文件 | 职责 |
|---|---|
| `src-tauri/src/claude_settings.rs`（改） | 写入 Claude 配置前过滤空值；备份与原子写入逻辑不动 |
| `src-tauri/src/main.rs`（改） | 新增读取生效环境变量的命令；`list_providers` 在列表为空时预置方案 |
| `src/activeProvider.ts`（新） | 生效判定纯函数，不依赖 Tauri API |
| `src/activeProvider.test.ts`（新） | 生效判定的单元测试 |
| `src/feedback.ts`（新） | 反馈消息的纯逻辑（文案与是否自动消失） |
| `src/feedback.test.ts`（新） | 反馈逻辑的单元测试 |
| `src/main.tsx`（改） | 接入生效状态显示与操作反馈 |
| `src/styles.css`（改） | 生效徽标与右上角消息的样式 |

---

### Task 1: 写入 Claude 配置时过滤空值

**Files:**
- Modify: `src-tauri/src/claude_settings.rs:19-47`
- Test: `src-tauri/src/claude_settings.rs`（同文件 `#[cfg(test)] mod tests`）

**Interfaces:**
- Consumes: 无
- Produces: `switch_env(settings_path: &Path, backups_dir: &Path, env: &Value) -> Result<(), SwitchError>` 签名不变，但写入行为改为跳过空字符串值。Task 3 的判定函数必须与此规则一致。

- [ ] **Step 1: 写失败的测试**

在 `src-tauri/src/claude_settings.rs` 的 `mod tests` 内，`switch_env_creates_backup_and_preserves_other_fields` 之后追加：

```rust
    #[test]
    fn switch_env_skips_empty_values_and_keeps_other_sections() {
        let root = test_root();
        let settings = root.join("settings.json");
        let backups = root.join("backups");
        fs::write(&settings, r#"{"env":{"OLD":"1"},"permissions":{"allow":["Bash"]}}"#).unwrap();
        switch_env(&settings, &backups, &serde_json::json!({
            "ANTHROPIC_BASE_URL": "",
            "ANTHROPIC_AUTH_TOKEN": "",
            "CLAUDE_CODE_EFFORT_LEVEL": "max"
        })).unwrap();
        let updated: serde_json::Value = serde_json::from_slice(&fs::read(&settings).unwrap()).unwrap();
        let env = updated["env"].as_object().unwrap();
        assert!(!env.contains_key("ANTHROPIC_BASE_URL"), "空值键不应写入");
        assert!(!env.contains_key("ANTHROPIC_AUTH_TOKEN"), "空值键不应写入");
        assert_eq!(env["CLAUDE_CODE_EFFORT_LEVEL"], "max");
        assert_eq!(env.len(), 1);
        assert_eq!(updated["permissions"]["allow"][0], "Bash");
        assert_eq!(fs::read_dir(backups).unwrap().count(), 1);
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cd src-tauri && cargo test --lib claude_settings::tests::switch_env_skips_empty_values`
Expected: FAIL，断言 `空值键不应写入` 失败（当前实现原样写入空字符串）

- [ ] **Step 3: 实现过滤**

在 `src-tauri/src/claude_settings.rs` 中，把 `switch_env` 里的这一行：

```rust
    object.insert("env".into(), env.clone());
```

替换为：

```rust
    let filtered: serde_json::Map<String, Value> = env
        .as_object()
        .ok_or(SwitchError::InvalidJson)?
        .iter()
        .filter(|(_, value)| value.as_str() != Some(""))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    let filtered = Value::Object(filtered);
    object.insert("env".into(), filtered.clone());
```

同时把函数末尾的回读校验从比对 `env` 改为比对 `filtered`：

```rust
    if verified.get("env") != Some(&filtered) {
        return Err(SwitchError::VerificationFailed);
    }
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cd src-tauri && cargo test --lib`
Expected: PASS，5 个测试全通过（原有 4 个 + 新增 1 个）。原有 `switch_env_creates_backup_and_preserves_other_fields` 必须仍然通过，因为它传的值非空。

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/claude_settings.rs
git commit -m "feat: skip empty env values when writing Claude settings"
```

---

### Task 2: 读取当前生效环境变量的命令

**Files:**
- Modify: `src-tauri/src/main.rs:69-74`（在 `import_current_env` 之后新增命令）
- Modify: `src-tauri/src/main.rs:114`（注册到 `invoke_handler`）

**Interfaces:**
- Consumes: `AppState.settings_path`（已存在，`src-tauri/src/main.rs:6`）
- Produces: Tauri 命令 `read_active_env`，返回 `Result<Option<BTreeMap<String, String>>, String>`。`Some(map)` 表示读到了生效环境变量；`None` 表示配置文件缺失或 JSON 损坏。前端据此决定是否显示生效状态。

- [ ] **Step 1: 实现命令**

在 `src-tauri/src/main.rs` 的 `import_current_env` 函数之后插入：

```rust
/// 读取当前生效的环境变量。配置文件缺失或损坏时返回 None，让界面照常渲染。
#[tauri::command]
fn read_active_env(state: tauri::State<AppState>) -> Result<Option<BTreeMap<String, String>>, String> {
    let Ok(bytes) = std::fs::read(&state.settings_path) else { return Ok(None) };
    let Ok(document) = serde_json::from_slice::<serde_json::Value>(&bytes) else { return Ok(None) };
    let Some(env) = document.get("env") else { return Ok(Some(BTreeMap::new())) };
    Ok(serde_json::from_value(env.clone()).ok())
}
```

- [ ] **Step 2: 注册命令**

在 `src-tauri/src/main.rs` 的 `invoke_handler` 中，把 `import_current_env` 后面加上 `read_active_env`：

```rust
        .invoke_handler(tauri::generate_handler![list_providers, save_provider, delete_provider, import_current_env, read_active_env, switch_provider, test_connection, backups_path, open_backups_directory])
```

- [ ] **Step 3: 确认编译通过**

Run: `cd src-tauri && cargo build --lib && cargo test --lib`
Expected: 编译无警告无错误；测试仍是 3 个全通过。

注意：`read_active_env` 依赖 Tauri 运行时状态，无法用 `cargo test --lib` 覆盖（`lib.rs` 不包含 `main.rs`）。它的行为由 Task 7 的手动验收覆盖。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/main.rs
git commit -m "feat: add command to read active Claude env"
```

---

### Task 3: 生效判定纯函数

**Files:**
- Create: `src/activeProvider.ts`
- Test: `src/activeProvider.test.ts`

**Interfaces:**
- Consumes: Task 1 确立的空值过滤规则（值为空字符串的键不参与比对）
- Produces:
  - `type ActiveState = { kind: "active"; providerId: string } | { kind: "stale"; providerId: string } | { kind: "unknown" } | { kind: "unreadable" }`
  - `withoutEmptyValues(env: Record<string, string>): Record<string, string>`
  - `detectActiveState(activeEnv: Record<string, string> | null, providers: { id: string; env: Record<string, string> }[]): ActiveState`
  - `presentActiveBadge(state: ActiveState, providerId: string): string`（返回该方案应显示的徽标文案，无徽标时返回空字符串）

- [ ] **Step 1: 写失败的测试**

创建 `src/activeProvider.test.ts`：

```typescript
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
```

- [ ] **Step 2: 跑测试确认失败**

Run: `npm test -- src/activeProvider.test.ts`
Expected: FAIL，报找不到模块 `./activeProvider`

- [ ] **Step 3: 实现**

创建 `src/activeProvider.ts`：

```typescript
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
```

- [ ] **Step 4: 跑测试确认通过**

Run: `npm test`
Expected: PASS，4 个测试文件共 17 个测试全通过（原有 6 个 + 新增 11 个）

- [ ] **Step 5: 提交**

```bash
git add src/activeProvider.ts src/activeProvider.test.ts
git commit -m "feat: add active provider state detection"
```

---

### Task 4: 反馈消息纯逻辑

**Files:**
- Create: `src/feedback.ts`
- Test: `src/feedback.test.ts`

**Interfaces:**
- Consumes: 无
- Produces:
  - `type Feedback = { text: string; tone: "success" | "error"; sticky: boolean }`
  - `successFeedback(text: string): Feedback`
  - `errorFeedback(text: string): Feedback`
  - `FEEDBACK_DISMISS_MS`（数值常量 2000，Task 5 用它设定定时器）

- [ ] **Step 1: 写失败的测试**

创建 `src/feedback.test.ts`：

```typescript
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
```

- [ ] **Step 2: 跑测试确认失败**

Run: `npm test -- src/feedback.test.ts`
Expected: FAIL，报找不到模块 `./feedback`

- [ ] **Step 3: 实现**

创建 `src/feedback.ts`：

```typescript
export type Feedback = { text: string; tone: "success" | "error"; sticky: boolean };

/** 成功消息 2 秒后自动消失。 */
export const FEEDBACK_DISMISS_MS = 2000;

export const successFeedback = (text: string): Feedback => ({ text, tone: "success", sticky: false });

/** 错误消息不自动消失，避免用户没看到就溜走。 */
export const errorFeedback = (text: string): Feedback => ({ text, tone: "error", sticky: true });
```

- [ ] **Step 4: 跑测试确认通过**

Run: `npm test`
Expected: PASS，5 个测试文件共 20 个测试全通过

- [ ] **Step 5: 提交**

```bash
git add src/feedback.ts src/feedback.test.ts
git commit -m "feat: add operation feedback presentation logic"
```

---

### Task 5: 界面接入生效状态与操作反馈

**Files:**
- Modify: `src/main.tsx`
- Modify: `src/styles.css`

**Interfaces:**
- Consumes: Task 2 的 `read_active_env` 命令；Task 3 的 `detectActiveState`、`presentActiveBadge`、`ActiveState`；Task 4 的 `successFeedback`、`errorFeedback`、`FEEDBACK_DISMISS_MS`、`Feedback`
- Produces: 无（终端界面）

- [ ] **Step 1: 替换 import 与状态声明**

在 `src/main.tsx` 中，把 `import { checkForUpdate, ... }` 那一行之后追加两行 import：

```typescript
import { detectActiveState, presentActiveBadge, type ActiveState } from "./activeProvider";
import { errorFeedback, FEEDBACK_DISMISS_MS, successFeedback, type Feedback } from "./feedback";
```

把 `const [message, setMessage] = useState("");` 这一行替换为：

```typescript
  const [feedback, setFeedback] = useState<Feedback | null>(null);
  const [busyAction, setBusyAction] = useState<"" | "saved" | "switched">("");
  const [activeState, setActiveState] = useState<ActiveState>({ kind: "unreadable" });
```

- [ ] **Step 2: 加入反馈与判定的辅助函数**

在 `const load = async () => {` 之前插入：

```typescript
  const notify = (next: Feedback) => {
    setFeedback(next);
    if (!next.sticky) setTimeout(() => setFeedback(current => (current === next ? null : current)), FEEDBACK_DISMISS_MS);
  };
  const confirmAction = (action: "saved" | "switched", text: string) => {
    setBusyAction(action);
    notify(successFeedback(text));
    setTimeout(() => setBusyAction(current => (current === action ? "" : current)), FEEDBACK_DISMISS_MS);
  };
  const refreshActiveState = async (profiles: Provider[]) => {
    const activeEnv = await invoke<Record<string, string> | null>("read_active_env").catch(() => null);
    setActiveState(detectActiveState(activeEnv, profiles));
  };
```

- [ ] **Step 3: 让 load 顺带刷新判定**

把 `load` 函数体替换为：

```typescript
  const load = async () => {
    const profiles = await invoke<Provider[]>("list_providers");
    setProviders(profiles);
    setSelected(current => current ? profiles.find(item => item.id === current.id) ?? current : profiles[0] ?? null);
    await refreshActiveState(profiles);
  };
```

- [ ] **Step 4: 改写各操作的反馈**

把 `save`、`switchTo`、`remove`、`importCurrent` 四个函数替换为：

```typescript
  const save = async () => {
    if (!selected?.name.trim()) return notify(errorFeedback("请先填写供应商名称。"));
    try { await invoke("save_provider", { profile: selected }); await load(); confirmAction("saved", "方案已保存。密钥以明文保存在本机应用配置中。"); }
    catch (error) { notify(errorFeedback(`保存失败：${String(error)}`)); }
  };
  const switchTo = async (provider: Provider) => {
    try { await invoke("switch_provider", { id: provider.id }); await load(); confirmAction("switched", `已切换到 ${provider.name}；原 settings.json 已完整备份。`); }
    catch (error) { notify(errorFeedback(`切换失败：${String(error)}`)); }
  };
  const remove = async () => {
    if (!selected || !confirm(`删除“${selected.name}”方案？不会删除 Claude 配置或任何备份。`)) return;
    try { await invoke("delete_provider", { id: selected.id }); setSelected(null); await load(); notify(successFeedback("方案已删除。")); }
    catch (error) { notify(errorFeedback(`删除失败：${String(error)}`)); }
  };
  const importCurrent = async () => {
    try {
      const env = await invoke<Record<string, string>>("import_current_env");
      setSelected({ id: crypto.randomUUID(), name: "从当前配置导入", env: { ...defaultProviderEnv, ...env } });
      notify(successFeedback("已导入当前 env；请命名后保存。"));
    } catch (error) { notify(errorFeedback(`导入失败：${String(error)}`)); }
  };
```

同时把 `useEffect` 里的 `void load().catch(error => setMessage(String(error)));` 改为：

```typescript
  useEffect(() => { void load().catch(error => notify(errorFeedback(String(error)))); void checkUpdates(); }, []);
```

- [ ] **Step 5: 侧边栏显示徽标与未知提示**

把侧边栏那一行（`{providers.length ? providers.map(...)}`）替换为：

```typescript
        {activeState.kind === "unknown" && <div className="active-unknown">当前生效的配置不属于任何已存方案。</div>}
        {providers.length ? providers.map(item => { const badge = presentActiveBadge(activeState, item.id); return <button className={`profile-card ${selected?.id === item.id ? "active" : ""}`} key={item.id} onClick={() => setSelected(item)}><span className="profile-name">{item.name}{badge && <em className={`badge ${badge === "已生效" ? "badge-active" : "badge-stale"}`}>{badge}</em>}</span><span>{host(item.env.ANTHROPIC_BASE_URL)}</span><span className="profile-model">{item.env.ANTHROPIC_MODEL || "未设置模型"}</span></button>; }) : <div className="empty-list">还没有方案<br />从当前 Claude 配置导入，或创建新的方案。</div>}
```

- [ ] **Step 6: 按钮就地反馈与右上角消息**

把切换按钮那一行（`<button className="switch-button" ...>`）替换为：

```typescript
<button className="switch-button" disabled={busyAction === "switched"} onClick={() => void switchTo(selected)}>{busyAction === "switched" ? "✓ 已切换" : "切换到此方案 →"}</button>
```

把编辑器底部的保存按钮替换为：

```typescript
<button className="primary" disabled={busyAction === "saved"} onClick={() => void save()}>{busyAction === "saved" ? "✓ 已保存" : "保存方案"}</button>
```

把 `main` 结尾的两行 `{updateMessage && ...}` 与 `{message && ...}` 替换为：

```typescript
    {updateMessage && <output className="notice">{updateMessage}</output>}
    {feedback && <output className={`toast toast-${feedback.tone}`}>{feedback.text}{feedback.sticky && <button onClick={() => setFeedback(null)}>关闭</button>}</output>}
```

- [ ] **Step 7: 加样式**

在 `src/styles.css` 末尾追加：

```css
.badge { margin-left:7px; padding:1px 6px; border-radius:99px; font-size:10px; font-style:normal; font-weight:800; vertical-align:middle; }
.badge-active { background:#dcfce7; color:#15803d; }
.badge-stale { background:#fef3c7; color:#a16207; }
.active-unknown { margin:0 4px 10px; padding:9px 10px; border-radius:9px; background:#fef3c7; color:#8a5a05; font-size:12px; line-height:1.5; }
.toast { position:fixed; top:22px; right:22px; z-index:20; display:flex; align-items:center; gap:12px; padding:12px 16px; border-radius:10px; font-size:13px; font-weight:700; box-shadow:0 10px 30px #1e293b26; }
.toast-success { background:#dcfce7; color:#15803d; }
.toast-error { background:#fee2e2; color:#b3261e; }
.toast button { background:transparent; color:inherit; font-size:12px; font-weight:800; text-decoration:underline; padding:0; }
.switch-button:disabled, .primary:disabled { cursor:default; opacity:.9; }
```

- [ ] **Step 8: 确认类型检查与测试通过**

Run: `npm run build && npm test`
Expected: `tsc -b` 无错误，vite 构建成功；测试仍是 20 个全通过。

若 `tsc` 报 `message`/`setMessage` 未定义，说明 Step 4 有遗漏的引用点，搜索 `setMessage` 全部替换为 `notify(...)` 形式。

- [ ] **Step 9: 提交**

```bash
git add src/main.tsx src/styles.css
git commit -m "feat: show active provider state and inline operation feedback"
```

---

### Task 6: 首次启动预置「官方 Claude」方案

**Files:**
- Modify: `src-tauri/src/providers.rs`
- Modify: `src-tauri/src/main.rs:52-53`（`list_providers`）
- Test: `src-tauri/src/providers.rs`（同文件 `mod tests`）

**Interfaces:**
- Consumes: `ProviderProfile`（`src-tauri/src/providers.rs:8`）、`save_profiles`
- Produces: `pub fn native_profile() -> ProviderProfile`，名称为「官方 Claude」，供应商字段为空字符串，行为偏好与前端 `defaultProviderEnv` 一致

- [ ] **Step 1: 写失败的测试**

在 `src-tauri/src/providers.rs` 的 `mod tests` 内追加：

```rust
    #[test]
    fn native_profile_leaves_provider_fields_empty() {
        let profile = native_profile();
        assert_eq!(profile.name, "官方 Claude");
        for key in ["ANTHROPIC_BASE_URL", "ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_MODEL", "CLAUDE_CODE_SUBAGENT_MODEL"] {
            assert_eq!(profile.env.get(key).map(String::as_str), Some(""), "{key} 应为空");
        }
        assert_eq!(profile.env.get("CLAUDE_CODE_EFFORT_LEVEL").map(String::as_str), Some("max"));
        assert!(!profile.id.is_empty());
    }

    #[test]
    fn native_profiles_get_distinct_ids() {
        assert_ne!(native_profile().id, native_profile().id);
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cd src-tauri && cargo test --lib providers::tests::native_profile`
Expected: FAIL，`cannot find function native_profile in this scope`

- [ ] **Step 3: 实现**

在 `src-tauri/src/providers.rs` 的 `save_profiles` 之后插入：

```rust
/// 预置方案：供应商字段留空，切换后这些键不会写入 Claude 配置，等于回到官方默认。
pub fn native_profile() -> ProviderProfile {
    let empty = ["ANTHROPIC_BASE_URL", "ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_MODEL",
        "ANTHROPIC_DEFAULT_OPUS_MODEL", "ANTHROPIC_DEFAULT_SONNET_MODEL",
        "ANTHROPIC_DEFAULT_HAIKU_MODEL", "ANTHROPIC_DEFAULT_FABLE_MODEL",
        "CLAUDE_CODE_SUBAGENT_MODEL"];
    let mut env: BTreeMap<String, String> = empty.iter().map(|key| (key.to_string(), String::new())).collect();
    env.insert("CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS".into(), "1".into());
    env.insert("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC".into(), "1".into());
    env.insert("CLAUDE_CODE_ATTRIBUTION_HEADER".into(), "0".into());
    env.insert("CLAUDE_CODE_EFFORT_LEVEL".into(), "max".into());
    ProviderProfile { id: new_id(), name: "官方 Claude".into(), env }
}

/// 生成方案 ID。不引入 uuid 依赖，用时间戳加计数器保证同一进程内不重复。
fn new_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|value| value.as_nanos()).unwrap_or(0);
    format!("{nanos:x}-{:x}", COUNTER.fetch_add(1, Ordering::Relaxed))
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cd src-tauri && cargo test --lib`
Expected: PASS，7 个测试全通过（Task 1 后的 5 个 + 本任务新增 2 个）

- [ ] **Step 5: 在列表为空时预置**

把 `src-tauri/src/main.rs` 的 `list_providers` 替换为：

```rust
#[tauri::command]
fn list_providers(state: tauri::State<AppState>) -> Result<Vec<ProviderProfile>, String> {
    let profiles = providers::load_profiles(&state.providers_path)?;
    if !profiles.is_empty() { return Ok(profiles); }
    // 列表为空时预置一个官方方案；用户删除后不会被反复塞回，除非再次归零。
    let seeded = vec![providers::native_profile()];
    providers::save_profiles(&state.providers_path, &seeded)?;
    Ok(seeded)
}
```

- [ ] **Step 6: 确认编译与测试通过**

Run: `cd src-tauri && cargo build --lib && cargo test --lib`
Expected: 编译无错误；7 个测试全通过。

- [ ] **Step 7: 提交**

```bash
git add src-tauri/src/providers.rs src-tauri/src/main.rs
git commit -m "feat: seed a native Claude profile when the list is empty"
```

---

### Task 7: 端到端手动验收

**Files:**
- 无代码改动

**Interfaces:**
- Consumes: Task 1-6 的全部产出
- Produces: 无

- [ ] **Step 1: 备份真实配置**

```bash
cp ~/.claude/settings.json ~/.claude/settings.json.manual-backup-$(date +%s)
ls -la ~/.claude/settings.json.manual-backup-*
```

这一步是为了在验收出问题时能自己恢复，不依赖应用的备份机制。

- [ ] **Step 2: 启动应用**

```bash
npm run tauri dev
```

- [ ] **Step 3: 逐条核对验收标准**

按 spec 的 Acceptance Criteria 逐条实测，每条记录实际观察结果：

1. 方案列表为空时启动出现「官方 Claude」，且可编辑、可重命名、可删除。
2. 切换到供应商字段留空的方案后，`~/.claude/settings.json` 中不含那 8 个键，其余键与非 `env` 段原样保留，且备份目录新增一个文件。
3. 切换到某方案后侧边栏显示「已生效」。
4. 修改已生效方案的主模型并保存但不切换，显示「已改动未生效」。
5. 手动编辑 `~/.claude/settings.json` 使其不匹配任何方案后重启应用，侧边栏顶部出现相应提示。
6. 临时改名 `~/.claude/settings.json` 使其缺失，应用正常启动、方案列表可用、不显示生效状态；之后改回。
7. 保存与切换成功后按钮就地反馈并在 2 秒后复原；构造失败（例如把方案名清空后保存）时消息为红色且不自动消失。

第 2 条用这个命令核对：

```bash
python3 -c "import json;env=json.load(open('$HOME/.claude/settings.json'))['env'];print('残留供应商键:',[k for k in env if k.startswith('ANTHROPIC_') or k=='CLAUDE_CODE_SUBAGENT_MODEL'])"
```

Expected: `残留供应商键: []`

- [ ] **Step 4: 确认全部测试仍通过**

Run: `npm test && cd src-tauri && cargo test --lib`
Expected: 前端 20 个测试通过；Rust 7 个测试通过。

- [ ] **Step 5: 提交验收记录**

若前面任务有需要修补的地方，在此提交修复。无修复则跳过提交。

