//! 程序自身的运行日志：发布版没有控制台，错误和崩溃信息写到这里
use crate::runtime::local_time_string;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

/// 启动时日志超过该大小，先改名为 venvdock.old.log 再重新开始
const MAX_BYTES: u64 = 1024 * 1024;

static FILE: OnceLock<Mutex<File>> = OnceLock::new();

/// 打开日志文件并安装 panic 钩子；只在启动时调用一次
pub fn init(path: &Path) {
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    if fs::metadata(path).is_ok_and(|m| m.len() > MAX_BYTES) {
        let _ = fs::rename(path, path.with_extension("old.log"));
    }
    if let Ok(file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = FILE.set(Mutex::new(file));
    }
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        write(&format!("程序出错：{info}"));
        default_hook(info);
    }));
}

/// 写一条日志；同时输出到控制台，开发版可直接看到
pub fn write(msg: &str) {
    eprintln!("{msg}");
    if let Some(file) = FILE.get() {
        let mut file = file.lock().unwrap_or_else(|e| e.into_inner());
        let _ = writeln!(file, "[{}] {msg}", local_time_string());
    }
}
