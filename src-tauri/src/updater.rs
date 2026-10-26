//! 自动更新：下载 GitHub Release 中的新版本并校验 minisign 签名（打包时用私钥签名，程序里只有公钥），
//! 由临时目录中的助手进程在本程序退出后替换 exe（免安装版）或运行安装程序（安装版），再启动新版本
use crate::applog;
use crate::paths::AppPaths;
use crate::runtime::{Supervisor, local_time_string};
use serde::Deserialize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
use windows_sys::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
use windows_sys::Win32::System::Com::Urlmon::URLDownloadToFileW;
use windows_sys::Win32::System::Threading::{
    CREATE_NO_WINDOW, OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject,
};

/// 助手模式参数：venvdock.exe --apply-update <旧进程 pid> <setup|portable> <下载的文件> <目标 exe>
pub const UPDATE_HELPER_ARG: &str = "--apply-update";

/// 更新失败时助手重新打开原有版本带的参数，程序据此提示用户
pub const UPDATE_FAILED_ARG: &str = "--update-failed";

/// 只接受本项目 Release 的下载地址
const DOWNLOAD_PREFIX: &str = "https://github.com/chengzhirenchaoshuai/VenvDock/releases/download/";

/// 更新签名公钥（minisign，对应私钥 ProjectKey\VenvDock\venvdock.key）
const PUBLIC_KEY: &str = "RWRjknjGnKPKpR3u6b0HGMljWvm67uuGNuNLh7noxx1mP5rI8z9Cub2T";

/// 助手等待旧程序退出的最长时间（旧程序要先优雅停止所有项目）
const EXIT_WAIT: Duration = Duration::from_secs(180);

/// 前端从 GitHub API 取得的发布文件信息
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseAsset {
    pub name: String,
    pub url: String,
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

/// 安装版的程序目录里有卸载程序，免安装版没有
fn is_installed(exe: &Path) -> bool {
    exe.with_file_name("uninstall.exe").is_file()
}

fn download(url: &str, dest: &Path) -> Result<(), String> {
    let (url_w, dest_w) = (wide(url), wide(&dest.to_string_lossy()));
    // URLDownloadToFileW 需要 COM；本线程已初始化时返回 S_FALSE，不影响使用
    let hr = unsafe {
        CoInitializeEx(std::ptr::null(), COINIT_MULTITHREADED as u32);
        URLDownloadToFileW(
            std::ptr::null_mut(),
            url_w.as_ptr(),
            dest_w.as_ptr(),
            0,
            std::ptr::null_mut(),
        )
    };
    if hr != 0 {
        return Err(format!("下载失败（错误码 0x{:08X}）", hr as u32));
    }
    Ok(())
}

/// 校验签名：`sig` 是 `tauri signer sign` 生成的 .sig 文件内容（minisign 签名的 base64），
/// 可信注释里的文件名必须与期望的一致，防止用旧版本的合法安装包冒充新版本
fn verify(data: &[u8], sig: &[u8], name: &str) -> Result<(), String> {
    use base64::Engine;
    let invalid = || "更新文件的签名无效，已取消更新".to_string();
    let text = base64::engine::general_purpose::STANDARD
        .decode(sig.trim_ascii())
        .map_err(|_| invalid())?;
    let text = String::from_utf8(text).map_err(|_| invalid())?;
    let signature = minisign_verify::Signature::decode(&text).map_err(|_| invalid())?;
    let key = minisign_verify::PublicKey::from_base64(PUBLIC_KEY).map_err(|_| invalid())?;
    key.verify(data, &signature, false).map_err(|_| invalid())?;
    let file_ok = signature
        .trusted_comment()
        .split('\t')
        .any(|part| part.strip_prefix("file:") == Some(name));
    if !file_ok {
        return Err(invalid());
    }
    Ok(())
}

/// 准备更新：下载并校验新版本、记录正在运行的项目、启动助手。成功后调用方应退出程序
pub fn prepare(app: &AppHandle, version: &str, assets: &[ReleaseAsset]) -> Result<(), String> {
    if version.is_empty() || !version.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return Err("版本号无效".into());
    }
    let exe = std::env::current_exe().map_err(|e| format!("无法获取程序路径：{e}"))?;
    let kind = if is_installed(&exe) { "setup" } else { "portable" };
    let name = format!("VenvDock_{version}_x64-{kind}.exe");
    let find = |wanted: &str| {
        let asset = assets
            .iter()
            .find(|a| a.name == wanted)
            .ok_or_else(|| format!("发布中缺少 {wanted}，无法自动更新"))?;
        if !asset.url.starts_with(DOWNLOAD_PREFIX) {
            return Err("下载地址不是本项目的发布页，已取消更新".to_string());
        }
        Ok(asset.url.as_str())
    };
    let sig_name = format!("{name}.sig");
    let (url, sig_url) = (find(&name)?, find(&sig_name)?);

    let dir = std::env::temp_dir().join("venvdock-update");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).map_err(|e| format!("创建临时目录失败：{e}"))?;
    let (file, sig_file) = (dir.join(&name), dir.join(&sig_name));
    download(url, &file)?;
    download(sig_url, &sig_file)?;
    let data = fs::read(&file).map_err(|e| format!("读取下载的文件失败：{e}"))?;
    let sig = fs::read(&sig_file).map_err(|e| format!("读取签名文件失败：{e}"))?;
    if let Err(e) = verify(&data, &sig, &name) {
        let _ = fs::remove_file(&file);
        return Err(e);
    }

    // 助手从临时目录运行：安装版的程序文件要被安装程序覆盖，免安装版的 exe 要被替换
    let helper = dir.join("venvdock-updater.exe");
    fs::copy(&exe, &helper).map_err(|e| format!("准备更新助手失败：{e}"))?;

    // 记录正在运行的项目，新版本启动后恢复
    let resume_file = app.state::<AppPaths>().resume_file.clone();
    let running = app.state::<Supervisor>().active_ids();
    fs::write(&resume_file, serde_json::to_vec(&running).unwrap_or_default())
        .map_err(|e| format!("记录运行中的项目失败：{e}"))?;

    let spawned = Command::new(&helper)
        .arg(UPDATE_HELPER_ARG)
        .arg(std::process::id().to_string())
        .arg(kind)
        .arg(&file)
        .arg(&exe)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
    if let Err(e) = spawned {
        let _ = fs::remove_file(&resume_file);
        return Err(format!("启动更新助手失败：{e}"));
    }
    applog::write(&format!("开始更新到 {version}（{kind}）"));
    Ok(())
}

