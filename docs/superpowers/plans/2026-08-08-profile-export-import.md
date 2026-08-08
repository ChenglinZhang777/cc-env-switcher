# 方案配置导出与导入 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把本机保存的全部供应商方案一次性导出，在另一台机器上导入，不必逐个手工重建。

**Architecture:** 新增 Rust 模块 `profile_package.rs` 承担打包与解析（纯函数，可单测），`main.rs` 增加四个薄命令（导出/导入 × 文件/剪贴板）。配置包不含方案 ID，导入时在本机重新生成，使「只追加不覆盖」成为结构性保证而非需要遵守的规则。

**Tech Stack:** Rust + Tauri 2、`tauri-plugin-dialog`（已有）、`tauri-plugin-clipboard-manager`（新增）、React 19 前端、Rust 内置测试 + vitest。

## Global Constraints

- 所有面向用户的文案用中文；代码注释用中文。
- 分号用中文分号「；」，不用英文分号（见提交 `5ea9ec0`）。
- 配置包 `kind` 固定为 `cc-env-switcher-profiles`，`version` 当前唯一合法值为 `1`。
- 配置包**不含**方案 ID；导入时用 `providers::new_id()` 重新生成。
- 撞名判定按原样逐字符比较，不忽略大小写、不裁剪首尾空格。
- 凡弹出对话框或读剪贴板的 Tauri 命令**必须**声明为 `async`，否则与事件循环死锁（表现为窗口无响应而非报错）。
- 新增依赖锁定版本：`tauri-plugin-clipboard-manager = "2"`（解析为 2.3.2）。
- 写入方案列表必须复用 `providers::save_profiles`（临时文件 + 原子替换）。

## File Structure

| 文件 | 职责 |
| --- | --- |
| `src-tauri/src/profile_package.rs` | 新建。打包、解析、合并三个纯函数 + 单元测试。不接触文件系统。 |
| `src-tauri/src/lib.rs` | 修改。注册新模块。 |
| `src-tauri/src/providers.rs` | 修改。`new_id()` 由私有改为 `pub(crate)`。 |
| `src-tauri/Cargo.toml` | 修改。加剪贴板依赖。 |
| `src-tauri/src/main.rs` | 修改。注册剪贴板插件 + 四个命令。 |
| `src/exportImport.ts` | 新建。导入结果的中文文案（纯函数，可 vitest）。 |
| `src/exportImport.test.ts` | 新建。上述文案的测试。 |
| `src/main.tsx` | 修改。工具栏两个按钮 + 载体小菜单。 |
| `src/styles.css` | 修改。小菜单样式。 |
| `README.md` | 修改。补明文 Key 提醒。 |

---

### Task 1: 配置包的打包与解析

