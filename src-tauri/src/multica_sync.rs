//! 把供应商方案接线成 multica 的自定义运行时。
//!
//! 一次同步做四件事：写启动器、写该方案的包装脚本、真跑一轮无头对话体检、
//! 再把运行时配置注册到全部 workspace。体检不能省——终端可达不代表能跑通，
//! 注册一个跑不通的运行时，派过去的任务会一去不回，且界面上仍显示在线。

use serde::Serialize;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// 包装脚本与各方案独立配置目录的落脚点。
pub fn runtimes_dir(home: &Path) -> PathBuf {
    home.join(".multica/runtimes")
}

/// 方案名转成文件名与命令名可用的形式。
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.extend(ch.to_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

/// 该方案对应的命令名，也是 multica 运行时配置里的 command_name。
pub fn command_name(profile_name: &str) -> String {
    format!("cc-{}", slug(profile_name))
}

/// 通用启动器。内容与本文件同版本发布，每次同步覆写，避免手改后与应用行为不一致。
///
/// 关键在 CLAUDE_CONFIG_DIR：`~/.claude` 里存着 OAuth 订阅登录态，带着它跑第三方
/// 网关的无头 `claude -p` 会挂死且无任何输出（2026-08-10 实测，单独覆盖 model 无效）。
/// 换成独立目录就没有登录态，API Key 才会被真正使用。其余配置照旧软链过去，
/// 让这些运行时和交互式用起来一致。
pub fn launcher_script() -> &'static str {
    include_str!("../resources/claude-env.sh")
}

/// 单个方案的薄包装：只负责把方案名交给启动器。
pub fn shim_script(dir: &Path, profile_name: &str) -> String {
    format!(
        "#!/bin/sh\n# 由 CC Env Switcher 生成。供应商方案：{profile_name}\nexec \"{}/claude-env\" \"{profile_name}\" \"$@\"\n",
        dir.display()
    )
}

#[derive(Serialize, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SyncOutcome {
    pub kind: String,
    pub detail: String,
    pub workspaces: usize,
}

/// 体检判定：唯一可信的信号是真的说出了话。
///
/// 曾用 curl 直连 /v1/messages 做快速预筛，两次假阴性后废弃：一次是模型名的
/// `[1m]` 后缀（Claude Code 发请求前会剥掉，直接发原文会被拒），一次是网关不备货
/// Claude 官方模型名（这类方案本就只改地址和密钥，靠网关自己映射）。
pub fn classify_probe(stdout: &str) -> Result<(), String> {
    if stdout.trim().is_empty() {
        return Err("体检未通过：跑不出任何回复。若该方案在交互式下可用，多半是这台机器缺 claude 命令。".into());
    }
    Ok(())
}

/// 从 `multica workspace list --output json` 里取出全部 workspace id。
pub fn parse_workspace_ids(json: &str) -> Result<Vec<String>, String> {
    let value: serde_json::Value = serde_json::from_str(json).map_err(|_| "workspace 列表不是有效 JSON".to_string())?;
    let ids: Vec<String> = value
        .as_array()
        .ok_or("workspace 列表格式异常")?
        .iter()
        .filter_map(|item| item.get("id")?.as_str().map(str::to_string))
        .collect();
    if ids.is_empty() {
        return Err("没有可用的 workspace；请先确认 multica 已登录。".into());
    }
    Ok(ids)
}

/// 在某个 workspace 的既有运行时配置里，按命令名找出已注册的那条。
pub fn find_profile_id(json: &str, command: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()?
        .as_array()?
        .iter()
        .find(|item| item.get("command_name").and_then(serde_json::Value::as_str) == Some(command))
        .and_then(|item| item.get("id")?.as_str().map(str::to_string))
        .filter(|id| !id.is_empty())
}

/// 已在 multica 注册的命令名集合，供界面标出哪些方案还没接线。
pub fn registered_commands(json: &str) -> Vec<String> {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .and_then(|value| {
            Some(
                value
                    .as_array()?
                    .iter()
                    .filter_map(|item| item.get("command_name")?.as_str().map(str::to_string))
                    .collect(),
            )
        })
        .unwrap_or_default()
}

fn write_executable(path: &Path, contents: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| format!("创建目录失败：{error}"))?;
    }
    let mut file = std::fs::File::create(path).map_err(|error| format!("写入 {} 失败：{error}", path.display()))?;
    file.write_all(contents.as_bytes()).map_err(|error| format!("写入 {} 失败：{error}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).map_err(|error| format!("设置执行权限失败：{error}"))?;
    }
    Ok(())
}

/// 写出启动器与该方案的包装脚本，返回包装脚本路径。
pub fn write_scripts(home: &Path, profile_name: &str) -> Result<PathBuf, String> {
    let dir = runtimes_dir(home);
    write_executable(&dir.join("claude-env"), launcher_script())?;
    let shim = dir.join(command_name(profile_name));
    write_executable(&shim, &shim_script(&dir, profile_name))?;
    Ok(shim)
}

