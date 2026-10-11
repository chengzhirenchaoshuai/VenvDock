//! 数据文件位置：安装版放在 %APPDATA%，便携版放在程序旁的 data 目录
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

pub struct AppPaths {
    pub config_file: PathBuf,
    pub log_dir: PathBuf,
    /// WebView2 数据目录；None 表示使用默认位置（%LOCALAPPDATA%\com.venvdock.app）
    pub webview_dir: Option<PathBuf>,
}

impl AppPaths {
    /// 程序所在目录存在 data 子目录时进入便携模式，所有数据都写在其中
    pub fn resolve(app: &AppHandle) -> tauri::Result<Self> {
        let portable_dir = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.join("data")))
            .filter(|dir| dir.is_dir());
        if let Some(dir) = portable_dir {
            return Ok(Self {
                config_file: dir.join("config.json"),
                log_dir: dir.join("logs"),
                webview_dir: Some(dir.join("webview")),
            });
        }
        Ok(Self {
            config_file: app.path().app_config_dir()?.join("config.json"),
            log_dir: app.path().app_data_dir()?.join("logs"),
            webview_dir: None,
        })
    }
}
