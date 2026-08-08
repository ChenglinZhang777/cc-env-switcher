# Claude Env Switcher Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 交付一个与 cc-switch 同类的 Tauri 2 macOS 菜单栏应用：管理供应商配置，备份后仅切换 Claude 的 `env`。

**Architecture:** React/TypeScript 实现管理界面；Rust/Tauri 实现系统托盘、应用数据存储、对 `~/.claude/settings.json` 的读取、永久备份、原子替换和写后校验。供应商保存在应用数据目录的 SQLite 数据库中，密钥依照确认过的需求以明文存储。

**Tech Stack:** Tauri 2、Rust、React、TypeScript、Vite、SQLite（rusqlite）、Vitest。

---

### Task 1: 初始化隔离仓库和 Tauri 工程

**Files:**
- Create: `package.json`, `vite.config.ts`, `tsconfig.json`, `index.html`
- Create: `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/src/main.rs`
- Create: `src/main.tsx`

- [ ] 写入 Tauri 2、React、Vitest 和 Rust 依赖配置。
- [ ] 运行 `npm install` 与 `npm run tauri dev`；确认菜单栏应用启动。
- [ ] 提交：`chore: initialize Tauri desktop app`。

### Task 2: 测试先行实现安全配置切换核心

**Files:**
- Create: `src-tauri/src/claude_settings.rs`
- Create: `src-tauri/src/claude_settings_tests.rs`

- [ ] 编写 `switch_env_creates_backup_and_preserves_other_fields` 测试，使用临时目录；运行 `cargo test`，确认因实现缺失失败。
- [ ] 实现：合法 JSON 检查、完整原文件备份、时间戳唯一命名、仅替换顶层 `env`、同目录临时文件原子替换、重读校验。
- [ ] 加入文件缺失、JSON 无效、备份失败和校验失败测试；运行 `cargo test` 并提交：`feat: safely switch Claude env`。

### Task 3: 测试先行实现供应商数据库与 Tauri 命令

**Files:**
- Create: `src-tauri/src/providers.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] 编写方案新增、编辑、删除、排序、重启可读回测试；先运行并确认失败。
- [ ] 实现 SQLite schema 和 Tauri commands：`list_providers`、`save_provider`、`delete_provider`、`move_provider`、`import_current_env`、`switch_provider`、`open_backups_directory`。
- [ ] 运行 `cargo test` 并提交：`feat: add provider storage and commands`。

### Task 4: 构建轻量界面与菜单栏

**Files:**
- Create: `src/App.tsx`, `src/styles.css`
- Modify: `src/main.tsx`, `src-tauri/src/main.rs`

- [ ] 编写前端测试：加载方案、保存编辑内容、切换失败显示无密钥错误。
- [ ] 实现供应商列表、名称和环境变量编辑、导入、新增、删除、排序、切换、备份目录入口。
- [ ] 实现动态系统托盘菜单：列出方案、标记当前成功切换的方案、打开管理窗口、打开备份目录和退出。
- [ ] 运行 `npm test`、`npm run build`、`cargo test` 并提交：`feat: add provider management UI and tray switching`。

### Task 5: 文档、打包和 GitHub

**Files:**
- Create: `README.md`, `.github/workflows/release.yml`, `.gitignore`

- [ ] 记录数据路径、明文密钥警示、备份不自动清理和本地运行/打包方法。
- [ ] 配置 GitHub Actions，在 macOS runner 产出已签名配置可选的 DMG 工件。
- [ ] 运行 `npm test && npm run build && cargo test && npm run tauri build`；检查生成的 `.app` 和 DMG。
- [ ] 创建私有 GitHub 仓库 `claude-env-switcher`，推送 `main`，验证远程分支和 Actions 工作流。

