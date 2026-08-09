#!/bin/sh
# 由 CC Env Switcher 生成，每次同步覆写；要改行为请改应用里的这份模板。
#
#   claude-env <方案名> [claude 的参数...]
#
# 方案名先在 CC Env Switcher 的 providers.json 里找，找不到再回退到
# ~/.claude/settings.json.<方案名>。密钥在运行时才读，不会复制进任何脚本。
set -e

PROVIDERS="$HOME/Library/Application Support/com.chenglinzhang.cc-env-switcher/providers.json"

name="$1"
if [ -z "$name" ]; then
  echo "用法: claude-env <方案名> [claude 的参数...]" >&2
  exit 2
fi
shift

env_json=""

if [ -r "$PROVIDERS" ]; then
  env_json=$(jq -r --arg n "$name" '[.[] | select(.name == $n)] | if length > 0 then .[0].env else empty end' "$PROVIDERS")
fi

if [ -z "$env_json" ]; then
  settings="$HOME/.claude/settings.json.$name"
  if [ -r "$settings" ]; then
    env_json=$(jq -r '.env // empty' "$settings")
  fi
fi

if [ -z "$env_json" ]; then
  echo "claude-env: 找不到名为「$name」的方案" >&2
  exit 1
fi

# 空值会把可用的默认模型盖成空模型名，必须滤掉。
eval "$(printf '%s' "$env_json" | jq -r 'to_entries[] | select(.value != null and .value != "") | "export \(.key)=\(.value | @sh)"')"

# 每个方案一个独立配置目录，刻意与 ~/.claude 分开。
#
# ~/.claude 里存着 OAuth 订阅登录态。带着它跑第三方网关的无头 claude -p 会超时且
# 零输出（2026-08-10 实测；单独用 --model 覆盖钉死的模型无效）。独立目录没有登录态，
# ANTHROPIC_AUTH_TOKEN 才会被真正使用。
slug=$(printf '%s' "$name" | tr '[:upper:]' '[:lower:]' | sed 's/[^a-z0-9]\{1,\}/-/g; s/^-//; s/-$//')
CLAUDE_CONFIG_DIR="$HOME/.multica/runtimes/config/$slug"
export CLAUDE_CONFIG_DIR
mkdir -p "$CLAUDE_CONFIG_DIR"

# 隔离的只有登录态。其余配置软链过来，让这些运行时与交互式行为一致；
# 用软链而非拷贝，改 ~/.claude 会立刻对所有方案生效。
for item in CLAUDE.md skills plugins commands hooks memory; do
  [ -e "$HOME/.claude/$item" ] || continue
  [ -e "$CLAUDE_CONFIG_DIR/$item" ] || ln -s "$HOME/.claude/$item" "$CLAUDE_CONFIG_DIR/$item"
done

# settings.json 重新生成而不是软链：交互式那份钉着 model（opus[1m]），第三方网关没有
# 这个模型；它的 env 块也会和上面注入的供应商变量打架。
if [ -r "$HOME/.claude/settings.json" ]; then
  jq 'del(.model, .env)' "$HOME/.claude/settings.json" > "$CLAUDE_CONFIG_DIR/settings.json.tmp" 2>/dev/null \
    && mv "$CLAUDE_CONFIG_DIR/settings.json.tmp" "$CLAUDE_CONFIG_DIR/settings.json" \
    || rm -f "$CLAUDE_CONFIG_DIR/settings.json.tmp"
fi

exec claude "$@"
