//! 主窗口：按需创建；关闭后先隐藏，一段时间内未再打开则销毁窗口和 WebView 以释放内存
use crate::applog;
use crate::config::ConfigStore;
use crate::paths::AppPaths;
use crate::runtime::Supervisor;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Manager, Theme, WebviewWindowBuilder, Window};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_opener::OpenerExt;

const WEBVIEW2_URL: &str = "https://developer.microsoft.com/microsoft-edge/webview2/";

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
        applog::write(&format!("创建主窗口失败：{e}"));
        report_create_failure(app, &e);
    }
}

/// 窗口创建失败多半是缺少 WebView2 运行时；弹出系统对话框说明，不然用户只看到一个托盘图标
fn report_create_failure(app: &AppHandle, error: &str) {
    let opener = app.clone();
    app.dialog()
        .message(format!(
            "无法打开主窗口：{error}

通常是系统缺少 Microsoft Edge WebView2 运行时，安装后重新打开 VenvDock 即可。"
        ))
        .title("VenvDock")
        .kind(MessageDialogKind::Error)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "打开下载页面".into(),
            "关闭".into(),
        ))
        .show(move |ok| {
            if ok {
                let _ = opener.opener().open_url(WEBVIEW2_URL, None::<&str>);
            }
        });
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
