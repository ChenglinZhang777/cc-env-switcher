# Claude Env Switcher

轻量的 macOS 菜单栏应用，用于保存和切换 Claude Code 的模型供应商环境变量。

## 它会做什么

- 保存可新增、编辑和删除的供应商方案。
- 切换前永久完整备份 `~/.claude/settings.json`。
- 仅替换该 JSON 文件的顶层 `env` 字段；其他字段保持不变。
- 使用临时文件和原子替换写入，再读回校验 `env`。
- 通过菜单栏打开应用，或在界面中快速切换。

## 数据位置

- 供应商方案：`~/Library/Application Support/com.chenglinzhang.claude-env-switcher/providers.json`
- 备份：`~/Library/Application Support/com.chenglinzhang.claude-env-switcher/backups/`

供应商方案中的 API Key 按当前需求以明文保存在本机该 JSON 文件中；请保护你的 macOS 用户账户与备份。

## 构建

```bash
npm install
source ~/.cargo/env
npm run tauri build
```

macOS 产物位于 `src-tauri/target/release/bundle/macos/` 和 `src-tauri/target/release/bundle/dmg/`。本地构建无需安装完整 Xcode；需要 Node.js、Rust 以及 macOS Command Line Tools。
