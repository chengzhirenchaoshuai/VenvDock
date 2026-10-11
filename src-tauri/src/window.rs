//! 主窗口：按需创建；关闭后先隐藏，一段时间内未再打开则销毁窗口和 WebView 以释放内存
use crate::config::ConfigStore;
use crate::paths::AppPaths;
use crate::runtime::Supervisor;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Manager, Theme, WebviewWindowBuilder, Window};

const MAIN: &str = "main";

/// 隐藏后多久销毁 WebView；期间再次打开可瞬间恢复
const DESTROY_AFTER: Duration = Duration::from_secs(30);

/// 每次显示或隐藏都递增，用于作废之前排定的销毁
static HIDE_SEQ: AtomicU64 = AtomicU64::new(0);

/// 显示并激活主窗口；窗口已被销毁时重新创建
pub fn show(app: &AppHandle) {
    HIDE_SEQ.fetch_add(1, Ordering::SeqCst);
    if let Some(window) = app.get_webview_window(MAIN) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    if let Err(e) = create(app) {
        eprintln!("创建主窗口失败：{e}");
    }
}

/// 按 tauri.conf.json 中的窗口配置创建主窗口
fn create(app: &AppHandle) -> Result<(), String> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == MAIN)
        .cloned()
        .ok_or("缺少主窗口配置")?;
    let mut builder = WebviewWindowBuilder::from_config(app, &config)
        .map_err(|e| e.to_string())?
        .visible(true)
        .theme(native_theme(app));
    if let Some(dir) = &app.state::<AppPaths>().webview_dir {
        builder = builder.data_directory(dir.clone());
    }
    let window = builder.build().map_err(|e| e.to_string())?;
    let _ = window.set_focus();
    Ok(())
}

/// 创建窗口时标题栏的深浅色，避免首帧闪浅色；之后由前端 applyTheme 同步
/// “跟随系统”返回 None，交给系统决定
fn native_theme(app: &AppHandle) -> Option<Theme> {
    match app.state::<ConfigStore>().settings().appearance.as_str() {
        "light" => Some(Theme::Light),
        "dark" => Some(Theme::Dark),
        _ => None,
    }
}

/// 点击关闭按钮：隐藏到托盘，DESTROY_AFTER 后仍未显示则销毁
pub fn hide(window: &Window) {
    let _ = window.hide();
    let seq = HIDE_SEQ.fetch_add(1, Ordering::SeqCst) + 1;
    let app = window.app_handle().clone();
    thread::spawn(move || {
        thread::sleep(DESTROY_AFTER);
        if HIDE_SEQ.load(Ordering::SeqCst) != seq {
            return;
        }
        if let Some(window) = app.get_webview_window(MAIN)
            && !window.is_visible().unwrap_or(true)
        {
            // 日志查看器随窗口一起消失，停止向它推送
            app.state::<Supervisor>().close_log();
            let _ = window.destroy();
        }
    });
}
