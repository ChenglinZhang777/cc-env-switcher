use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderProfile {
    pub id: String,
    pub name: String,
    pub env: BTreeMap<String, String>,
}

pub fn load_profiles(path: &Path) -> Result<Vec<ProviderProfile>, String> {
    if !path.exists() { return Ok(vec![]); }
    serde_json::from_slice(&fs::read(path).map_err(|error| error.to_string())?).map_err(|error| error.to_string())
}

pub fn save_profiles(path: &Path, profiles: &[ProviderProfile]) -> Result<(), String> {
    let parent = path.parent().ok_or("无效的供应商配置路径")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temp = path.with_extension("json.writing");
    fs::write(&temp, serde_json::to_vec_pretty(profiles).map_err(|error| error.to_string())?).map_err(|error| error.to_string())?;
    fs::rename(temp, path).map_err(|error| error.to_string())
}

/// 预置方案：供应商字段留空，切换后这些键不会写入 Claude 配置，等于回到官方默认。
pub fn native_profile() -> ProviderProfile {
    let empty = ["ANTHROPIC_BASE_URL", "ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_MODEL",
        "ANTHROPIC_DEFAULT_OPUS_MODEL", "ANTHROPIC_DEFAULT_SONNET_MODEL",
        "ANTHROPIC_DEFAULT_HAIKU_MODEL", "ANTHROPIC_DEFAULT_FABLE_MODEL",
        "CLAUDE_CODE_SUBAGENT_MODEL"];
    let mut env: BTreeMap<String, String> = empty.iter().map(|key| (key.to_string(), String::new())).collect();
    env.insert("CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS".into(), "1".into());
    env.insert("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC".into(), "1".into());
    env.insert("CLAUDE_CODE_ATTRIBUTION_HEADER".into(), "0".into());
    env.insert("CLAUDE_CODE_EFFORT_LEVEL".into(), "max".into());
    ProviderProfile { id: new_id(), name: "官方 Claude".into(), env }
}

/// 生成方案 ID。不引入 uuid 依赖，用时间戳加计数器保证同一进程内不重复。
pub(crate) fn new_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|value| value.as_nanos()).unwrap_or(0);
    format!("{nanos:x}-{:x}", COUNTER.fetch_add(1, Ordering::Relaxed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn profiles_round_trip_through_local_file() {
        let path = std::env::temp_dir().join(format!("cc-env-profiles-{}.json", std::process::id()));
        let profile = ProviderProfile { id: "a".into(), name: "OpenRouter".into(), env: BTreeMap::from([("ANTHROPIC_AUTH_TOKEN".into(), "token".into())]) };
        save_profiles(&path, &[profile.clone()]).unwrap();
        assert_eq!(load_profiles(&path).unwrap(), vec![profile]);
    }

    #[test]
    fn native_profile_leaves_provider_fields_empty() {
        let profile = native_profile();
        assert_eq!(profile.name, "官方 Claude");
        for key in ["ANTHROPIC_BASE_URL", "ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_MODEL",
                    "ANTHROPIC_DEFAULT_OPUS_MODEL", "ANTHROPIC_DEFAULT_SONNET_MODEL",
                    "ANTHROPIC_DEFAULT_HAIKU_MODEL", "ANTHROPIC_DEFAULT_FABLE_MODEL",
                    "CLAUDE_CODE_SUBAGENT_MODEL"] {
            assert_eq!(profile.env.get(key).map(String::as_str), Some(""), "{key} 应为空");
        }
        assert_eq!(profile.env.get("CLAUDE_CODE_EFFORT_LEVEL").map(String::as_str), Some("max"));
        assert!(!profile.id.is_empty());
    }

    #[test]
    fn native_profiles_get_distinct_ids() {
        assert_ne!(native_profile().id, native_profile().id);
    }
}
