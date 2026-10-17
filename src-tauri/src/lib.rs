mod applog;
mod commands;
mod config;
mod notify;
mod paths;
mod runtime;
mod tray;
mod window;

pub use runtime::{CTRL_C_HELPER_ARG, run_ctrl_c_helper};

use config::ConfigStore;
use paths::AppPaths;
use runtime::Supervisor;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Manager, RunEvent, WindowEvent};
use tauri_plugin_autostart::ManagerExt;

/// 开机自启时附带的命令行参数，用于区分“开机启动”和“手动打开”
const AUTOSTART_ARG: &str = "--autostart";

/// 让注册表中的开机自启项与设置保持一致。
/// 开发版不写入，避免把 target\debug 下的程序注册为开机启动
pub(crate) fn sync_autostart(app: &AppHandle, enabled: bool) {
    if cfg!(debug_assertions) {
        return;
    }
    let launcher = app.autolaunch();
    let result = match (enabled, launcher.is_enabled().unwrap_or(false)) {
        (true, _) => launcher.enable(), // 总是重写一次，程序位置变化后也能更新路径
        (false, true) => launcher.disable(),
        (false, false) => Ok(()),
    };
    if let Err(e) = result {
        applog::write(&format!("设置开机自启失败：{e}"));
    }
}

/// 退出程序：先优雅停止所有项目（在后台线程等待，不卡界面），再退出
pub(crate) fn quit_app(app: &AppHandle) {
    static QUITTING: AtomicBool = AtomicBool::new(false);
    if QUITTING.swap(true, Ordering::SeqCst) {
        return;
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    let app = app.clone();
    std::thread::spawn(move || {
        app.state::<Supervisor>().shutdown();
        app.exit(0);
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // 单实例插件必须最先注册：再次打开程序时只唤出已有窗口
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            window::show(app);
        }))
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name("VenvDock")
                .arg(AUTOSTART_ARG)
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let paths = AppPaths::resolve(app.handle())?;
            applog::init(&paths.app_log);
            applog::write(&format!("VenvDock {} 启动", app.package_info().version));
            let store = ConfigStore::load(paths.config_file.clone());
            let settings = store.settings();
            let supervisor = Supervisor::new(app.handle().clone(), paths.log_dir.clone());
            app.manage(paths);
            app.manage(store);
            sync_autostart(app.handle(), settings.launch_on_boot);
            app.manage(supervisor.clone());
            notify::register(app.handle());
            tray::setup(app.handle())?;
            supervisor.start_auto_projects();
            // 手动打开时总是显示窗口；开机自启时按设置决定，默认只在托盘（不创建 WebView）
            let boot_launch = std::env::args().any(|a| a == AUTOSTART_ARG);
            if !boot_launch || settings.show_window_on_start {
                window::show(app.handle());
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_project,
            commands::delete_project,
            commands::update_settings,
            commands::reveal_config_file,
            commands::export_config,
            commands::import_config,
            commands::get_statuses,
            commands::reveal_log,
            commands::open_log,
            commands::close_log,
            commands::start_project,
            commands::stop_project,
            commands::restart_project,
            commands::start_all,
            commands::stop_all,
        ])
        // 点击关闭按钮时只隐藏到托盘，真正退出走托盘菜单
        .on_window_event(|win, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                window::hide(win);
            }
        })
        .build(tauri::generate_context!())
        .expect("启动 VenvDock 失败")
        .run(|_app, event| {
            // 主窗口被销毁时不退出程序；只有托盘“退出”（app.exit 带退出码）才真正退出
            if let RunEvent::ExitRequested {
                code: None, api, ..
            } = event
            {
                api.prevent_exit();
            }
        });
}
