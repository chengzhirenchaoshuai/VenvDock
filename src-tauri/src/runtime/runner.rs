//! 单个项目进程：启动、采集输出、优雅停止、强制结束
use super::logs::{LineSplitter, LogWriter, decode_line, safe_filename};
use super::win::{self, Job};
use crate::config::{Project, Settings};
use std::ffi::OsString;
use std::io::Read;
use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use windows_sys::Win32::System::Threading::{CREATE_NO_WINDOW, CREATE_SUSPENDED};

/// 启动一个进程所需的全部信息（已解析好默认值）
#[derive(Debug, Clone)]
pub struct LaunchSpec {
    pub command: String,
    pub venv_path: Option<PathBuf>,
    pub work_dir: PathBuf,
    pub log_file: PathBuf,
    pub env: Vec<(String, String)>,
    pub log_max_bytes: u64,
    pub log_keep: u32,
}

impl LaunchSpec {
    /// 由项目配置解析：工作目录默认为 venv 上级目录，再退到用户目录；日志默认放在 default_log_dir
    pub fn new(project: &Project, settings: &Settings, default_log_dir: &Path) -> Self {
        let venv_path = (!project.venv_path.is_empty()).then(|| PathBuf::from(&project.venv_path));
        let work_dir = if !project.work_dir.is_empty() {
            PathBuf::from(&project.work_dir)
        } else if let Some(parent) = venv_path.as_deref().and_then(Path::parent) {
            parent.to_path_buf()
        } else {
            std::env::var_os("USERPROFILE")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("C:\\"))
        };
        let log_dir = if project.log_dir.is_empty() {
            default_log_dir.to_path_buf()
        } else {
            PathBuf::from(&project.log_dir)
        };
        Self {
            command: project.command.clone(),
            venv_path,
            work_dir,
            log_file: log_dir.join(format!("{}.log", safe_filename(&project.name))),
            env: project
                .env
                .iter()
                .map(|v| (v.key.clone(), v.value.clone()))
                .collect(),
            log_max_bytes: settings.log_max_size_mb as u64 * 1024 * 1024,
            log_keep: settings.log_keep_files,
        }
    }

    /// 等同于激活 venv，并让 Python 子进程以 UTF-8、无缓冲方式输出；用户环境变量最后应用，可覆盖前面的值
    fn apply_env(&self, cmd: &mut Command) -> Result<(), String> {
        cmd.env_remove("PYTHONHOME");
        if let Some(venv) = &self.venv_path {
            let mut paths = vec![venv.join("Scripts")];
            paths.extend(std::env::split_paths(
                &std::env::var_os("PATH").unwrap_or_default(),
            ));
            let path = std::env::join_paths(paths).map_err(|e| format!("PATH 无效：{e}"))?;
            cmd.env("VIRTUAL_ENV", venv).env("PATH", path);
        }
        cmd.env("PYTHONUTF8", "1")
            .env("PYTHONIOENCODING", "utf-8")
            .env("PYTHONUNBUFFERED", "1");
        for (key, value) in &self.env {
            cmd.env(key, value);
        }
        Ok(())
    }
}

/// 进程退出标志，供“优雅停止”等待使用
#[derive(Default)]
struct ExitSignal {
    exited: Mutex<bool>,
    cv: Condvar,
}

impl ExitSignal {
    fn set(&self) {
        *self.exited.lock().unwrap_or_else(|e| e.into_inner()) = true;
        self.cv.notify_all();
    }

    /// 等待进程退出，返回是否在超时前退出
    fn wait_timeout(&self, timeout: Duration) -> bool {
        let guard = self.exited.lock().unwrap_or_else(|e| e.into_inner());
        let (guard, _) = self
            .cv
            .wait_timeout_while(guard, timeout, |exited| !*exited)
            .unwrap_or_else(|e| e.into_inner());
        *guard
    }
}

/// 每输出一行日志时的回调（用于实时日志查看）
pub type LineCallback = Arc<dyn Fn(&str) + Send + Sync>;

