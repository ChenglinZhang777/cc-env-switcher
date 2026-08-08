use cc_env_switcher_lib::{claude_settings, providers::{self, ProviderProfile}};
use std::{collections::BTreeMap, fs, path::PathBuf};
use tauri::{Manager, menu::{Menu, MenuItem}, tray::TrayIconBuilder};
use tauri_plugin_opener::OpenerExt;

struct AppState { providers_path: PathBuf, backups_path: PathBuf, settings_path: PathBuf }

fn app_state(app: &tauri::App) -> Result<AppState, Box<dyn std::error::Error>> {
    let data = app.path().app_config_dir()?;

    // 数据迁移：从旧 bundle ID 路径迁移到新路径（仅首次）
    let old_data = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or("未找到用户目录")?
        .join("Library/Application Support/com.chenglinzhang.claude-env-switcher");

    if old_data.exists() && !data.exists() {
        eprintln!("检测到旧版本数据，开始迁移...");
        fs::create_dir_all(&data)?;

        // 迁移 providers.json
        let old_providers = old_data.join("providers.json");
        if old_providers.exists() {
            fs::copy(&old_providers, data.join("providers.json"))?;
            eprintln!("已迁移 providers.json");
        }

        // 迁移 backups/ 目录
        let old_backups = old_data.join("backups");
        if old_backups.exists() {
            let new_backups = data.join("backups");
            fs::create_dir_all(&new_backups)?;
            for entry in fs::read_dir(&old_backups)? {
                let entry = entry?;
                if entry.file_type()?.is_file() {
                    fs::copy(entry.path(), new_backups.join(entry.file_name()))?;
                }
            }
            eprintln!("已迁移 {} 个备份文件", fs::read_dir(&new_backups)?.count());
        }

        eprintln!("数据迁移完成！旧数据保留在: {}", old_data.display());
    }

    Ok(AppState {
        providers_path: data.join("providers.json"),
        backups_path: data.join("backups"),
        settings_path: std::env::var_os("HOME").map(PathBuf::from).ok_or("未找到用户目录")?.join(".claude/settings.json"),
    })
}

#[tauri::command]
fn list_providers(state: tauri::State<AppState>) -> Result<Vec<ProviderProfile>, String> {
    let profiles = providers::load_profiles(&state.providers_path)?;
    if !profiles.is_empty() { return Ok(profiles); }
    // 列表为空时预置一个官方方案；用户删除后不会被反复塞回，除非再次归零。
    let seeded = vec![providers::native_profile()];
    providers::save_profiles(&state.providers_path, &seeded)?;
    Ok(seeded)
}

#[tauri::command]
fn save_provider(state: tauri::State<AppState>, profile: ProviderProfile) -> Result<(), String> {
    let mut profiles = providers::load_profiles(&state.providers_path)?;
    if let Some(index) = profiles.iter().position(|item| item.id == profile.id) { profiles[index] = profile; } else { profiles.push(profile); }
    providers::save_profiles(&state.providers_path, &profiles)
}

#[tauri::command]
fn delete_provider(state: tauri::State<AppState>, id: String) -> Result<(), String> {
    let mut profiles = providers::load_profiles(&state.providers_path)?;
    profiles.retain(|profile| profile.id != id);
    providers::save_profiles(&state.providers_path, &profiles)
}

#[tauri::command]
fn import_current_env(state: tauri::State<AppState>) -> Result<BTreeMap<String, String>, String> {
    let bytes = std::fs::read(&state.settings_path).map_err(|error| error.to_string())?;
    let document: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| "settings.json 不是有效 JSON".to_string())?;
    serde_json::from_value(document.get("env").cloned().unwrap_or_else(|| serde_json::json!({}))).map_err(|_| "env 必须是字符串键值对".to_string())
}

/// 读取当前生效的环境变量。配置文件缺失或损坏时返回 None，让界面照常渲染。
#[tauri::command]
fn read_active_env(state: tauri::State<AppState>) -> Result<Option<BTreeMap<String, String>>, String> {
    let Ok(bytes) = std::fs::read(&state.settings_path) else { return Ok(None) };
    let Ok(document) = serde_json::from_slice::<serde_json::Value>(&bytes) else { return Ok(None) };
    let Some(env) = document.get("env") else { return Ok(Some(BTreeMap::new())) };
    Ok(serde_json::from_value(env.clone()).ok())
}

#[tauri::command]
fn switch_provider(state: tauri::State<AppState>, id: String) -> Result<(), String> {
    let profile = providers::load_profiles(&state.providers_path)?.into_iter().find(|profile| profile.id == id).ok_or("未找到该供应商")?;
    claude_settings::switch_env(&state.settings_path, &state.backups_path, &serde_json::to_value(profile.env).map_err(|error| error.to_string())?).map_err(|error| error.to_string())
}

