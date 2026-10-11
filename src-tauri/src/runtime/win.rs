//! Win32 API 的薄封装：Job Object、恢复挂起进程、发送 Ctrl+C、编码转换、本地时间
use std::os::windows::process::CommandExt;
use std::process::Command;
use std::time::Duration;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE, SYSTEMTIME};
use windows_sys::Win32::Globalization::{CP_OEMCP, MultiByteToWideChar};
use windows_sys::Win32::System::Console::{
    AttachConsole, CTRL_C_EVENT, FreeConsole, GenerateConsoleCtrlEvent, SetConsoleCtrlHandler,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject,
};
use windows_sys::Win32::System::SystemInformation::GetLocalTime;
use windows_sys::Win32::System::Threading::{
    DETACHED_PROCESS, OpenThread, ResumeThread, THREAD_SUSPEND_RESUME,
};

/// 命令行参数：以“发送 Ctrl+C 助手”模式运行本程序
pub const CTRL_C_HELPER_ARG: &str = "--send-ctrl-c";

/// Job Object：句柄关闭时（包括本程序崩溃）系统会结束其中所有进程
pub struct Job(HANDLE);

// 内核句柄可以跨线程使用
unsafe impl Send for Job {}
unsafe impl Sync for Job {}

impl Job {
    pub fn new() -> Result<Self, String> {
        unsafe {
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return Err(format!(
                    "创建 Job Object 失败：{}",
                    std::io::Error::last_os_error()
                ));
            }
            let job = Job(handle);
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let ok = SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            if ok == 0 {
                return Err(format!(
                    "设置 Job Object 失败：{}",
                    std::io::Error::last_os_error()
                ));
            }
            Ok(job)
        }
    }

    /// 把进程加入 Job；之后它创建的子孙进程也自动属于该 Job
    pub fn assign(&self, process: HANDLE) -> Result<(), String> {
        if unsafe { AssignProcessToJobObject(self.0, process) } == 0 {
            return Err(format!(
                "进程加入 Job Object 失败：{}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(())
    }

    /// 立即结束 Job 中的所有进程
    pub fn terminate(&self) {
        unsafe { TerminateJobObject(self.0, 1) };
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

/// 恢复以 CREATE_SUSPENDED 方式创建的进程（标准库不暴露主线程句柄，只能枚举线程）
pub fn resume_process(pid: u32) -> Result<(), String> {
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(format!("枚举线程失败：{}", std::io::Error::last_os_error()));
        }
        let mut entry: THREADENTRY32 = std::mem::zeroed();
        entry.dwSize = size_of::<THREADENTRY32>() as u32;
        let mut resumed = false;
        let mut ok = Thread32First(snapshot, &mut entry);
        while ok != 0 {
            if entry.th32OwnerProcessID == pid {
                let thread = OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID);
                if !thread.is_null() {
                    resumed |= ResumeThread(thread) != u32::MAX;
                    CloseHandle(thread);
                }
            }
            ok = Thread32Next(snapshot, &mut entry);
        }
        CloseHandle(snapshot);
        if resumed {
            Ok(())
        } else {
            Err("恢复进程主线程失败".into())
        }
    }
}

/// 向目标进程所在控制台发送 Ctrl+C，让服务优雅退出。
/// 本程序可能本身挂着控制台（开发模式），所以交给一个无控制台的助手子进程去做，
/// 避免改动本进程的控制台状态。
pub fn send_ctrl_c(pid: u32) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut helper = Command::new(exe)
        .args([CTRL_C_HELPER_ARG, &pid.to_string()])
        .creation_flags(DETACHED_PROCESS)
        .spawn()
        .map_err(|e| format!("启动 Ctrl+C 助手失败：{e}"))?;
    // 助手只做几次系统调用，正常瞬间完成；等待上限防止异常卡住
    for _ in 0..50 {
        match helper.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(_)) => return Err("Ctrl+C 助手执行失败".into()),
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(e.to_string()),
        }
    }
    let _ = helper.kill();
    Err("Ctrl+C 助手超时".into())
}

/// 恢复本进程对 Ctrl+C 的默认处理。该“忽略 Ctrl+C”标志会被子进程继承，
/// 若本进程是被设置了该标志的父进程启动的，子进程将收不到 Ctrl+C，无法优雅停止
pub fn enable_ctrl_c_for_children() {
    unsafe { SetConsoleCtrlHandler(None, 0) };
}

/// 助手模式入口：附加到目标控制台，向其中所有进程发送 Ctrl+C，返回进程退出码
pub fn run_ctrl_c_helper(pid: u32) -> i32 {
    unsafe {
        FreeConsole();
        if AttachConsole(pid) == 0 {
            return 1;
        }
        // 助手自己也在这个控制台里，先忽略 Ctrl+C 以免被一起结束
        SetConsoleCtrlHandler(None, 1);
        let ok = GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0);
        FreeConsole();
        if ok == 0 { 1 } else { 0 }
    }
}

/// 用系统 OEM 代码页（中文系统为 GBK）解码，用于 cmd 等非 UTF-8 输出
pub fn decode_oem(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    unsafe {
        let len = MultiByteToWideChar(
            CP_OEMCP,
            0,
            bytes.as_ptr(),
            bytes.len() as i32,
            std::ptr::null_mut(),
            0,
        );
        if len <= 0 {
            return String::from_utf8_lossy(bytes).into_owned();
        }
        let mut wide = vec![0u16; len as usize];
        MultiByteToWideChar(
            CP_OEMCP,
            0,
            bytes.as_ptr(),
            bytes.len() as i32,
            wide.as_mut_ptr(),
            len,
        );
        String::from_utf16_lossy(&wide)
    }
}

/// 本地时间，格式 2026-10-11 12:34:56
pub fn local_time_string() -> String {
    let mut t: SYSTEMTIME = unsafe { std::mem::zeroed() };
    unsafe { GetLocalTime(&mut t) };
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond
    )
}
