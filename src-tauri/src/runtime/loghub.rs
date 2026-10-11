//! 实时日志推送：只转发界面正在查看的那个项目的输出，按批次发给前端
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

/// 推送给前端的事件名
pub const LOG_EVENT: &str = "project-log";

/// 两次推送之间的最短间隔，把高频输出合并成一批
const BATCH_INTERVAL: Duration = Duration::from_millis(100);

/// 待推送行数上限，输出过快时丢弃最旧的行，保护界面
const MAX_PENDING: usize = 5000;

#[derive(Serialize, Clone)]
struct LogBatch {
    id: String,
    lines: Vec<String>,
}

#[derive(Default)]
struct State {
    watched: Option<String>,
    pending: VecDeque<String>,
}

#[derive(Default)]
struct Inner {
    state: Mutex<State>,
    cv: Condvar,
}

#[derive(Clone)]
pub struct LogHub(Arc<Inner>);

impl LogHub {
    /// 创建并启动后台推送线程
    pub fn new(app: AppHandle) -> Self {
        let hub = Self(Arc::new(Inner::default()));
        let inner = hub.0.clone();
        thread::spawn(move || {
            loop {
                // 等到有待推送的行
                {
                    let guard = inner.state.lock().unwrap_or_else(|e| e.into_inner());
                    let _guard = inner
                        .cv
                        .wait_while(guard, |s| s.pending.is_empty())
                        .unwrap_or_else(|e| e.into_inner());
                }
                // 稍等片刻，让同一时间段的输出合并成一批
                thread::sleep(BATCH_INTERVAL);
                let batch = {
                    let mut s = inner.state.lock().unwrap_or_else(|e| e.into_inner());
                    let lines: Vec<String> = std::mem::take(&mut s.pending).into();
                    // 等待期间可能切换或关闭了查看，此时这批行已被清空或已无人查看
                    match s.watched.clone() {
                        Some(id) if !lines.is_empty() => LogBatch { id, lines },
                        _ => continue,
                    }
                };
                let _ = app.emit(LOG_EVENT, batch);
            }
        });
        hub
    }

    /// 设置正在查看的项目（None 表示关闭查看），并丢弃尚未推送的旧行
    pub fn watch(&self, id: Option<String>) {
        let mut s = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        s.watched = id;
        s.pending.clear();
    }

    /// 某项目输出了一行；只有正在查看的项目才会被推送
    pub fn push(&self, id: &str, line: &str) {
        let mut s = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        if s.watched.as_deref() != Some(id) {
            return;
        }
        if s.pending.len() >= MAX_PENDING {
            s.pending.pop_front();
        }
        s.pending.push_back(line.to_string());
        self.0.cv.notify_one();
    }
}
