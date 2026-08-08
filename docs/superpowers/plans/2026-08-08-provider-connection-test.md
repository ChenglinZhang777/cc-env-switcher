# Provider Connection Test Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 增加两项 Claude Code 环境变量，并允许用户对正在编辑的供应商方案进行安全的最小 API 连接测试。

**Architecture:** 前端只校验表单并展示状态；Tauri Rust command 发出 Anthropic Messages 兼容请求。独立 Rust 模块负责 URL、请求体和状态分类，Token 不进入日志、持久化或错误文本。

**Tech Stack:** React 19、TypeScript、Vitest、Tauri 2、Rust、reqwest、serde_json。

---

### Task 1: 供应商默认值与前端结果模块

**Files:**
- Modify: `src/providerTemplate.ts`
- Modify: `src/providerTemplate.test.ts`
- Create: `src/connectionTest.ts`
- Create: `src/connectionTest.test.ts`

- [ ] **Step 1: 写出失败的默认值与结果提示测试**

将 `src/providerTemplate.test.ts` 的 keys 期望值改为在 `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS` 后包含：

```ts
"CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC",
"CLAUDE_CODE_ATTRIBUTION_HEADER",
```

并增加：

```ts
expect(defaultProviderEnv.CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC).toBe("1");
expect(defaultProviderEnv.CLAUDE_CODE_ATTRIBUTION_HEADER).toBe("0");
```

创建 `src/connectionTest.test.ts`：

```ts
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
```

- [ ] **Step 2: 验证测试确实失败**

Run: `npm test -- src/providerTemplate.test.ts src/connectionTest.test.ts`

Expected: FAIL，因为两个默认值和 `connectionTest` 模块尚不存在。

- [ ] **Step 3: 实现默认值与纯函数**

在 `src/providerTemplate.ts` 中紧随现有 Agent Teams 默认值添加：

```ts
CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC: "1",
CLAUDE_CODE_ATTRIBUTION_HEADER: "0",
```

创建 `src/connectionTest.ts`：

```ts
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
```

- [ ] **Step 4: 验证并提交**

Run: `npm test -- src/providerTemplate.test.ts src/connectionTest.test.ts`

Expected: PASS。

```bash
git add src/providerTemplate.ts src/providerTemplate.test.ts src/connectionTest.ts src/connectionTest.test.ts
git commit -m "feat: add provider connection test defaults"
```

### Task 2: 原生最小请求命令

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/connection_test.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: 写出失败的 Rust 测试**

创建 `src-tauri/src/connection_test.rs`，先只放入：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_endpoint_and_builds_minimal_payload() {
        assert_eq!(endpoint("https://api.example.com/anthropic/").unwrap(), "https://api.example.com/anthropic/v1/messages");
        assert_eq!(payload("deepseek-v4-flash[1m]"), serde_json::json!({
            "model": "deepseek-v4-flash[1m]", "max_tokens": 1,
            "messages": [{"role": "user", "content": "ping"}],
        }));
    }

    #[test]
    fn classifies_http_status_without_body() {
        assert_eq!(classify_status(reqwest::StatusCode::UNAUTHORIZED), "authentication");
        assert_eq!(classify_status(reqwest::StatusCode::BAD_REQUEST), "request");
        assert_eq!(classify_status(reqwest::StatusCode::TOO_MANY_REQUESTS), "unavailable");
        assert_eq!(classify_status(reqwest::StatusCode::OK), "success");
    }
}
```

暂时在 `src-tauri/src/lib.rs` 增加 `pub mod connection_test;`。

- [ ] **Step 2: 验证 Rust 测试失败**

Run: `cargo test connection_test --manifest-path src-tauri/Cargo.toml`

Expected: FAIL，因为函数和 `reqwest` 依赖尚不存在。

- [ ] **Step 3: 加入依赖并实现原生模块**

在 `src-tauri/Cargo.toml` 的 `[dependencies]` 增加：

```toml
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
```

将 `src-tauri/src/connection_test.rs` 完整替换为：

```rust
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

#[derive(Deserialize)]
pub struct ConnectionTestInput { pub base_url: String, pub auth_token: String, pub model: String }

#[derive(Serialize)]
pub struct ConnectionTestResult { pub kind: String }

fn endpoint(base_url: &str) -> Result<String, String> {
    let base = base_url.trim().trim_end_matches('/');
    if base.is_empty() { return Err("缺少 API 地址".into()); }
    let parsed = reqwest::Url::parse(base).map_err(|_| "API 地址格式无效".to_string())?;
    if !matches!(parsed.scheme(), "https" | "http") { return Err("API 地址必须使用 HTTP 或 HTTPS".into()); }
    Ok(format!("{base}/v1/messages"))
}

fn payload(model: &str) -> Value {
    json!({"model": model, "max_tokens": 1, "messages": [{"role": "user", "content": "ping"}]})
}

fn classify_status(status: StatusCode) -> &'static str {
    if status.is_success() { "success" }
    else if matches!(status.as_u16(), 401 | 403) { "authentication" }
    else if matches!(status.as_u16(), 400 | 404) { "request" }
    else { "unavailable" }
}