/// 读取并删除更新前记录的运行中项目
pub fn take_resume(path: &Path) -> Vec<String> {
    let ids = fs::read(path)
        .ok()
        .and_then(|data| serde_json::from_slice(&data).ok())
        .unwrap_or_default();
    let _ = fs::remove_file(path);
    ids
}

/// 等待进程退出；进程不存在时立即返回 true
fn wait_exit(pid: u32, timeout: Duration) -> bool {
    unsafe {
        let handle = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
        if handle.is_null() {
            return true;
        }
        let result = WaitForSingleObject(handle, timeout.as_millis() as u32);
        CloseHandle(handle);
        result == WAIT_OBJECT_0
    }
}

/// 用新文件替换程序：先复制到程序旁的 .new，再改名覆盖（同一磁盘上是原子操作），
/// 中途失败时原程序不受影响。旧进程刚退出时文件可能还被占用，重试一段时间
fn replace_exe(file: &Path, target: &Path) -> bool {
    let mut new = target.as_os_str().to_owned();
    new.push(".new");
    let new = PathBuf::from(new);
    let ok = fs::copy(file, &new).is_ok()
        && (0..40).any(|_| {
            let renamed = fs::rename(&new, target).is_ok();
            if !renamed {
                thread::sleep(Duration::from_millis(500));
            }
            renamed
        });
    let _ = fs::remove_file(&new);
    ok
}

/// 助手进程：等待旧程序退出，替换 exe 或运行安装程序，然后启动程序。过程记录在下载目录的 update.log
pub fn run_helper(pid: u32, kind: &str, file: &Path, target: &Path) -> i32 {
    let log_path: PathBuf = file.with_file_name("update.log");
    let note = |msg: &str| {
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&log_path) {
            let _ = writeln!(f, "[{}] {msg}", local_time_string());
        }
    };
    if !wait_exit(pid, EXIT_WAIT) {
        note("等待旧程序退出超时，仍继续更新");
    }
    let ok = match kind {
        "portable" => replace_exe(file, target),
        // /P 只显示进度条无需操作，/UPDATE 不重新创建快捷方式
        "setup" => Command::new(file)
            .args(["/P", "/UPDATE"])
            .status()
            .is_ok_and(|s| s.success()),
        _ => false,
    };
    note(if ok {
        "更新完成，启动新版本"
    } else {
        "更新失败，启动原有版本"
    });
    let mut relaunch = Command::new(target);
    if !ok {
        relaunch.arg(UPDATE_FAILED_ARG);
    }
    if let Err(e) = relaunch.spawn() {
        note(&format!("启动程序失败：{e}"));
    }
    if ok { 0 } else { 1 }
}
