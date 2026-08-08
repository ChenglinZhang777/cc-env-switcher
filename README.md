# CC Env Switcher

轻量的 macOS 菜单栏应用，用于保存和切换 Claude Code 的模型供应商环境变量。

## 它会做什么

- 保存可新增、编辑和删除的供应商方案。
- 切换前永久完整备份 `~/.claude/settings.json`。
- 仅替换该 JSON 文件的顶层 `env` 字段；其他字段保持不变。
- 使用临时文件和原子替换写入，再读回校验 `env`。
- 通过菜单栏打开应用，或在界面中快速切换。

## 安装

从 Release 下载 DMG，打开后把应用拖进「应用程序」文件夹。首次打开时 macOS 会提示「已损坏，无法打开」，在终端整段复制执行下面这条命令即可：

```bash
APP="/Applications/CC Env Switcher.app"; if [ -d "$APP" ]; then xattr -dr com.apple.quarantine "$APP" && codesign --force --deep --sign - "$APP" && echo "完成，现在可以打开了"; else echo "没找到 $APP，请先把应用拖进「应用程序」文件夹"; fi
```

看到「完成，现在可以打开了」就说明处理好了，双击应用即可。若提示没找到，说明应用还没拖进「应用程序」文件夹。重复执行没有副作用，升级到新版本后再跑一次即可。

它做了两件事：移除下载隔离标记，然后在本机重新生成完整签名。原因是本项目没有使用 Apple 开发者证书签名和公证（需要付费的开发者账号）。请注意「系统设置 → 隐私与安全性」里不会出现放行入口，只有这条命令这一条路。

## 数据位置

- 供应商方案：`~/Library/Application Support/com.chenglinzhang.cc-env-switcher/providers.json`
- 备份：`~/Library/Application Support/com.chenglinzhang.cc-env-switcher/backups/`

供应商方案中的 API Key 按当前需求以明文保存在本机该 JSON 文件中；请保护你的 macOS 用户账户与备份。

**首次启动自动迁移**：如果检测到旧版本（`claude-env-switcher`）的数据，会自动迁移到新路径，旧数据保留备用。

## 测试连接

在"连接与主模型"区域可直接测试当前正在编辑的 API 地址、Key 与主模型，无需先保存方案。应用会向 Anthropic 兼容的 Messages 接口发送一次 `max_tokens: 1` 的最小请求，因此可能消耗极少量供应商额度。测试不会保存当前编辑值，也不会改写 `~/.claude/settings.json`；Token、请求正文与响应正文不会显示或写入日志。

## 构建

```bash
npm install
source ~/.cargo/env
npm run tauri build
```

macOS 产物位于 `src-tauri/target/release/bundle/macos/` 和 `src-tauri/target/release/bundle/dmg/`。本地构建无需安装完整 Xcode；需要 Node.js、Rust 以及 macOS Command Line Tools。

## 自动更新与发布

应用会从公开 GitHub Release 检查签名更新，也可以在顶部点击"检查更新"。首次安装下载 DMG；后续版本由应用下载并验证更新包后重启安装。

维护者发布新版本时：同步更新 `package.json`、`src-tauri/Cargo.toml` 与 `src-tauri/tauri.conf.json` 的版本号，推送 `v<版本号>` 标签。GitHub Actions 使用 `TAURI_SIGNING_PRIVATE_KEY` 与 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` Secrets 签名更新产物。

签名私钥和密码不得提交或分享；它们必须保留在维护者的安全密码管理工具中。若丢失，已安装的应用无法信任后续更新。

---

**免责声明**：本项目是独立的第三方工具，与 Anthropic 无关联。"Claude Code" 是 Anthropic 的商标，此处仅用于描述兼容性。
