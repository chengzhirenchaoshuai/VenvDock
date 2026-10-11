//! 监管器：维护每个项目的运行状态，处理启动、停止、重启以及崩溃后的退避重启
use super::loghub::LogHub;
use super::logs::read_tail;
use super::{LaunchSpec, RunningProcess};
use crate::config::ConfigStore;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

/// 运行超过该时长后退出，视为“稳定运行过”，重启计数从头开始
const STABLE_RUN: Duration = Duration::from_secs(30);

/// 状态变化时推送给前端的事件名
pub const STATUS_EVENT: &str = "project-status";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Stopped,
    Running,
    Stopping,
    /// 崩溃后等待自动重启
    Backoff,
    /// 启动失败、异常退出或已放弃自动重启
    Failed,
}

/// 推送给前端的项目运行状态（时间为 Unix 毫秒时间戳）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectStatus {
    pub id: String,
    pub phase: Phase,
    pub pid: Option<u32>,
    pub started_at: Option<u64>,
    pub last_exit_code: Option<i32>,
    /// 连续自动重启次数
    pub restart_count: u32,
    pub next_restart_at: Option<u64>,
    pub message: Option<String>,
}

impl ProjectStatus {
    fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            phase: Phase::Stopped,
            pid: None,
            started_at: None,
            last_exit_code: None,
            restart_count: 0,
            next_restart_at: None,
            message: None,
        }
    }
}

struct Slot {
    status: ProjectStatus,
    process: Option<RunningProcess>,
    /// 每次启动或取消待执行的重启都递增，用于识别过期的退出回调和重启定时
    generation: u64,
    started: Option<Instant>,
    stop_requested: bool,
    restart_after_stop: bool,
}

impl Slot {
    fn new(id: &str) -> Self {
        Self {
            status: ProjectStatus::new(id),
            process: None,
            generation: 0,
            started: None,
            stop_requested: false,
            restart_after_stop: false,
        }
    }
}

struct Shared {
    app: AppHandle,
    log_dir: PathBuf,
    slots: Mutex<HashMap<String, Slot>>,
    hub: LogHub,
}

/// 可克隆的监管器句柄（退出回调和重启定时线程中也要使用）
#[derive(Clone)]
pub struct Supervisor(Arc<Shared>);

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

fn exit_code_text(code: Option<i32>) -> String {
    code.map_or("未知".into(), |c| c.to_string())
}

