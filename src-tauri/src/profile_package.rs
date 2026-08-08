use crate::providers::ProviderProfile;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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