#[tauri::command]
async fn test_connection(input: cc_env_switcher_lib::connection_test::ConnectionTestInput) -> Result<cc_env_switcher_lib::connection_test::ConnectionTestResult, String> {
    cc_env_switcher_lib::connection_test::test(input).await
}

#[tauri::command]
fn backups_path(state: tauri::State<AppState>) -> String { state.backups_path.display().to_string() }

#[tauri::command]
fn open_backups_directory(app: tauri::AppHandle, state: tauri::State<AppState>) -> Result<(), String> {
    fs::create_dir_all(&state.backups_path).map_err(|error| error.to_string())?;
    app.opener().open_path(state.backups_path.display().to_string(), None::<&str>).map_err(|error| error.to_string())
}

use cc_env_switcher_lib::profile_package;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};

#[derive(serde::Serialize)]
struct ImportSummary { imported: usize, renamed: usize }

/// 导出前的明文提醒。用户点取消返回 false，调用方须中止导出。
fn confirm_plaintext_export(app: &tauri::AppHandle) -> bool {
    app.dialog()
        .message("导出的配置包含 API Key 明文。文件或剪贴板内容离开本机后，取得它的人即可使用你的全部 Key。")
        .title("确认导出？")
        .buttons(MessageDialogButtons::OkCancelCustom("继续导出".into(), "取消".into()))
        .blocking_show()
}

#[tauri::command]
async fn export_profiles_to_file(app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let text = profile_package::pack(&providers::load_profiles(&state.providers_path)?)?;
    if !confirm_plaintext_export(&app) { return Ok(false); }
    let stamp = chrono::Local::now().format("%Y%m%d");
    let Some(target) = app.dialog().file()
        .set_title("导出全部方案")
        .set_file_name(format!("cc-env-switcher-profiles-{stamp}.json"))
        .add_filter("JSON", &["json"])
        .blocking_save_file()
    else { return Ok(false) };
    let path = target.into_path().map_err(|error| error.to_string())?;
    fs::write(path, text).map_err(|error| format!("写入文件失败：{error}"))?;
    Ok(true)
}

#[tauri::command]
async fn export_profiles_to_clipboard(app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let text = profile_package::pack(&providers::load_profiles(&state.providers_path)?)?;
    if !confirm_plaintext_export(&app) { return Ok(false); }
    app.clipboard().write_text(text).map_err(|error| format!("写入剪贴板失败：{error}"))?;
    Ok(true)
}

/// 解析后合并落盘。抽出来是因为文件和剪贴板两条路只有取文本的方式不同。
fn apply_import(state: &AppState, text: &str) -> Result<ImportSummary, String> {
    let incoming = profile_package::parse(text)?;
    if incoming.is_empty() {
        return Err("这份配置里没有方案；请确认原机器上已保存方案后重新导出。".to_string());
    }
    let outcome = profile_package::merge(&providers::load_profiles(&state.providers_path)?, incoming);
    providers::save_profiles(&state.providers_path, &outcome.profiles)?;
    Ok(ImportSummary { imported: outcome.imported, renamed: outcome.renamed })
}

#[tauri::command]
async fn import_profiles_from_file(app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> Result<Option<ImportSummary>, String> {
    let Some(source) = app.dialog().file()
        .set_title("导入方案")
        .add_filter("JSON", &["json"])
        .blocking_pick_file()
    else { return Ok(None) };
    let path = source.into_path().map_err(|error| error.to_string())?;
    let text = fs::read_to_string(path).map_err(|error| format!("读取文件失败：{error}"))?;
    apply_import(&state, &text).map(Some)
}

#[tauri::command]
async fn import_profiles_from_clipboard(app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> Result<ImportSummary, String> {
    let text = app.clipboard().read_text()
        .map_err(|_| "剪贴板里没有可读文本；请先在原机器上点导出到剪贴板。".to_string())?;
    apply_import(&state, &text)
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let state = app_state(app)?;
            let open = MenuItem::with_id(app, "open", "打开 CC Env Switcher", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;
            TrayIconBuilder::new().menu(&menu).tooltip("CC Env Switcher").on_menu_event(|app, event| match event.id.as_ref() {
                "open" => { if let Some(window) = app.get_webview_window("main") { let _ = window.show(); let _ = window.set_focus(); } }
                "quit" => app.exit(0),
                _ => {}
            }).build(app)?;
            app.manage(state);
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![list_providers, save_provider, delete_provider, import_current_env, read_active_env, switch_provider, test_connection, backups_path, open_backups_directory, export_profiles_to_file, export_profiles_to_clipboard, import_profiles_from_file, import_profiles_from_clipboard])
        .run(tauri::generate_context!())
        .expect("启动 CC Env Switcher 失败");
}
