//! 托盘：双击显示主窗口；右键菜单列出运行中服务的“打开网页”，以及显示主窗口和退出
use crate::config::ConfigStore;
use crate::runtime::{Phase, STATUS_EVENT, Supervisor};
use crate::window;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tauri::{
    AppHandle, Listener, Manager, Wry,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_opener::OpenerExt;

const TRAY_ID: &str = "main";

/// 项目配置变化（新增、修改、删除）时发出的事件，用于刷新托盘菜单
pub const CONFIG_EVENT: &str = "config-changed";

/// 菜单项 id 前缀：open:<项目 id>
const OPEN_PREFIX: &str = "open:";

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(
            app.default_window_icon()
                .cloned()
                .expect("缺少默认窗口图标"),
        )
        .tooltip("VenvDock")
        .menu(&build_menu(app)?)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => window::show(app),
            "quit" => crate::quit_app(app),
            id => {
                if let Some(url) = id
                    .strip_prefix(OPEN_PREFIX)
                    .and_then(|pid| project_url(app, pid))
                {
                    let _ = app.opener().open_url(url, None::<&str>);
                }
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } = event
            {
                window::show(tray.app_handle());
            }
        })
        .build(app)?;

    // 状态或配置变化时在后台线程重建菜单：事件可能在持有监管器锁时发出，不能就地重建
    let (tx, rx) = mpsc::channel::<()>();
    for event in [STATUS_EVENT, CONFIG_EVENT] {
        let tx = tx.clone();
        app.listen_any(event, move |_| {
            let _ = tx.send(());
        });
    }
    let app = app.clone();
    thread::spawn(move || {
        while rx.recv().is_ok() {
            // 合并短时间内的多次变化
            thread::sleep(Duration::from_millis(200));
            while rx.try_recv().is_ok() {}
            if let (Some(tray), Ok(menu)) = (app.tray_by_id(TRAY_ID), build_menu(&app)) {
                let _ = tray.set_menu(Some(menu));
            }
        }
    });
    Ok(())
}

/// 运行中项目的网页地址：优先用配置的网址，否则用从日志识别到的
fn project_url(app: &AppHandle, id: &str) -> Option<String> {
    let project = app.state::<ConfigStore>().project(id)?;
    let status = app
        .state::<Supervisor>()
        .statuses()
        .into_iter()
        .find(|s| s.id == id)?;
    if status.phase != Phase::Running {
        return None;
    }
    if project.url.is_empty() {
        status.detected_url
    } else {
        Some(project.url)
    }
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let menu = Menu::new(app)?;
    let mut has_links = false;
    for project in app.state::<ConfigStore>().snapshot().projects {
        if project_url(app, &project.id).is_some() {
            let id = format!("{OPEN_PREFIX}{}", project.id);
            menu.append(&MenuItem::with_id(
                app,
                id,
                format!("打开 {}", project.name),
                true,
                None::<&str>,
            )?)?;
            has_links = true;
        }
    }
    if has_links {
        menu.append(&PredefinedMenuItem::separator(app)?)?;
    }
    menu.append(&MenuItem::with_id(
        app,
        "show",
        "显示主窗口",
        true,
        None::<&str>,
    )?)?;
    menu.append(&MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?)?;
    Ok(menu)
}
