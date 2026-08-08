# Provider Connection Test Design

## Goal

为 CC Env Switcher 的供应商方案增加两项 Claude Code 环境变量，并让用户能够在保存或切换前，用当前编辑值验证 Anthropic 兼容服务的地址、凭据与主模型。

## Scope

本次功能包含：

- 新方案默认包含 `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1`。
- 新方案默认包含 `CLAUDE_CODE_ATTRIBUTION_HEADER=0`。
- 在“Agent 行为”区域提供与这两个变量对应的开关。
- 在“连接与主模型”区域提供“测试连接”操作与结果反馈。

本次功能不包含：

- 自动保存测试中的编辑内容。
- 修改 `~/.claude/settings.json` 或创建备份。
- 保存或展示 API Key、请求正文、响应正文或完整服务端错误。
- 提供针对各供应商的专有测试协议。

## User Experience

### Environment variable controls

“Agent 行为”卡片新增两个开关：

- “关闭非必要网络流量”：选中时写入 `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1`，取消选中时写入 `0`。
- “关闭 Attribution Header”：选中时写入 `CLAUDE_CODE_ATTRIBUTION_HEADER=0`，取消选中时写入 `1`。

新建和从当前配置导入的方案都采用这两个变量的默认值。已有方案在编辑时按实际保存的值显示；未保存的方案会在新字段缺失时使用默认值。

### Connection test

“测试连接”位于“连接与主模型”卡片中的 API Key 输入项下方。点击时直接读取当前表单值，不要求先保存，不写入本地供应商方案，不改动 Claude 配置。

当 API 地址、API Key 或主模型为空时，前端不发起请求，直接指出缺少的字段。发起测试后，按钮显示进行中状态并禁用，直到请求完成或超时。

## Request Contract

桌面端 Rust 后台负责网络请求，前端不得直接向供应商 API 发起请求。这样避免浏览器跨域限制，并将 API Key 限制在应用进程内存和原生请求中。

后台将 API 地址去除末尾斜杠，再拼接 `/v1/messages`。请求采用 Anthropic Messages 兼容格式：

- 方法：`POST`
- 请求头：`content-type: application/json`、`x-api-key`、`authorization`、`anthropic-version: 2023-06-01`
- 请求体：当前 `ANTHROPIC_MODEL`、`max_tokens: 1`，以及一条固定的 `ping` 用户消息
- 超时：20 秒

收到任意成功状态码即判定为“连接成功，可使用此模型”。该测试会消耗极少量供应商额度。

## Error Handling and Privacy

测试结果只在当前界面内显示：

- 401 或 403：提示检查 API Key。
- 400 或 404：提示检查 API 地址或模型名称。
- 408、429、5xx：提示服务暂时不可用或请求超时，可稍后重试。
- 网络错误或本地超时：提示检查网络与 API 地址。

日志、界面、持久化配置与错误文本中不得出现 API Key、Authorization 头、请求体或响应体。测试失败不得改变用户已经保存的方案。

## Architecture

- `providerTemplate` 负责两个新环境变量的默认值。
- 前端编辑器负责显示开关、维护当前草稿、触发测试并展示状态。
- 一个独立的前端纯函数模块负责表单校验和将后台结果映射为用户提示，便于单元测试。
- Tauri Rust command 负责验证请求输入、构造最小 Messages 请求、设置超时并返回分类结果，不记录敏感内容。

## Acceptance Criteria

1. 新建方案包含两项环境变量，默认值分别为 `1` 和 `0`。
2. 两个开关保存后，切换方案会将其写进 `settings.json` 的顶层 `env`。
3. 填写地址、Key、主模型后可点击测试；测试不保存方案且不触碰 `settings.json`。
4. 空字段不会产生网络请求，并提示缺少字段。
5. 最小请求使用 `/v1/messages`、`max_tokens: 1` 与当前主模型，并在 20 秒内结束。
6. 成功、认证失败、参数失败、限流/服务失败、网络失败均有不含敏感信息的可行动提示。
7. 单元测试覆盖默认变量、表单校验和结果提示映射；Rust 测试覆盖请求 URL、请求内容与错误分类。
