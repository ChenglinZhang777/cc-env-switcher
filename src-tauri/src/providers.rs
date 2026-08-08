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
}
