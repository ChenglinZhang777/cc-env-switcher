use claude_env_switcher_lib::{claude_settings, providers::{self, ProviderProfile}};
use std::{collections::BTreeMap, fs, path::PathBuf};
use tauri::{Manager, menu::{Menu, MenuItem}, tray::TrayIconBuilder};
use tauri_plugin_opener::OpenerExt;

struct AppState { providers_path: PathBuf, backups_path: PathBuf, settings_path: PathBuf }

fn app_state(app: &tauri::App) -> Result<AppState, Box<dyn std::error::Error>> {
    let data = app.path().app_config_dir()?;
    Ok(AppState {
        providers_path: data.join("providers.json"),
        backups_path: data.join("backups"),
        settings_path: std::env::var_os("HOME").map(PathBuf::from).ok_or("未找到用户目录")?.join(".claude/settings.json"),
    })
}

#[tauri::command]
fn list_providers(state: tauri::State<AppState>) -> Result<Vec<ProviderProfile>, String> { providers::load_profiles(&state.providers_path) }

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

#[tauri::command]
fn switch_provider(state: tauri::State<AppState>, id: String) -> Result<(), String> {
    let profile = providers::load_profiles(&state.providers_path)?.into_iter().find(|profile| profile.id == id).ok_or("未找到该供应商")?;
    claude_settings::switch_env(&state.settings_path, &state.backups_path, &serde_json::to_value(profile.env).map_err(|error| error.to_string())?).map_err(|error| error.to_string())
}

#[tauri::command]
fn backups_path(state: tauri::State<AppState>) -> String { state.backups_path.display().to_string() }

#[tauri::command]
fn open_backups_directory(app: tauri::AppHandle, state: tauri::State<AppState>) -> Result<(), String> {
    fs::create_dir_all(&state.backups_path).map_err(|error| error.to_string())?;
    app.opener().open_path(state.backups_path.display().to_string(), None::<&str>).map_err(|error| error.to_string())
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let state = app_state(app)?;
            let open = MenuItem::with_id(app, "open", "打开 Claude Env Switcher", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;
            TrayIconBuilder::new().menu(&menu).tooltip("Claude Env Switcher").on_menu_event(|app, event| match event.id.as_ref() {
                "open" => { if let Some(window) = app.get_webview_window("main") { let _ = window.show(); let _ = window.set_focus(); } }
                "quit" => app.exit(0),
                _ => {}
            }).build(app)?;
            app.manage(state);
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![list_providers, save_provider, delete_provider, import_current_env, switch_provider, backups_path, open_backups_directory])
        .run(tauri::generate_context!())
        .expect("启动 Claude Env Switcher 失败");
}