/// 进程输出去向：写入日志文件，同时交给回调
struct Output {
    file: Mutex<LogWriter>,
    on_line: LineCallback,
}

impl Output {
    fn write(&self, line: &str) {
        self.file
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .write_line(line);
        (self.on_line)(line);
    }
}

/// 持续读取一个输出管道，按行写入日志，直到管道关闭
fn pump(mut source: impl Read + Send + 'static, out: Arc<Output>) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut splitter = LineSplitter::default();
        let mut write = |line: &[u8]| out.write(&decode_line(line));
        let mut buf = [0u8; 8192];
        while let Ok(n) = source.read(&mut buf) {
            if n == 0 {
                break;
            }
            splitter.push(&buf[..n], &mut write);
        }
        splitter.finish(&mut write);
    })
}

/// 一个正在运行的项目进程（通过 cmd /c 启动，整个进程树都在同一个 Job 中）
pub struct RunningProcess {
    pid: u32,
    job: Arc<Job>,
    exit: Arc<ExitSignal>,
}

impl RunningProcess {
    /// 启动进程；每行输出调用 on_line；进程退出且输出全部写完后，在后台线程中调用 on_exit(退出码)
    pub fn spawn(
        spec: &LaunchSpec,
        on_line: LineCallback,
        on_exit: impl FnOnce(Option<i32>) + Send + 'static,
    ) -> Result<Self, String> {
        if let Some(venv) = &spec.venv_path
            && !venv.join("Scripts").is_dir()
        {
            return Err(format!(
                "venv 无效（缺少 Scripts 目录）：{}",
                venv.display()
            ));
        }
        if !spec.work_dir.is_dir() {
            return Err(format!("工作目录不存在：{}", spec.work_dir.display()));
        }

        let log = LogWriter::open(spec.log_file.clone(), spec.log_max_bytes, spec.log_keep)?;
        let out = Arc::new(Output {
            file: Mutex::new(log),
            on_line,
        });
        out.write(&format!(
            "========== 启动于 {} ==========",
            win::local_time_string()
        ));
        out.write(&format!("命令：{}", spec.command));
        out.write(&format!("工作目录：{}", spec.work_dir.display()));
        if let Some(venv) = &spec.venv_path {
            out.write(&format!("虚拟环境：{}", venv.display()));
        }

        win::enable_ctrl_c_for_children();
        let job = Arc::new(Job::new()?);
        let shell = std::env::var_os("ComSpec").unwrap_or_else(|| OsString::from("cmd.exe"));
        let mut cmd = Command::new(shell);
        // /d 不执行 AutoRun，/s 配合外层引号原样保留命令内容
        cmd.raw_arg(format!("/d /s /c \"{}\"", spec.command))
            .current_dir(&spec.work_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // 先挂起创建，加入 Job 后再恢复，确保不会有子孙进程漏在 Job 之外
            .creation_flags(CREATE_NO_WINDOW | CREATE_SUSPENDED);
        spec.apply_env(&mut cmd)?;

        let mut child = cmd.spawn().map_err(|e| {
            let msg = format!("启动进程失败：{e}");
            out.write(&msg);
            msg
        })?;
        let pid = child.id();
        if let Err(e) = job
            .assign(child.as_raw_handle() as _)
            .and_then(|_| win::resume_process(pid))
        {
            let _ = child.kill();
            let _ = child.wait();
            out.write(&e);
            return Err(e);
        }

        let readers: Vec<_> = [
            child.stdout.take().map(|s| pump(s, out.clone())),
            child.stderr.take().map(|s| pump(s, out.clone())),
        ]
        .into_iter()
        .flatten()
        .collect();

        let exit = Arc::new(ExitSignal::default());
        let (job2, exit2) = (job.clone(), exit.clone());
        thread::spawn(move || {
            let code = child.wait().ok().and_then(|s| s.code());
            // 主进程已退出：结束残留的子孙进程，管道随之关闭，读取线程才能结束
            job2.terminate();
            for reader in readers {
                let _ = reader.join();
            }
            let code_text = code.map_or("未知".to_string(), |c| c.to_string());
            out.write(&format!(
                "========== 结束于 {}（退出码 {code_text}）==========",
                win::local_time_string()
            ));
            exit2.set();
            on_exit(code);
        });

        Ok(Self { pid, job, exit })
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// 优雅停止：先发 Ctrl+C，超时仍未退出则强制结束；在后台执行，不阻塞调用方
    pub fn stop(&self, timeout: Duration) {
        let (pid, job, exit) = (self.pid, self.job.clone(), self.exit.clone());
        thread::spawn(move || {
            if win::send_ctrl_c(pid).is_ok() && exit.wait_timeout(timeout) {
                return;
            }
            job.terminate();
        });
    }

    /// 立即结束整个进程树（目前仅测试使用）
    #[allow(dead_code)]
    pub fn kill(&self) {
        self.job.terminate();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Instant;

    /// 等待日志中出现指定文本
    fn wait_for(path: &Path, text: &str) -> String {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let content = std::fs::read_to_string(path).unwrap_or_default();
            if content.contains(text) || Instant::now() > deadline {
                return content;
            }
            thread::sleep(Duration::from_millis(100));
        }
    }

    fn pid_alive(pid: &str) -> bool {
        let out = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).contains(pid)
    }

    #[test]
    fn 优雅停止_编码_进程树清理_强制结束() {
        let dir = std::env::temp_dir().join(format!("venvdock-runner-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // 脚本：启动一个孙进程并打印其 PID，然后等待 Ctrl+C
        std::fs::write(
            dir.join("t.py"),
            "import subprocess, time\n\
             p = subprocess.Popen(['ping', '-n', '100', '127.0.0.1'], stdout=subprocess.DEVNULL)\n\
             print('中文输出 child=%d' % p.pid)\n\
             print('ready')\n\
             try:\n    time.sleep(60)\nexcept KeyboardInterrupt:\n    print('graceful')\n",
        )
        .unwrap();
        let spec = LaunchSpec {
            command: "echo 你好 & python t.py".into(),
            venv_path: None,
            work_dir: dir.clone(),
            log_file: dir.join("t.log"),
            env: vec![],
            log_max_bytes: 1024 * 1024,
            log_keep: 2,
        };

        // 1. 优雅停止（直接调用助手逻辑，测试进程不是 venvdock.exe，无法走助手子进程）
        let (tx, rx) = mpsc::channel();
        let proc =
            RunningProcess::spawn(&spec, Arc::new(|_| {}), move |code| tx.send(code).unwrap())
                .unwrap();
        let log = wait_for(&spec.log_file, "ready");
        let child_pid = log
            .split("child=")
            .nth(1)
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .trim()
            .to_string();
        assert_eq!(win::run_ctrl_c_helper(proc.pid()), 0);
        rx.recv_timeout(Duration::from_secs(10))
            .expect("Ctrl+C 后进程未退出");
        let log = std::fs::read_to_string(&spec.log_file).unwrap();
        assert!(log.contains("你好"), "cmd 的 GBK 输出解码失败：{log}");
        assert!(
            log.contains("中文输出"),
            "Python 的 UTF-8 输出解码失败：{log}"
        );
        assert!(log.contains("graceful"), "未优雅退出：{log}");
        assert!(!pid_alive(&child_pid), "孙进程未被清理");

        // 2. 强制结束
        let (tx, rx) = mpsc::channel();
        let proc =
            RunningProcess::spawn(&spec, Arc::new(|_| {}), move |code| tx.send(code).unwrap())
                .unwrap();
        wait_for(&spec.log_file, "ready");
        proc.kill();
        rx.recv_timeout(Duration::from_secs(5))
            .expect("强制结束后未收到退出通知");

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
