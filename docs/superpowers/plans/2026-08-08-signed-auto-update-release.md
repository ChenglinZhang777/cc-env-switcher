# Signed Auto-Update Release Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 CC Env Switcher 发布为可从公开 GitHub Release 安全检查、下载和安装更新的 macOS 应用。

**Architecture:** Tauri Updater 插件在 App 内检查 GitHub Release 的静态 `latest.json`，只接受使用嵌入公开键验证的更新包。GitHub Actions 仅在 `v*` 标签上读取 GitHub Secrets 中的私钥签名、创建公开 Release 并上传更新清单；普通 `main` 推送只执行测试和构建。

**Tech Stack:** Tauri 2 Updater、`@tauri-apps/plugin-updater`、Rust、React、GitHub Actions、GitHub Release、Tauri 签名器。

---

## 文件结构

- `src-tauri/tauri.conf.json`：公开键、更新端点、更新产物配置。
- `src-tauri/Cargo.toml`：Rust Updater 插件依赖。
- `src-tauri/src/main.rs`：注册 Updater 插件。
- `src/update.ts`：检查、下载、安装与重启的可测试更新逻辑。
- `src/update.test.ts`：更新状态与错误呈现测试。
- `src/main.tsx`：启动后台检查、手动检查入口、更新提示。
- `.github/workflows/build.yml`：普通分支的验证构建。
- `.github/workflows/release.yml`：标签触发的签名构建、Release 与 `latest.json`。
- `README.md`：维护者密钥、发布与首次安装说明。

### Task 1: 加入可测试的更新状态逻辑

**Files:**
- Create: `src/update.ts`
- Create: `src/update.test.ts`

- [ ] **Step 1: 写失败测试。**

```ts
import { describe, expect, it } from "vitest";
import { presentUpdateResult } from "./update";

describe("presentUpdateResult", () => {
  it("describes an available signed release without exposing internal errors", () => {
    expect(presentUpdateResult({ kind: "available", version: "0.2.0", notes: "修复切换体验" }))
      .toEqual({ title: "发现新版本 0.2.0", detail: "修复切换体验", canInstall: true });
  });
});
```

- [ ] **Step 2: 运行失败测试。**

Run: `npm test -- update.test.ts`

Expected: FAIL，`./update` 不存在。

- [ ] **Step 3: 实现纯状态映射与 Tauri 更新检查适配器。**

```ts
export type UpdateResult = { kind: "available"; version: string; notes: string } | { kind: "current" } | { kind: "failed" };
export function presentUpdateResult(result: UpdateResult) {
  if (result.kind === "available") return { title: `发现新版本 ${result.version}`, detail: result.notes, canInstall: true };
  if (result.kind === "current") return { title: "已是最新版本", detail: "", canInstall: false };
  return { title: "暂时无法检查更新", detail: "不影响当前使用，可稍后重试。", canInstall: false };
}
```

- [ ] **Step 4: 添加“当前版本”和“网络失败不泄漏底层信息”测试并验证。**

Run: `npm test -- update.test.ts`

Expected: PASS，3 个测试通过。

- [ ] **Step 5: 提交。**

Run: `git add src/update.ts src/update.test.ts && git commit -m "feat: add safe update status handling"`

Expected: 提交成功。

### Task 2: 配置并注册 Tauri Updater

**Files:**
- Modify: `package.json`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: 写出令应用初始化函数必须注册 Updater 插件的失败编译检查。**

```rust
let app = tauri::Builder::default()
    .plugin(tauri_plugin_updater::Builder::new().build());
```

- [ ] **Step 2: 运行编译并确认缺少依赖。**

Run: `cd src-tauri && cargo check`

Expected: FAIL，提示找不到 `tauri_plugin_updater`。

- [ ] **Step 3: 添加固定版本的前端与 Rust 更新插件，注册插件，并在 Task 4 生成密钥后将其公开键与固定 GitHub 更新端点写入 Tauri 配置。**

- [ ] **Step 4: 使用本地生成的公开键替换占位值，运行编译与 Rust 测试。**

Run: `cd src-tauri && cargo test && cargo check`

Expected: PASS；应用能编译，现有配置切换测试仍通过。

- [ ] **Step 5: 提交。**

Run: `git add package.json package-lock.json src-tauri && git commit -m "feat: configure signed Tauri updater"`

