//! 暴露给前端调用的命令
use crate::config::{Config, ConfigStore, Project, Settings};
use crate::runtime::{ProjectStatus, Supervisor};
use crate::tray::CONFIG_EVENT;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
pub fn get_config(store: State<'_, ConfigStore>) -> Config {
    store.snapshot()
}

/// 新建或更新项目；id 为空表示新建。运行中的项目修改后在下次启动时生效
#[tauri::command]
pub fn save_project(
    app: AppHandle,
    store: State<'_, ConfigStore>,
    project: Project,
) -> Result<Project, String> {
    let saved = store.upsert_project(project)?;
    let _ = app.emit(CONFIG_EVENT, ());
    Ok(saved)
}

/// 删除项目：先停止其进程，再从配置中移除
#[tauri::command]
pub fn delete_project(
    app: AppHandle,
    store: State<'_, ConfigStore>,
    supervisor: State<'_, Supervisor>,
    id: String,
) -> Result<(), String> {
    supervisor.remove(&id);
    store.delete_project(&id)?;
    let _ = app.emit(CONFIG_EVENT, ());
    Ok(())
}

#[tauri::command]
pub fn update_settings(
    app: AppHandle,
    store: State<'_, ConfigStore>,
    settings: Settings,
) -> Result<Settings, String> {
    let saved = store.update_settings(settings)?;
    crate::sync_autostart(&app, saved.launch_on_boot);
    Ok(saved)
}

/// 在资源管理器中定位配置文件
#[tauri::command]
pub fn reveal_config_file(app: AppHandle, store: State<'_, ConfigStore>) -> Result<(), String> {
    let path = store.path();
    if !path.exists() {
        // 还未保存过配置时，先写出一份默认配置，便于用户查看
        store.update_settings(store.snapshot().settings)?;
    }
    app.opener()
        .reveal_item_in_dir(path)
        .map_err(|e| e.to_string())
}

/// 在资源管理器中定位项目日志；日志还不存在时打开其所在目录
#[tauri::command]
pub fn reveal_log(
    app: AppHandle,
    supervisor: State<'_, Supervisor>,
    id: String,
) -> Result<(), String> {
    let path = supervisor.log_file(&id).ok_or("项目不存在")?;
    if path.exists() {
        return app
            .opener()
            .reveal_item_in_dir(&path)
            .map_err(|e| e.to_string());
    }
    let dir = path.parent().ok_or("日志路径无效")?;
    std::fs::create_dir_all(dir).map_err(|e| format!("创建日志目录失败：{e}"))?;
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// 开始实时查看日志，返回最近的若干行
#[tauri::command]
pub fn open_log(supervisor: State<'_, Supervisor>, id: String) -> Result<Vec<String>, String> {
    supervisor.open_log(&id, 1000)
}

#[tauri::command]
pub fn close_log(supervisor: State<'_, Supervisor>) {
    supervisor.close_log();
}

#[tauri::command]
pub fn get_statuses(supervisor: State<'_, Supervisor>) -> Vec<ProjectStatus> {
    supervisor.statuses()
}

#[tauri::command]
pub fn start_project(supervisor: State<'_, Supervisor>, id: String) -> Result<(), String> {
    supervisor.start(&id)
}

#[tauri::command]
pub fn stop_project(supervisor: State<'_, Supervisor>, id: String) {
    supervisor.stop(&id);
}

#[tauri::command]
pub fn restart_project(supervisor: State<'_, Supervisor>, id: String) -> Result<(), String> {
    supervisor.restart(&id)
}