**Files:**
- Create: `src-tauri/src/profile_package.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `providers::ProviderProfile`（字段 `id: String`、`name: String`、`env: BTreeMap<String, String>`）
- Produces:
  - `pub const KIND: &str = "cc-env-switcher-profiles";`
  - `pub const VERSION: u32 = 1;`
  - `pub struct PackagedProfile { pub name: String, pub env: BTreeMap<String, String> }`
  - `pub fn pack(profiles: &[ProviderProfile]) -> Result<String, String>`
  - `pub fn parse(text: &str) -> Result<Vec<PackagedProfile>, String>`

- [ ] **Step 1: 写失败的测试**

创建 `src-tauri/src/profile_package.rs`，只写测试模块：

```rust
use crate::providers::ProviderProfile;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ProviderProfile {
        ProviderProfile {
            id: "local-id-1".into(),
            name: "DeepSeek V4".into(),
            env: BTreeMap::from([
                ("ANTHROPIC_BASE_URL".to_string(), "https://api.example.com".to_string()),
                ("ANTHROPIC_AUTH_TOKEN".to_string(), "sk-secret".to_string()),
            ]),
        }
    }

    #[test]
    fn pack_then_parse_round_trips() {
        let text = pack(&[sample()]).unwrap();
        let parsed = parse(&text).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].name, "DeepSeek V4");
        assert_eq!(parsed[0].env.get("ANTHROPIC_AUTH_TOKEN").map(String::as_str), Some("sk-secret"));
    }

    #[test]
    fn packed_text_carries_no_profile_id() {
        let text = pack(&[sample()]).unwrap();
        assert!(!text.contains("local-id-1"), "配置包不应包含方案 ID");
        assert!(!text.contains("\"id\""), "配置包不应包含 id 字段");
    }

    #[test]
    fn parse_rejects_wrong_kind() {
        let text = r#"{"kind":"something-else","version":1,"profiles":[]}"#;
        let error = parse(text).unwrap_err();
        assert!(error.contains("不是"), "应说明这不是配置文件，实际：{error}");
    }

    #[test]
    fn parse_rejects_broken_json() {
        let error = parse("{ not json").unwrap_err();
        assert!(error.contains("损坏"), "应说明内容损坏，实际：{error}");
    }

    #[test]
    fn parse_rejects_future_version() {
        let text = r#"{"kind":"cc-env-switcher-profiles","version":2,"profiles":[]}"#;
        let error = parse(text).unwrap_err();
        assert!(error.contains("升级"), "应要求升级应用，实际：{error}");
    }

    #[test]
    fn parse_rejects_non_string_env_value() {
        let text = r#"{"kind":"cc-env-switcher-profiles","version":1,
            "profiles":[{"name":"X","env":{"ANTHROPIC_MODEL":123}}]}"#;
        let error = parse(text).unwrap_err();
        assert!(error.contains("损坏"), "非字符串值应按损坏处理，实际：{error}");
    }

    #[test]
    fn parse_accepts_empty_profile_list() {
        let text = r#"{"kind":"cc-env-switcher-profiles","version":1,"profiles":[]}"#;
        assert_eq!(parse(text).unwrap().len(), 0, "空列表是合法包，应与损坏区分");
    }
}
```

在 `src-tauri/src/lib.rs` 末尾加一行注册模块：

```rust
pub mod profile_package;
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cd src-tauri && source ~/.cargo/env && cargo test profile_package`
Expected: 编译失败，报 `cannot find function pack` / `cannot find function parse`。

- [ ] **Step 3: 写最小实现**

在 `profile_package.rs` 的 `#[cfg(test)] mod tests` **之前**插入：

