# CC Env Switcher 自动更新与 Release 设计

## 目标

为 CC Env Switcher 增加基于公开 GitHub Release 的安全自动更新，并通过 GitHub Actions 发布 macOS 安装包。

## 发布渠道

仓库和 Release 公开。应用通过 GitHub Release 的固定 `latest/download/latest.json` 地址检查更新；客户端不需要、也不得携带 GitHub 访问令牌。

## 安全契约

自动更新使用 Tauri Updater 的非对称签名：

- 应用包只包含公开验证键。
- 用于签名更新包的私钥及其密码仅存于 GitHub Actions Secrets。
- 私钥不得提交到 Git、写入应用配置、出现在日志或 Release 附件中。
- 客户端只安装能够由内置公钥验证的更新包；签名不合法或更新清单无效时不得下载或安装。

更新私钥一旦丢失，已发布应用无法信任新版本，因此必须由仓库维护者在安全的密码管理工具中保留离线副本。

## 发布流程

1. 维护者将应用版本改为符合 SemVer 的版本并推送 `v*` 标签。
2. GitHub Actions 在 macOS runner 安装依赖、注入 Secrets、构建并签名更新产物。
3. 工作流创建公开 GitHub Release，上传 DMG、macOS 更新归档及签名。
4. 工作流生成并上传 `latest.json`，其中包含版本、发布时间、更新说明、macOS 下载地址和签名文本。
5. 普通 `main` 推送仅执行构建与测试，不创建 Release。

首次安装由用户从 Release 下载 DMG。已安装应用随后通过签名的 macOS `.app.tar.gz` 更新包下载、安装并重启。

## 应用交互

- 应用启动后在后台检查更新；网络失败保持静默，不影响供应商切换。
- 顶部提供“检查更新”入口。
- 发现更高版本时显示版本号和 Release Notes，用户可选择“立即安装”或“稍后”。
- 下载、验证、安装或重启失败时显示不含令牌或私钥的可理解错误。
- 无更新时显示“已是最新版本”。

## 配置与验收

- Tauri 配置启用 Updater，指向公开 `latest.json`，并嵌入公开验证键。
- 前端使用 Tauri Updater API 执行检查、下载与安装。
- Rust 层注册 Updater 插件。
- 发布工作流缺失所需 Secrets 时失败，不生成未签名更新。
- 发布标签后，Release 包含 DMG、`*.app.tar.gz`、对应 `.sig` 和 `latest.json`。
- 对已安装旧版本，存在更高签名版本时可完成“检查 → 下载 → 安装 → 重启”闭环；无更新、离线和验签失败均不影响现有应用使用。