/// 真跑一轮无头对话。超时交给 multica CLI 之外的 `timeout` 不可靠，这里直接等子进程。
fn probe(shim: &Path) -> Result<(), String> {
    let output = Command::new(shim)
        .args(["-p", "reply with: ok"])
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("无法执行包装脚本：{error}"))?;
    classify_probe(&String::from_utf8_lossy(&output.stdout))
}

fn cli(multica: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new(multica)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("无法执行 multica 命令：{error}"))?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr);
        let message = if message.trim().is_empty() { String::from_utf8_lossy(&output.stdout).to_string() } else { message.to_string() };
        return Err(message.trim().chars().take(120).collect());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// 完整同步一个方案：写脚本 → 体检 → 注册到全部 workspace 并钉死本机路径。
pub fn sync(home: &Path, multica: &Path, profile_name: &str) -> Result<SyncOutcome, String> {
    let shim = write_scripts(home, profile_name)?;
    probe(&shim)?;

    let command = command_name(profile_name);
    let shim_path = shim.display().to_string();
    let mut registered = 0;

    for workspace in parse_workspace_ids(&cli(multica, &["workspace", "list", "--output", "json"])?)? {
        let listed = cli(multica, &["runtime", "profile", "list", "--workspace-id", &workspace, "--output", "json"]).unwrap_or_default();
        let id = match find_profile_id(&listed, &command) {
            Some(existing) => existing,
            None => {
                let created = cli(multica, &[
                    "runtime", "profile", "create",
                    "--workspace-id", &workspace,
                    "--display-name", &format!("Claude Code ({profile_name})"),
                    "--command-name", &command,
                    "--protocol-family", "claude",
                    "--description", &format!("CC Env Switcher 方案：{profile_name}"),
                    "--output", "json",
                ])?;
                serde_json::from_str::<serde_json::Value>(&created).ok()
                    .and_then(|value| value.get("id")?.as_str().map(str::to_string))
                    .ok_or("创建运行时配置后没拿到 ID")?
            }
        };
        cli(multica, &["runtime", "profile", "set-path", &id, "--path", &shim_path])?;
        registered += 1;
    }

    Ok(SyncOutcome {
        kind: "success".into(),
        detail: format!("已接线到 {registered} 个 workspace；重启 multica daemon 后生效。"),
        workspaces: registered,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_collapses_punctuation_and_lowercases() {
        assert_eq!(slug("DS-v4-flash"), "ds-v4-flash");
        assert_eq!(slug("Jiaming GPT"), "jiaming-gpt");
        assert_eq!(slug("  官方 Claude  "), "claude");
        assert_eq!(command_name("Limi-claude"), "cc-limi-claude");
    }

    #[test]
    fn shim_delegates_to_launcher_with_profile_name() {
        let script = shim_script(Path::new("/home/u/.multica/runtimes"), "DS-v4-flash");
        assert!(script.starts_with("#!/bin/sh\n"));
        assert!(script.contains("exec \"/home/u/.multica/runtimes/claude-env\" \"DS-v4-flash\" \"$@\""));
    }

    #[test]
    fn launcher_isolates_config_dir_so_the_login_session_cannot_leak_in() {
        let script = launcher_script();
        assert!(script.contains("CLAUDE_CONFIG_DIR"), "启动器必须隔离配置目录，否则第三方网关会挂死");
        assert!(script.contains("exec claude"));
    }

    #[test]
    fn probe_requires_an_actual_reply() {
        assert!(classify_probe("ok\n").is_ok());
        assert!(classify_probe("").is_err());
        assert!(classify_probe("   \n  ").is_err());
    }

    #[test]
    fn reads_workspace_ids_and_rejects_an_empty_account() {
        let ids = parse_workspace_ids(r#"[{"id":"a","slug":"personal"},{"id":"b","slug":"ai"}]"#).unwrap();
        assert_eq!(ids, vec!["a", "b"]);
        assert!(parse_workspace_ids("[]").is_err());
        assert!(parse_workspace_ids("not json").is_err());
    }

    #[test]
    fn matches_an_existing_profile_by_command_name() {
        let listed = r#"[{"id":"p1","command_name":"cc-ds-v4-flash"},{"id":"p2","command_name":"cc-other"}]"#;
        assert_eq!(find_profile_id(listed, "cc-ds-v4-flash"), Some("p1".into()));
        assert_eq!(find_profile_id(listed, "cc-missing"), None);
        assert_eq!(find_profile_id("", "cc-ds-v4-flash"), None);
        assert_eq!(registered_commands(listed), vec!["cc-ds-v4-flash", "cc-other"]);
        assert!(registered_commands("boom").is_empty());
    }
}