pub async fn test(input: ConnectionTestInput) -> Result<ConnectionTestResult, String> {
    let url = endpoint(&input.base_url)?;
    if input.auth_token.trim().is_empty() { return Err("缺少 API Key".into()); }
    if input.model.trim().is_empty() { return Err("缺少主模型".into()); }
    let response = Client::builder().timeout(Duration::from_secs(20)).build()
        .map_err(|_| "无法建立测试连接".to_string())?
        .post(url).header("content-type", "application/json")
        .header("x-api-key", &input.auth_token)
        .header("authorization", format!("Bearer {}", input.auth_token))
        .header("anthropic-version", "2023-06-01")
        .json(&payload(&input.model)).send().await;
    match response {
        Ok(response) => Ok(ConnectionTestResult { kind: classify_status(response.status()).into() }),
        Err(_) => Ok(ConnectionTestResult { kind: "network".into() }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalizes_endpoint_and_builds_minimal_payload() {
        assert_eq!(endpoint("https://api.example.com/anthropic/").unwrap(), "https://api.example.com/anthropic/v1/messages");
        assert_eq!(payload("deepseek-v4-flash[1m]"), json!({"model": "deepseek-v4-flash[1m]", "max_tokens": 1, "messages": [{"role": "user", "content": "ping"}]}));
    }
    #[test]
    fn classifies_http_status_without_body() {
        assert_eq!(classify_status(StatusCode::UNAUTHORIZED), "authentication");
        assert_eq!(classify_status(StatusCode::BAD_REQUEST), "request");
        assert_eq!(classify_status(StatusCode::TOO_MANY_REQUESTS), "unavailable");
        assert_eq!(classify_status(StatusCode::OK), "success");
    }
}
```

在 `src-tauri/src/main.rs` 的 `backups_path` command 前加入：

```rust
#[tauri::command]
async fn test_connection(input: claude_env_switcher_lib::connection_test::ConnectionTestInput) -> Result<claude_env_switcher_lib::connection_test::ConnectionTestResult, String> {
    claude_env_switcher_lib::connection_test::test(input).await
}
```

并将 `test_connection` 加入既有 `tauri::generate_handler!` 参数列表。

- [ ] **Step 4: 验证并提交**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`

Expected: PASS，包括两个新测试。

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/connection_test.rs src-tauri/src/lib.rs src-tauri/src/main.rs
git commit -m "feat: add safe native connection test"
```

### Task 3: 编辑器控制、文档与构建验证

**Files:**
- Modify: `src/main.tsx`
- Modify: `src/styles.css`
- Modify: `README.md`

- [ ] **Step 1: 接入当前草稿而不保存**

在 `src/main.tsx` 导入：

```ts
import { missingConnectionFields, presentConnectionResult, type ConnectionResultKind } from "./connectionTest";
```

增加状态和 callback：

```ts
const [testingConnection, setTestingConnection] = useState(false);
const [connectionMessage, setConnectionMessage] = useState("");

const testConnection = async () => {
  if (!selected) return;
  const missing = missingConnectionFields(selected.env);
  if (missing.length) { setConnectionMessage("请先填写：" + missing.join("、") + "。"); return; }
  setTestingConnection(true); setConnectionMessage("");
  try {
    const result = await invoke<{ kind: ConnectionResultKind }>("test_connection", {
      input: { baseUrl: selected.env.ANTHROPIC_BASE_URL, authToken: selected.env.ANTHROPIC_AUTH_TOKEN, model: selected.env.ANTHROPIC_MODEL },
    });
    setConnectionMessage(presentConnectionResult(result.kind));
  } catch {
    setConnectionMessage(presentConnectionResult("network"));
  } finally { setTestingConnection(false); }
};
```

该 callback 不得调用 `save_provider`、`switch_provider` 或 `import_current_env`。

- [ ] **Step 2: 增加按钮、结果和两个开关**

API Key label 后插入：

```tsx
<div className="connection-test-row">
  <button className="secondary" disabled={testingConnection} onClick={() => void testConnection()}>
    {testingConnection ? "正在测试…" : "测试连接"}
  </button>
  {connectionMessage && <span className="connection-result">{connectionMessage}</span>}
</div>
```

既有 Agent Teams 开关后插入：

```tsx
<label className="toggle"><input type="checkbox" checked={selected.env.CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC === "1"} onChange={event => setEnv("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", event.target.checked ? "1" : "0")} /><span><strong>关闭非必要网络流量</strong><small>写入 CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1</small></span></label>
<label className="toggle"><input type="checkbox" checked={selected.env.CLAUDE_CODE_ATTRIBUTION_HEADER === "0"} onChange={event => setEnv("CLAUDE_CODE_ATTRIBUTION_HEADER", event.target.checked ? "0" : "1")} /><span><strong>关闭 Attribution Header</strong><small>写入 CLAUDE_CODE_ATTRIBUTION_HEADER=0</small></span></label>
```

在 `src/styles.css` 末尾增加：

```css
.connection-test-row { display:flex; align-items:center; gap:10px; margin-top:-3px; }
.connection-result { color:#52647d; font-size:13px; font-weight:700; }
.connection-test-row button:disabled { cursor:wait; opacity:.65; }
```

- [ ] **Step 3: 更新 README**

添加“测试连接”小节：使用当前未保存的 API 地址、Key 和主模型发送 `max_tokens: 1` 请求；不会保存测试内容或改写 `settings.json`；请求可能消耗极少量供应商额度。

- [ ] **Step 4: 全量验证、签名构建并提交**

Run:

```bash
npm test
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
signing_password=$(security find-generic-password -a xiaogouzi -s "Claude Env Switcher updater signing password" -w)
export TAURI_SIGNING_PRIVATE_KEY=/Users/xiaogouzi/.tauri/claude-env-switcher.key
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$signing_password"
export PATH=/Users/xiaogouzi/.rustup/toolchains/stable-aarch64-apple-darwin/lib/rustlib/aarch64-apple-darwin/bin:$PATH
npm run tauri build -- --bundles app
git add src/main.tsx src/styles.css README.md
git commit -m "feat: add provider connection test controls"
git status --short
```

Expected: Vitest、TypeScript、Rust 测试均通过，生成签名 macOS `.app`，没有待提交的产品文件。

