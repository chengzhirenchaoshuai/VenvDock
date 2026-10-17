//! 托盘：双击显示主窗口；右键菜单按项目列出子菜单（打开网页、启停、重启），以及全部启停、显示主窗口和退出
use crate::config::ConfigStore;
use crate::runtime::{Phase, ProjectStatus, STATUS_EVENT, Supervisor};
use crate::window;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tauri::{
    AppHandle, Listener, Manager, Wry,
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_opener::OpenerExt;

const TRAY_ID: &str = "main";

/// 项目配置变化（新增、修改、删除）时发出的事件，用于刷新托盘菜单
pub const CONFIG_EVENT: &str = "config-changed";

/// 项目菜单项的 id 形如 “动作:项目 id”
const OPEN: &str = "open";
const START: &str = "start";
const STOP: &str = "stop";
const RESTART: &str = "restart";

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
            "start-all" => app.state::<Supervisor>().start_all(),
            "stop-all" => app.state::<Supervisor>().stop_all(),
            id => {
                let Some((action, pid)) = id.split_once(':') else {
                    return;
                };
                let supervisor = app.state::<Supervisor>();
                match action {
                    OPEN => {
                        if let Some(url) = project_url(app, pid) {
                            let _ = app.opener().open_url(url, None::<&str>);
                        }
                    }
                    // 失败原因会显示在项目状态里
                    START => drop(supervisor.start(pid)),
                    STOP => supervisor.stop(pid),
                    RESTART => drop(supervisor.restart(pid)),
                    _ => {}
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
    running_url(&project.url, &status)
}

fn running_url(configured: &str, status: &ProjectStatus) -> Option<String> {
    if status.phase != Phase::Running {
        return None;
    }
    if configured.is_empty() {
        status.detected_url.clone()
    } else {
        Some(configured.to_string())
    }
}

fn phase_text(phase: Phase) -> &'static str {
    match phase {
        Phase::Stopped => "已停止",
        Phase::Running => "运行中",
        Phase::Stopping => "停止中",
        Phase::Backoff => "等待重启",
        Phase::Failed => "失败",
    }
}

fn item(app: &AppHandle, id: String, text: &str, enabled: bool) -> tauri::Result<MenuItem<Wry>> {
    MenuItem::with_id(app, id, text, enabled, None::<&str>)
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let menu = Menu::new(app)?;
    let projects = app.state::<ConfigStore>().snapshot().projects;
    let statuses = app.state::<Supervisor>().statuses();
    for project in &projects {
        let id = &project.id;
        // 两份快照分别获取，按 id 对应，避免中途配置变化时错位
        let Some(status) = statuses.iter().find(|s| &s.id == id) else {
            continue;
        };
        // 等待重启时也可以“停止”，用于取消重启
        let active = matches!(status.phase, Phase::Running | Phase::Stopping | Phase::Backoff);
        let url = running_url(&project.url, status);
        let sub = Submenu::with_items(
            app,
            format!("{}（{}）", project.name, phase_text(status.phase)),
            true,
            &[
                &item(app, format!("{OPEN}:{id}"), "打开网页", url.is_some())?,
                &PredefinedMenuItem::separator(app)?,
                &if active {
                    item(app, format!("{STOP}:{id}"), "停止", status.phase != Phase::Stopping)?
                } else {
                    item(app, format!("{START}:{id}"), "启动", true)?
                },
                &item(app, format!("{RESTART}:{id}"), "重启", status.phase == Phase::Running)?,
            ],
        )?;
        menu.append(&sub)?;
    }
    if !projects.is_empty() {
        menu.append(&PredefinedMenuItem::separator(app)?)?;
        menu.append(&item(app, "start-all".into(), "全部启动", true)?)?;
        menu.append(&item(app, "stop-all".into(), "全部停止", true)?)?;
        menu.append(&PredefinedMenuItem::separator(app)?)?;
    }
    menu.append(&item(app, "show".into(), "显示主窗口", true)?)?;
    menu.append(&item(app, "quit".into(), "退出", true)?)?;
    Ok(menu)
}
