use serde_json::Value;
use std::fs;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum SwitchError {
    #[error("找不到 Claude 配置文件：{0}")]
    SettingsMissing(String),
    #[error("Claude 配置文件不是有效的 JSON")]
    InvalidJson,
    #[error("无法创建备份：{0}")]
    Backup(#[source] std::io::Error),
    #[error("无法写入 Claude 配置文件：{0}")]
    Write(#[source] std::io::Error),
    #[error("写入后的环境变量校验失败")]
    VerificationFailed,
}

pub fn switch_env(settings_path: &Path, backups_dir: &Path, env: &Value) -> Result<(), SwitchError> {
    if !settings_path.is_file() {
        return Err(SwitchError::SettingsMissing(settings_path.display().to_string()));
    }
    let original = fs::read(settings_path).map_err(SwitchError::Write)?;
    let mut document: Value = serde_json::from_slice(&original).map_err(|_| SwitchError::InvalidJson)?;
    let object = document.as_object_mut().ok_or(SwitchError::InvalidJson)?;
    if !env.is_object() {
        return Err(SwitchError::InvalidJson);
    }

    fs::create_dir_all(backups_dir).map_err(SwitchError::Backup)?;
    let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S-%3f");
    let backup_path = backups_dir.join(format!("settings-{stamp}.json"));
    fs::write(&backup_path, &original).map_err(SwitchError::Backup)?;

    object.insert("env".into(), env.clone());
    let replacement = serde_json::to_vec_pretty(&document).map_err(|_| SwitchError::InvalidJson)?;
    let temporary = settings_path.with_extension(format!("json.switching-{stamp}"));
    fs::write(&temporary, replacement).map_err(SwitchError::Write)?;
    fs::rename(&temporary, settings_path).map_err(SwitchError::Write)?;

    let verified: Value = serde_json::from_slice(&fs::read(settings_path).map_err(SwitchError::Write)?)
        .map_err(|_| SwitchError::VerificationFailed)?;
    if verified.get("env") != Some(env) {
        return Err(SwitchError::VerificationFailed);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn test_root() -> PathBuf {
        let root = std::env::temp_dir().join(format!("claude-env-switcher-test-{}", uuid()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn uuid() -> String { format!("{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()) }

    #[test]
    fn switch_env_creates_backup_and_preserves_other_fields() {
        let root = test_root();
        let settings = root.join("settings.json");
        let backups = root.join("backups");
        fs::write(&settings, r#"{"env":{"OLD":"1"},"permissions":{"allow":["Bash"]}}"#).unwrap();
        switch_env(&settings, &backups, &serde_json::json!({"ANTHROPIC_BASE_URL":"https://example.test"})).unwrap();
        let updated: serde_json::Value = serde_json::from_slice(&fs::read(&settings).unwrap()).unwrap();
        assert_eq!(updated["env"]["ANTHROPIC_BASE_URL"], "https://example.test");
        assert_eq!(updated["permissions"]["allow"][0], "Bash");
        assert_eq!(fs::read_dir(backups).unwrap().count(), 1);
    }
}