impl Supervisor {
    pub fn new(app: AppHandle, log_dir: PathBuf) -> Self {
        let hub = LogHub::new(app.clone());
        Self(Arc::new(Shared {
            app,
            log_dir,
            slots: Mutex::new(HashMap::new()),
            hub,
        }))
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, Slot>> {
        self.0.slots.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn store(&self) -> tauri::State<'_, ConfigStore> {
        self.0.app.state::<ConfigStore>()
    }

    fn emit(&self, status: &ProjectStatus) {
        let _ = self.0.app.emit(STATUS_EVENT, status);
    }

    /// 所有项目的当前状态，顺序与配置一致
    pub fn statuses(&self) -> Vec<ProjectStatus> {
        let projects = self.store().snapshot().projects;
        let slots = self.lock();
        projects
            .iter()
            .map(|p| {
                slots
                    .get(&p.id)
                    .map_or_else(|| ProjectStatus::new(&p.id), |s| s.status.clone())
            })
            .collect()
    }

    /// 项目当前的日志文件路径
    pub fn log_file(&self, id: &str) -> Option<PathBuf> {
        let store = self.store();
        let project = store.project(id)?;
        Some(LaunchSpec::new(&project, &store.settings(), &self.0.log_dir).log_file)
    }

    /// 开始实时查看某项目的日志：返回日志文件最后 max_lines 行，之后的新输出通过事件推送
    pub fn open_log(&self, id: &str, max_lines: usize) -> Result<Vec<String>, String> {
        let path = self.log_file(id).ok_or("项目不存在")?;
        self.0.hub.watch(Some(id.to_string()));
        Ok(read_tail(&path, max_lines))
    }

    /// 关闭实时日志查看
    pub fn close_log(&self) {
        self.0.hub.watch(None);
    }

    /// 启动所有勾选了“自动启动”的项目
    pub fn start_auto_projects(&self) {
        for project in self
            .store()
            .snapshot()
            .projects
            .iter()
            .filter(|p| p.auto_start)
        {
            // 失败信息已记录在该项目的状态中
            let _ = self.start(&project.id);
        }
    }

    /// 手动启动：清零重启计数；正在停止时等停完再启动
    pub fn start(&self, id: &str) -> Result<(), String> {
        let mut slots = self.lock();
        let slot = slots.entry(id.to_string()).or_insert_with(|| Slot::new(id));
        match slot.status.phase {
            Phase::Running => Ok(()),
            Phase::Stopping => {
                slot.restart_after_stop = true;
                Ok(())
            }
            _ => {
                slot.status.restart_count = 0;
                self.launch(slot)
            }
        }
    }

    /// 停止：运行中则优雅停止；等待重启或失败状态则直接回到已停止
    pub fn stop(&self, id: &str) {
        let timeout = Duration::from_secs(self.store().settings().stop_timeout_secs.into());
        let mut slots = self.lock();
        let Some(slot) = slots.get_mut(id) else {
            return;
        };
        slot.restart_after_stop = false;
        match slot.status.phase {
            Phase::Running => self.begin_stop(slot, timeout),
            Phase::Backoff | Phase::Failed => {
                slot.generation += 1;
                slot.status.phase = Phase::Stopped;
                slot.status.next_restart_at = None;
                slot.status.message = None;
                self.emit(&slot.status);
            }
            Phase::Stopped | Phase::Stopping => {}
        }
    }

    /// 重启：运行中则先停止，退出后自动再启动
    pub fn restart(&self, id: &str) -> Result<(), String> {
        let timeout = Duration::from_secs(self.store().settings().stop_timeout_secs.into());
        {
            let mut slots = self.lock();
            if let Some(slot) = slots.get_mut(id) {
                match slot.status.phase {
                    Phase::Running => {
                        slot.restart_after_stop = true;
                        self.begin_stop(slot, timeout);
                        return Ok(());
                    }
                    Phase::Stopping => {
                        slot.restart_after_stop = true;
                        return Ok(());
                    }
                    _ => {}
                }
            }
        }
        self.start(id)
    }

    /// 项目被删除：在后台停止其进程并移除状态，之后的退出回调会被忽略
    pub fn remove(&self, id: &str) {
        let timeout = Duration::from_secs(self.store().settings().stop_timeout_secs.into());
        if let Some(slot) = self.lock().remove(id)
            && let Some(process) = &slot.process
        {
            process.stop(timeout);
        }
    }

    /// 退出程序前调用：停止所有项目并等待其退出（阻塞，最长为停止超时再加 2 秒）
    pub fn shutdown(&self) {
        let timeout = Duration::from_secs(self.store().settings().stop_timeout_secs.into());
        {
            let mut slots = self.lock();
            for slot in slots.values_mut() {
                slot.restart_after_stop = false;
                match slot.status.phase {
                    Phase::Running => self.begin_stop(slot, timeout),
                    Phase::Backoff => slot.generation += 1,
                    _ => {}
                }
            }
        }
        let deadline = Instant::now() + timeout + Duration::from_secs(2);
        while Instant::now() < deadline && self.lock().values().any(|s| s.process.is_some()) {
            thread::sleep(Duration::from_millis(100));
        }
    }

    fn begin_stop(&self, slot: &mut Slot, timeout: Duration) {
        slot.stop_requested = true;
        slot.status.phase = Phase::Stopping;
        if let Some(process) = &slot.process {
            process.stop(timeout);
        }
        self.emit(&slot.status);
    }

    /// 持锁时调用：按最新配置启动进程并更新状态
    fn launch(&self, slot: &mut Slot) -> Result<(), String> {
        let id = slot.status.id.clone();
        slot.stop_requested = false;
        slot.restart_after_stop = false;
        slot.status.next_restart_at = None;
        slot.generation += 1;

        let store = self.store();
        let result = match store.project(&id) {
            None => Err("项目不存在，可能已被删除".to_string()),
            Some(project) => {
                let spec = LaunchSpec::new(&project, &store.settings(), &self.0.log_dir);
                let (sup, generation, exit_id) = (self.clone(), slot.generation, id.clone());
                let (hub, line_id) = (self.0.hub.clone(), id.clone());
                RunningProcess::spawn(
                    &spec,
                    Arc::new(move |line: &str| hub.push(&line_id, line)),
                    move |code| sup.on_exit(&exit_id, generation, code),
                )
            }
        };

        let result = match result {
            Ok(process) => {
                slot.status.phase = Phase::Running;
                slot.status.pid = Some(process.pid());
                slot.status.started_at = Some(now_ms());
                slot.status.message = None;
                slot.started = Some(Instant::now());
                slot.process = Some(process);
                Ok(())
            }
            Err(e) => {
                slot.status.phase = Phase::Failed;
                slot.status.message = Some(e.clone());
                Err(e)
            }
        };
        self.emit(&slot.status);
        result
    }

    /// 进程退出回调（在后台线程中调用）
    fn on_exit(&self, id: &str, generation: u64, code: Option<i32>) {
        let mut slots = self.lock();
        let Some(slot) = slots.get_mut(id) else {
            return;
        };
        if slot.generation != generation {
            return;
        }
        slot.process = None;
        slot.status.pid = None;
        slot.status.started_at = None;
        slot.status.last_exit_code = code;
        let ran = slot.started.take().map(|t| t.elapsed()).unwrap_or_default();

        // 手动停止或重启
        if slot.stop_requested {
            if slot.restart_after_stop {
                slot.status.restart_count = 0;
                let _ = self.launch(slot);
                return;
            }
            slot.stop_requested = false;
            slot.status.phase = Phase::Stopped;
            slot.status.message = None;
            self.emit(&slot.status);
            return;
        }

        // 意外退出：未开启自动重启时停在“已停止/失败”
        let store = self.store();
        let settings = store.settings();
        let code_text = exit_code_text(code);
        if !store.project(id).is_some_and(|p| p.auto_restart) {
            if code == Some(0) {
                slot.status.phase = Phase::Stopped;
                slot.status.message = None;
            } else {
                slot.status.phase = Phase::Failed;
                slot.status.message = Some(format!("进程异常退出（退出码 {code_text}）"));
            }
            self.emit(&slot.status);
            return;
        }

        // 自动重启：稳定运行过则重新计数，否则累加，按指数退避等待
        let count = if ran >= STABLE_RUN {
            1
        } else {
            slot.status.restart_count + 1
        };
        slot.status.restart_count = count;
        if settings.max_restart_attempts > 0 && count > settings.max_restart_attempts {
            slot.status.phase = Phase::Failed;
            slot.status.message = Some(format!(
                "连续 {} 次启动后很快退出（退出码 {code_text}），已停止自动重启，请查看日志",
                settings.max_restart_attempts
            ));
            self.emit(&slot.status);
            return;
        }
        let delay = u64::from(settings.restart_delay_secs)
            .saturating_mul(1 << (count - 1).min(16))
            .min(settings.restart_max_delay_secs.into());
        slot.generation += 1;
        slot.status.phase = Phase::Backoff;
        slot.status.next_restart_at = Some(now_ms() + delay * 1000);
        slot.status.message = Some(format!(
            "进程退出（退出码 {code_text}），即将第 {count} 次自动重启"
        ));
        self.emit(&slot.status);

        let (sup, id, generation) = (self.clone(), id.to_string(), slot.generation);
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(delay));
            sup.on_backoff_elapsed(&id, generation);
        });
    }

    fn on_backoff_elapsed(&self, id: &str, generation: u64) {
        let mut slots = self.lock();
        let Some(slot) = slots.get_mut(id) else {
            return;
        };
        // 等待期间被手动启动、停止或删除，则取消本次重启
        if slot.generation == generation && slot.status.phase == Phase::Backoff {
            let _ = self.launch(slot);
        }
    }
}