Expected: 提交成功，私钥未被暂存。

### Task 3: 加入人工可控的更新界面

**Files:**
- Modify: `src/main.tsx`
- Modify: `src/styles.css`
- Modify: `src/update.ts`
- Modify: `src/update.test.ts`

- [ ] **Step 1: 写失败测试，要求检查失败只呈现固定的非敏感中文说明。**

```ts
it("hides transport errors", () => {
  expect(presentUpdateResult({ kind: "failed" }).detail).toBe("不影响当前使用，可稍后重试。");
});
```

- [ ] **Step 2: 运行测试确认失败。**

Run: `npm test -- update.test.ts`

Expected: FAIL，失败状态尚未实现。

- [ ] **Step 3: 实现启动后台检查、顶部“检查更新”按钮，以及有新版本时的“立即安装 / 稍后”提示。**

```ts
const update = await check();
if (update) {
  setUpdate({ kind: "available", version: update.version, notes: update.body ?? "" });
  await update.downloadAndInstall((event) => setProgress(event));
}
```

- [ ] **Step 4: 验证前端逻辑与生产构建。**

Run: `npm test && npm run build`

Expected: PASS；无更新和失败时不会阻断供应商切换界面。

- [ ] **Step 5: 提交。**

Run: `git add src/main.tsx src/styles.css src/update.ts src/update.test.ts && git commit -m "feat: add in-app update controls"`

Expected: 提交成功。

### Task 4: 安全建立签名密钥与 GitHub Secrets

**Files:**
- Modify: `README.md`

- [ ] **Step 1: 生成新的 Tauri 更新密钥对到用户私有目录，不写入项目。**

Run: `npm run tauri signer generate -- -w ~/.tauri/cc-env-switcher.key`

Expected: 输出公开键；私钥仅存在 `~/.tauri/cc-env-switcher.key`。

- [ ] **Step 2: 验证私钥不被 Git 追踪。**

Run: `git status --short && git ls-files | rg 'cc-env-switcher.key'`

Expected: 私钥不出现在状态或已追踪文件中。

- [ ] **Step 3: 将公开键写入 Tauri 配置，并用 GitHub CLI 将私钥和密码分别设置为 `TAURI_SIGNING_PRIVATE_KEY` 与 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`。**

Run: `gh secret set TAURI_SIGNING_PRIVATE_KEY --repo ChenglinZhang777/cc-env-switcher < ~/.tauri/cc-env-switcher.key`

Expected: GitHub 确认 Secret 已更新；命令输出不显示私钥。

- [ ] **Step 4: 在 README 写明仅维护者可操作的密钥备份与轮换规则。**

Run: `git add README.md src-tauri/tauri.conf.json && git commit -m "docs: document updater key custody"`

Expected: 提交成功，私钥不在提交中。

### Task 5: 创建标签发布工作流并验证

**Files:**
- Modify: `.github/workflows/build.yml`
- Create: `.github/workflows/release.yml`

- [ ] **Step 1: 写出工作流静态校验，要求标签流程仅匹配 `v*`，并写入 `latest.json`。**

```ts
expect(workflow).toContain("tags: [\"v*\"]");
expect(workflow).toContain("latest.json");
expect(workflow).toContain("TAURI_SIGNING_PRIVATE_KEY");
```

- [ ] **Step 2: 运行测试确认失败。**

Run: `npm test -- releaseWorkflow.test.ts`

Expected: FAIL，发布工作流测试和文件不存在。

- [ ] **Step 3: 实现 macOS 标签工作流：安装依赖、注入 Secrets、构建更新产物、创建公开 Release、上传 DMG、更新归档、签名和 `latest.json`。**

```yaml
on:
  push:
    tags: ["v*"]
permissions:
  contents: write
env:
  TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
  TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
```

- [ ] **Step 4: 运行全部本地测试与签名构建。**

Run: `npm test && npm run build && cd src-tauri && cargo test && cd .. && npm run tauri build`

Expected: PASS；构建目录含 `.app.tar.gz` 及其 `.sig`。

- [ ] **Step 5: 推送 `v0.2.0` 标签并验证公开 Release。**

Run: `git tag v0.2.0 && git push origin main v0.2.0 && gh release view v0.2.0 --repo ChenglinZhang777/cc-env-switcher`

Expected: Release 公开可访问，包含 DMG、更新归档、签名和 `latest.json`。