```rust
pub const KIND: &str = "cc-env-switcher-profiles";
pub const VERSION: u32 = 1;

/// 配置包里的单个方案：只有名称和环境变量，故意不含 ID。
/// 不带 ID 时导入无法指向已有方案，「只追加不覆盖」成为结构性保证。
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct PackagedProfile {
    pub name: String,
    pub env: BTreeMap<String, String>,
}

#[derive(Deserialize, Serialize)]
struct Package {
    kind: String,
    version: u32,
    profiles: Vec<PackagedProfile>,
}

/// 只读 kind 与 version，用于在报错前区分「选错文件」和「版本过新」。
#[derive(Deserialize)]
struct Envelope {
    #[serde(default)]
    kind: String,
    #[serde(default)]
    version: u32,
}

pub fn pack(profiles: &[ProviderProfile]) -> Result<String, String> {
    let package = Package {
        kind: KIND.to_string(),
        version: VERSION,
        profiles: profiles.iter()
            .map(|profile| PackagedProfile { name: profile.name.clone(), env: profile.env.clone() })
            .collect(),
    };
    serde_json::to_string_pretty(&package).map_err(|_| "无法生成配置内容。".to_string())
}

pub fn parse(text: &str) -> Result<Vec<PackagedProfile>, String> {
    // 先只解析外层，才能把「选错文件」和「版本过新」和「损坏」分开报。
    let envelope: Envelope = serde_json::from_str(text)
        .map_err(|_| "文件内容已损坏；建议在原机器上重新导出一份。".to_string())?;
    if envelope.kind != KIND {
        return Err("这不是 CC Env Switcher 导出的配置文件；请重新选择。".to_string());
    }
    if envelope.version > VERSION {
        return Err("这份配置来自更新版本的应用；请先升级应用再导入。".to_string());
    }
    let package: Package = serde_json::from_str(text)
        .map_err(|_| "文件内容已损坏；建议在原机器上重新导出一份。".to_string())?;
    Ok(package.profiles)
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cd src-tauri && source ~/.cargo/env && cargo test profile_package`
Expected: 7 个测试全部 PASS。

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/profile_package.rs src-tauri/src/lib.rs
git commit -m "feat: add profile package pack and parse"
```

---

### Task 2: 导入合并（追加、重新生成 ID、重名后缀）

**Files:**
- Modify: `src-tauri/src/profile_package.rs`
- Modify: `src-tauri/src/providers.rs:42`（`fn new_id` 改为 `pub(crate) fn new_id`）

**Interfaces:**
- Consumes: Task 1 的 `PackagedProfile`；`providers::new_id()`
- Produces:
  - `pub struct MergeOutcome { pub profiles: Vec<ProviderProfile>, pub imported: usize, pub renamed: usize }`
  - `pub fn merge(existing: &[ProviderProfile], incoming: Vec<PackagedProfile>) -> MergeOutcome`

- [ ] **Step 1: 写失败的测试**

在 `profile_package.rs` 的 `mod tests` 内追加：

```rust
    fn packaged(name: &str) -> PackagedProfile {
        PackagedProfile { name: name.into(), env: BTreeMap::new() }
    }

    #[test]
    fn merge_appends_without_touching_existing() {
        let existing = vec![sample()];
        let outcome = merge(&existing, vec![packaged("新方案")]);
        assert_eq!(outcome.profiles.len(), 2);
        assert_eq!(outcome.profiles[0], sample(), "已有方案不得被修改");
        assert_eq!(outcome.profiles[1].name, "新方案");
        assert_eq!(outcome.imported, 1);
        assert_eq!(outcome.renamed, 0);
    }

    #[test]
    fn merge_regenerates_ids_so_they_never_collide() {
        let existing = vec![sample()];
        let outcome = merge(&existing, vec![packaged("A"), packaged("B")]);
        let ids: Vec<&str> = outcome.profiles.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids[0], "local-id-1");
        assert_ne!(ids[1], ids[2], "同批导入的 ID 必须互不相同");
        assert!(!ids[1].is_empty() && !ids[2].is_empty());
    }

    #[test]
    fn importing_same_package_twice_duplicates_instead_of_overwriting() {
        let package = vec![packaged("X")];
        let first = merge(&[], package.clone());
        let second = merge(&first.profiles, package);
        assert_eq!(second.profiles.len(), 2, "同一配置包导入两次应得到两份");
        assert_eq!(second.profiles[0].name, "X", "第一次导入的方案仍在");
        assert_eq!(second.profiles[1].name, "X（导入）");
        assert_ne!(second.profiles[0].id, second.profiles[1].id);
    }

    #[test]
    fn merge_numbers_repeated_name_collisions_in_order() {
        let existing = vec![
            ProviderProfile { id: "1".into(), name: "X".into(), env: BTreeMap::new() },
            ProviderProfile { id: "2".into(), name: "X（导入）".into(), env: BTreeMap::new() },
        ];
        let outcome = merge(&existing, vec![packaged("X"), packaged("X")]);
        assert_eq!(outcome.profiles[2].name, "X（导入 2）");
        assert_eq!(outcome.profiles[3].name, "X（导入 3）");
        assert_eq!(outcome.renamed, 2);
    }

    #[test]
    fn merge_treats_different_case_as_different_names() {
        let existing = vec![ProviderProfile { id: "1".into(), name: "x".into(), env: BTreeMap::new() }];
        let outcome = merge(&existing, vec![packaged("X")]);
        assert_eq!(outcome.profiles[1].name, "X", "大小写不同视为不同名，不加后缀");
        assert_eq!(outcome.renamed, 0);
    }

    #[test]
    fn merge_keeps_blank_names_as_is() {
        let outcome = merge(&[], vec![packaged("")]);
        assert_eq!(outcome.profiles[0].name, "", "空名称照原样导入");
        assert_eq!(outcome.renamed, 0);
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cd src-tauri && source ~/.cargo/env && cargo test profile_package`
Expected: 编译失败，报 `cannot find function merge`。

- [ ] **Step 3: 写最小实现**

先改 `src-tauri/src/providers.rs` 第 42 行，把 `fn new_id()` 的可见性放开：

```rust
pub(crate) fn new_id() -> String {
```

然后在 `profile_package.rs` 的 `mod tests` 之前追加：

```rust
pub struct MergeOutcome {
    pub profiles: Vec<ProviderProfile>,
    pub imported: usize,
    pub renamed: usize,
}

/// 追加导入：已有方案一个字节都不动，导入项重新生成 ID 并在撞名时加后缀。
pub fn merge(existing: &[ProviderProfile], incoming: Vec<PackagedProfile>) -> MergeOutcome {
    let mut profiles = existing.to_vec();
    let mut renamed = 0;
    let imported = incoming.len();

    for item in incoming {
        // 比较对象含已处理过的导入项，因此批内后缀连续递增而不重复。
        let taken: Vec<&str> = profiles.iter().map(|profile| profile.name.as_str()).collect();
        let name = if item.name.is_empty() || !taken.contains(&item.name.as_str()) {
            item.name
        } else {
            renamed += 1;
            available_name(&item.name, &taken)
        };
        profiles.push(ProviderProfile { id: crate::providers::new_id(), name, env: item.env });
    }

    MergeOutcome { profiles, imported, renamed }
}

/// 撞名时找一个没被占用的名字：先试「X（导入）」，再试「X（导入 2）」依次递增。
fn available_name(base: &str, taken: &[&str]) -> String {
    let first = format!("{base}（导入）");
    if !taken.contains(&first.as_str()) {
        return first;
    }
    for suffix in 2.. {
        let candidate = format!("{base}（导入 {suffix}）");
        if !taken.contains(&candidate.as_str()) {
            return candidate;
        }
    }
    unreachable!()
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cd src-tauri && source ~/.cargo/env && cargo test`
Expected: 全部 PASS（Task 1 的 7 个 + 本任务 6 个 + 既有 providers/claude_settings 测试）。

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/profile_package.rs src-tauri/src/providers.rs
git commit -m "feat: add append-only merge for imported profiles"
```

---

### Task 3: 四个 Tauri 命令

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/main.rs`

**Interfaces:**
- Consumes: Task 1 的 `pack` / `parse`；Task 2 的 `merge` / `MergeOutcome`；`providers::load_profiles` / `save_profiles`
- Produces（前端调用名与返回形状）：
  - `export_profiles_to_file() -> Result<bool, String>`（`false` = 用户取消）
  - `export_profiles_to_clipboard() -> Result<bool, String>`
  - `import_profiles_from_file() -> Result<Option<ImportSummary>, String>`（`None` = 用户取消）
  - `import_profiles_from_clipboard() -> Result<ImportSummary, String>`
  - `ImportSummary` 序列化为 `{ imported: number, renamed: number }`

- [ ] **Step 1: 加依赖并注册插件**

在 `src-tauri/Cargo.toml` 的 `[dependencies]` 中，`tauri-plugin-dialog` 之后插入一行：

```toml
tauri-plugin-clipboard-manager = "2"
```

在 `src-tauri/src/main.rs` 的 `.plugin(tauri_plugin_dialog::init())` 之后插入一行：

```rust
        .plugin(tauri_plugin_clipboard_manager::init())
```

Run: `cd src-tauri && source ~/.cargo/env && cargo build`
Expected: 编译成功，下载 `tauri-plugin-clipboard-manager v2.3.2`。

- [ ] **Step 2: 写四个命令**

在 `src-tauri/src/main.rs` 的 `fn main()` **之前**插入。注意四个命令全部是 `async`：阻塞式对话框和 `read_text` 都不能在主线程调用，而不带 `async` 的命令就跑在主线程上，撞上会死锁成窗口无响应。

```rust
use cc_env_switcher_lib::profile_package;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};

#[derive(serde::Serialize)]
struct ImportSummary { imported: usize, renamed: usize }

/// 导出前的明文提醒。用户点取消返回 false，调用方须中止导出。
fn confirm_plaintext_export(app: &tauri::AppHandle) -> bool {
    app.dialog()
        .message("导出的配置包含 API Key 明文。文件或剪贴板内容离开本机后，取得它的人即可使用你的全部 Key。")
        .title("确认导出？")
        .buttons(MessageDialogButtons::OkCancelCustom("继续导出".into(), "取消".into()))
        .blocking_show()
}

#[tauri::command]
async fn export_profiles_to_file(app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let text = profile_package::pack(&providers::load_profiles(&state.providers_path)?)?;
    if !confirm_plaintext_export(&app) { return Ok(false); }
    let stamp = chrono::Local::now().format("%Y%m%d");
    let Some(target) = app.dialog().file()
        .set_title("导出全部方案")
        .set_file_name(format!("cc-env-switcher-profiles-{stamp}.json"))
        .add_filter("JSON", &["json"])
        .blocking_save_file()
    else { return Ok(false) };
    let path = target.into_path().map_err(|error| error.to_string())?;
    fs::write(path, text).map_err(|error| format!("写入文件失败：{error}"))?;
    Ok(true)
}

#[tauri::command]
async fn export_profiles_to_clipboard(app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let text = profile_package::pack(&providers::load_profiles(&state.providers_path)?)?;
    if !confirm_plaintext_export(&app) { return Ok(false); }
    app.clipboard().write_text(text).map_err(|error| format!("写入剪贴板失败：{error}"))?;
    Ok(true)
}

/// 解析后合并落盘。抽出来是因为文件和剪贴板两条路只有取文本的方式不同。
fn apply_import(state: &AppState, text: &str) -> Result<ImportSummary, String> {
    let incoming = profile_package::parse(text)?;
    if incoming.is_empty() {
        return Err("这份配置里没有方案；请确认原机器上已保存方案后重新导出。".to_string());
    }
    let outcome = profile_package::merge(&providers::load_profiles(&state.providers_path)?, incoming);
    providers::save_profiles(&state.providers_path, &outcome.profiles)?;
    Ok(ImportSummary { imported: outcome.imported, renamed: outcome.renamed })
}

#[tauri::command]
async fn import_profiles_from_file(app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> Result<Option<ImportSummary>, String> {
    let Some(source) = app.dialog().file()
        .set_title("导入方案")
        .add_filter("JSON", &["json"])
        .blocking_pick_file()
    else { return Ok(None) };
    let path = source.into_path().map_err(|error| error.to_string())?;
    let text = fs::read_to_string(path).map_err(|error| format!("读取文件失败：{error}"))?;
    apply_import(&state, &text).map(Some)
}

#[tauri::command]
async fn import_profiles_from_clipboard(app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> Result<ImportSummary, String> {
    let text = app.clipboard().read_text()
        .map_err(|_| "剪贴板里没有可读文本；请先在原机器上点导出到剪贴板。".to_string())?;
    apply_import(&state, &text)
}
```

- [ ] **Step 3: 注册命令**

把 `main.rs` 里 `invoke_handler` 那一行的命令列表末尾补上四个新命令：

```rust
        .invoke_handler(tauri::generate_handler![list_providers, save_provider, delete_provider, import_current_env, read_active_env, switch_provider, test_connection, backups_path, open_backups_directory, export_profiles_to_file, export_profiles_to_clipboard, import_profiles_from_file, import_profiles_from_clipboard])
```

- [ ] **Step 4: 编译确认通过**

Run: `cd src-tauri && source ~/.cargo/env && cargo build 2>&1 | tail -5`
Expected: `Finished` 且无 error。若报 `into_path` 找不到，确认 `use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};` 已加——`into_path` 来自 `FilePath`，由 dialog 插件重导出。

- [ ] **Step 5: 跑全部 Rust 测试**

Run: `cd src-tauri && source ~/.cargo/env && cargo test`
Expected: 全部 PASS，无新增失败。

- [ ] **Step 6: 提交**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/main.rs
git commit -m "feat: add export and import commands for profiles"
```

---

### Task 4: 导入结果文案

**Files:**
- Create: `src/exportImport.ts`
- Create: `src/exportImport.test.ts`

**Interfaces:**
- Produces:
  - `export type ImportSummary = { imported: number; renamed: number }`
  - `export function presentImportSummary(summary: ImportSummary): string`

- [ ] **Step 1: 写失败的测试**

创建 `src/exportImport.test.ts`：

```ts
import { describe, expect, it } from "vitest";
import { presentImportSummary } from "./exportImport";

describe("presentImportSummary", () => {
  it("只报数量，没有重名时不提后缀", () => {
    expect(presentImportSummary({ imported: 3, renamed: 0 })).toBe("已导入 3 个方案。");
  });

  it("有重名时说明改了几个名字", () => {
    expect(presentImportSummary({ imported: 3, renamed: 2 })).toBe("已导入 3 个方案；其中 2 个因重名已改名。");
  });

  it("单个方案也照常报数", () => {
    expect(presentImportSummary({ imported: 1, renamed: 0 })).toBe("已导入 1 个方案。");
  });
});
```

- [ ] **Step 2: 跑测试确认失败**

Run: `npm test -- exportImport`
Expected: FAIL，报找不到模块 `./exportImport`。

- [ ] **Step 3: 写最小实现**

创建 `src/exportImport.ts`：

```ts
export type ImportSummary = { imported: number; renamed: number };

/** 报数而不只说「成功」，否则用户无法判断是否有遗漏。 */
export function presentImportSummary(summary: ImportSummary): string {
  const base = `已导入 ${summary.imported} 个方案`;
  return summary.renamed > 0 ? `${base}；其中 ${summary.renamed} 个因重名已改名。` : `${base}。`;
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `npm test -- exportImport`
Expected: 3 个测试 PASS。

- [ ] **Step 5: 提交**

```bash
git add src/exportImport.ts src/exportImport.test.ts
git commit -m "feat: add import summary presentation"
```

---

### Task 5: 工具栏按钮与载体小菜单

**Files:**
- Modify: `src/main.tsx`
- Modify: `src/styles.css`

**Interfaces:**
- Consumes: Task 3 的四个命令名与返回形状；Task 4 的 `presentImportSummary`

- [ ] **Step 1: 加导入语句**

在 `src/main.tsx` 第 8 行（`import { errorFeedback, ... } from "./feedback";`）之后插入：

```tsx
import { presentImportSummary, type ImportSummary } from "./exportImport";
```

- [ ] **Step 2: 加状态与四个处理函数**

在 `const [installing, setInstalling] = useState(false);`（第 28 行）之后插入状态：

```tsx
  const [transferMenu, setTransferMenu] = useState<"" | "export" | "import">("");
```

在 `applyUpdate` 函数（第 104-109 行）之后插入四个处理函数：

```tsx
  const exportProfiles = async (target: "file" | "clipboard") => {
    setTransferMenu("");
    const command = target === "file" ? "export_profiles_to_file" : "export_profiles_to_clipboard";
    try {
      const done = await invoke<boolean>(command);
      // 用户在确认框或保存对话框点了取消，不算失败，不打扰。
      if (done) notify(successFeedback(target === "file" ? "方案已导出到文件。" : "方案已复制到剪贴板。"));
    } catch (error) { notify(errorFeedback(`导出失败：${String(error)}`)); }
  };
  const importProfiles = async (source: "file" | "clipboard") => {
    setTransferMenu("");
    try {
      const summary = source === "file"
        ? await invoke<ImportSummary | null>("import_profiles_from_file")
        : await invoke<ImportSummary>("import_profiles_from_clipboard");
      if (!summary) return;
      await load();
      notify(successFeedback(presentImportSummary(summary)));
    } catch (error) { notify(errorFeedback(String(error))); }
  };
```

- [ ] **Step 3: 加工具栏按钮**

把 `src/main.tsx` 第 114 行的 `header-actions` 整个 div 替换为下面这段（在「查看备份」之后插入导出与导入，各带一个载体小菜单）：

```tsx
      <div className="header-actions">
        <button className="secondary" onClick={() => void checkUpdates()}>检查更新</button>
        <button className="secondary" onClick={() => void invoke("open_backups_directory")}>查看备份</button>
        <div className="transfer-group">
          <button className="secondary" onClick={() => setTransferMenu(transferMenu === "export" ? "" : "export")}>导出全部方案</button>
          {transferMenu === "export" && <div className="transfer-menu">
            <button onClick={() => void exportProfiles("file")}>导出到文件…</button>
            <button onClick={() => void exportProfiles("clipboard")}>复制到剪贴板</button>
          </div>}
        </div>
        <div className="transfer-group">
          <button className="secondary" onClick={() => setTransferMenu(transferMenu === "import" ? "" : "import")}>导入方案</button>
          {transferMenu === "import" && <div className="transfer-menu">
            <button onClick={() => void importProfiles("file")}>从文件导入…</button>
            <button onClick={() => void importProfiles("clipboard")}>从剪贴板导入</button>
          </div>}
        </div>
        <button className="primary" onClick={() => { const item = newProvider(); setProviders([...providers, item]); setSelected(item); }}>＋ 新增方案</button>
      </div>
```

- [ ] **Step 4: 加样式**

在 `src/styles.css` 末尾追加：

```css
.transfer-group { position:relative; }
.transfer-menu { position:absolute; top:calc(100% + 6px); right:0; z-index:30; display:flex; flex-direction:column; min-width:168px; padding:6px; border:1px solid #e1e8f0; border-radius:10px; background:#fff; box-shadow:0 10px 30px #1e293b1f; }
.transfer-menu button { padding:9px 10px; border-radius:7px; background:transparent; color:#40516a; font-size:13px; font-weight:700; text-align:left; }
.transfer-menu button:hover { background:#edf3fb; }
```

- [ ] **Step 5: 类型检查与全部测试**

Run: `npm run build && npm test`
Expected: `tsc -b` 无错误，vite 构建成功，vitest 全部 PASS。

- [ ] **Step 6: 提交**

```bash
git add src/main.tsx src/styles.css
git commit -m "feat: add export and import controls to toolbar"
```

---

### Task 6: README 说明与手动验证

**Files:**
- Modify: `README.md:25-32`（「数据位置」一节）

- [ ] **Step 1: 补 README**

在 `README.md` 的「数据位置」一节，`**首次启动自动迁移**` 那段**之前**插入：

```markdown
## 导出与导入

顶部「导出全部方案」可把所有方案存成一个 JSON 文件，或复制到剪贴板；在另一台机器上点「导入方案」读回来。导入只追加，不会覆盖或删除该机器上已有的方案；同名方案会自动加「（导入）」后缀以便区分。

**导出内容包含 API Key 明文。** 这个文件或剪贴板内容一旦离开本机（聊天、邮件、网盘、U 盘），保护它的就不再是你的 macOS 账户权限，取得它的人即可使用你的全部 Key。传输后请及时删除中转副本。
```

- [ ] **Step 2: 构建应用**

Run: `source ~/.cargo/env && npm run tauri build 2>&1 | tail -5`
Expected: 构建成功，产物在 `src-tauri/target/release/bundle/`。

- [ ] **Step 3: 手动验证（照单逐条走）**

对话框与剪贴板需要真实点击，无法自动化。打开构建出的应用，逐条确认：

1. 点「导出全部方案 → 导出到文件…」→ 出现明文提醒确认框 → 点「取消」→ 不产生文件、无提示。
2. 再点一次 → 点「继续导出」→ 保存对话框默认文件名形如 `cc-env-switcher-profiles-20260808.json` → 保存 → 提示「方案已导出到文件。」
3. 点「导入方案 → 从文件导入…」→ 选刚导出的文件 → 提示导入数量，且所有方案名都带「（导入）」后缀（因为本机已有同名）。
4. 确认原有方案仍在、数量翻倍，侧边栏「已生效」标记仍指向原来那一个。
5. 点「导出全部方案 → 复制到剪贴板」→ 确认 → 提示已复制；点「导入方案 → 从剪贴板导入」→ 提示导入数量。
6. 故意选一个无关的 JSON 文件导入 → 提示「这不是 CC Env Switcher 导出的配置文件；请重新选择。」
7. 窗口在以上每一步都保持响应（验证 `async` 命令没有死锁主线程）。

任何一条不符合预期，停下来修复后重跑该条。

- [ ] **Step 4: 提交**

```bash
git add README.md
git commit -m "docs: document profile export and import"
```

---

## 完成标准

- `cd src-tauri && cargo test` 全绿；`npm test` 全绿；`npm run build` 无类型错误。
- Task 6 Step 3 的七条手动验证全部通过。
- 同一配置包导入两次得到两份副本，原有方案与「已生效」标记不受影响。
