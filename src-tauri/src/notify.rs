//! 系统通知：服务失败、自动更新失败时弹出 Windows 通知，点击后打开主窗口
use crate::applog;
use crate::config::ConfigStore;
use tauri::{AppHandle, Manager};
use tauri_winrt_notification::Toast;

/// 通知来源标识。开发版没有注册，借用 PowerShell 的标识才能显示
fn app_id(app: &AppHandle) -> String {
    if cfg!(debug_assertions) {
        Toast::POWERSHELL_APP_ID.to_string()
    } else {
        app.config().identifier.clone()
    }
}

/// 在注册表登记通知来源的显示名称。
/// 安装版的开始菜单快捷方式已带有该标识；免安装版没有快捷方式，靠这里让系统认出“VenvDock”
pub fn register(app: &AppHandle) {
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, REG_SZ, RegSetKeyValueW};
    if cfg!(debug_assertions) {
        return;
    }
    let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let key = wide(&format!(
        "Software\\Classes\\AppUserModelId\\{}",
        app_id(app)
    ));
    let name = wide("DisplayName");
    let value = wide(&app.package_info().name);
    let code = unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            name.as_ptr(),
            REG_SZ,
            value.as_ptr().cast(),
            (value.len() * 2) as u32,
        )
    };
    if code != 0 {
        applog::write(&format!("登记通知来源失败（错误码 {code}）"));
    }
}

/// 项目失败时发通知（受“失败时通知”设置控制）；在后台线程发送，不阻塞调用方
pub fn project_failed(app: &AppHandle, project_name: &str, message: &str) {
    if !app.state::<ConfigStore>().settings().notify_on_failure {
        return;
    }
    show(app, format!("{project_name} 已停止运行"), message.to_string());
}

/// 自动更新失败、助手重新打开了原有版本时提示（不受设置控制）
pub fn update_failed(app: &AppHandle) {
    let log = std::env::temp_dir().join("venvdock-update").join("update.log");
    show(
        app,
        "VenvDock 更新失败".into(),
        format!(
            "仍在使用 v{}。可能是程序所在文件夹不能写入，详情见 {}",
            app.package_info().version,
            log.display()
        ),
    );
}

/// 在后台线程发送通知，不阻塞调用方
fn show(app: &AppHandle, title: String, body: String) {
    let app = app.clone();
    std::thread::spawn(move || {
        let handle = app.clone();
        let result = Toast::new(&app_id(&app))
            .title(&title)
            .text1(&body)
            .on_activated(move |_| {
                crate::window::show(&handle);
                Ok(())
            })
            .show();
        if let Err(e) = result {
            applog::write(&format!("发送通知失败：{e}"));
        }
    });
}
