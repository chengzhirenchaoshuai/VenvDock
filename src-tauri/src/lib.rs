mod commands;
mod config;
mod runtime;

pub use runtime::{CTRL_C_HELPER_ARG, run_ctrl_c_helper};

use config::ConfigStore;
use runtime::Supervisor;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{
    AppHandle, Manager, WindowEvent,
    menu::{Menu, MenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
};
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
        eprintln!("设置开机自启失败：{e}");
    }
}

/// 显示并激活主窗口（同时处理最小化的情况）
fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// 退出程序：先优雅停止所有项目（在后台线程等待，不卡界面），再退出
fn quit_app(app: &AppHandle) {
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

/// 创建托盘图标：双击显示主窗口，右键菜单提供“显示”和“退出”
fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    TrayIconBuilder::with_id("main")
        .icon(
            app.default_window_icon()
                .cloned()
                .expect("缺少默认窗口图标"),
        )
        .tooltip("VenvDock")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main_window(app),
            "quit" => quit_app(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // 单实例插件必须最先注册：再次打开程序时只唤出已有窗口
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app);
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
            // 配置文件位于 %APPDATA%\com.venvdock.app\config.json
            let config_path = app.path().app_config_dir()?.join("config.json");
            let store = ConfigStore::load(config_path);
            let settings = store.settings();
            app.manage(store);
            sync_autostart(app.handle(), settings.launch_on_boot);
            // 默认日志目录：%APPDATA%\com.venvdock.app\logs
            let log_dir = app.path().app_data_dir()?.join("logs");
            let supervisor = Supervisor::new(app.handle().clone(), log_dir);
            app.manage(supervisor.clone());
            setup_tray(app.handle())?;
            supervisor.start_auto_projects();
            // 手动打开时总是显示窗口；开机自启时按设置决定，默认只在托盘
            let boot_launch = std::env::args().any(|a| a == AUTOSTART_ARG);
            if !boot_launch || settings.show_window_on_start {
                show_main_window(app.handle());
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_project,
            commands::delete_project,
            commands::update_settings,
            commands::reveal_config_file,
            commands::get_statuses,
            commands::reveal_log,
            commands::open_log,
            commands::close_log,
            commands::start_project,
            commands::stop_project,
            commands::restart_project,
        ])
        // 点击关闭按钮时只隐藏到托盘，真正退出走托盘菜单
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("启动 VenvDock 失败");
}
