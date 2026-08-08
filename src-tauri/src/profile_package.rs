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
}

