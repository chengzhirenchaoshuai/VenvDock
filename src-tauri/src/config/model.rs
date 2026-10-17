use serde::{Deserialize, Serialize};
use std::path::Path;

/// 当前配置结构版本，结构变化需要迁移时递增
pub const CONFIG_VERSION: u32 = 1;

/// 项目的一个环境变量（用列表而不是映射，便于界面保持顺序）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EnvVar {
    pub key: String,
    pub value: String,
}

/// 一个被托管的项目；路径类字段为空字符串表示“未设置”
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Project {
    /// 稳定标识，创建时生成，改名不变
    pub id: String,
    pub name: String,
    pub command: String,
    /// 虚拟环境目录（可选），设置后等同于先激活该 venv
    pub venv_path: String,
    /// 工作目录（可选），为空时取 venv 的上级目录
    pub work_dir: String,
    /// 日志目录（可选），为空时取应用数据目录下的 logs
    pub log_dir: String,
    pub env: Vec<EnvVar>,
    /// 本工具启动时自动启动该项目
    pub auto_start: bool,
    /// 非手动停止时自动重启
    pub auto_restart: bool,
    /// 服务的网页地址（可选）；为空时从日志中自动识别
    pub url: String,
    /// 定时访问网址，服务起来后连续无响应则结束进程并重启
    pub health_check: bool,
}

/// 全局设置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// 开机自动启动本工具
    pub launch_on_boot: bool,
    /// 启动时显示主窗口（否则只在托盘）
    pub show_window_on_start: bool,
    /// 点击窗口关闭按钮时最小化到托盘；关闭此项则直接退出程序
    pub close_to_tray: bool,
    /// 优雅停止的等待秒数，超时后强制结束
    pub stop_timeout_secs: u32,
    /// 自动重启的初始等待秒数，之后按指数退避
    pub restart_delay_secs: u32,
    /// 自动重启等待的上限秒数
    pub restart_max_delay_secs: u32,
    /// 连续快速退出达到该次数后放弃重启，0 表示不限
    pub max_restart_attempts: u32,
    /// 单个日志文件的大小上限（MB），超过后轮转
    pub log_max_size_mb: u32,
    /// 每个项目保留的历史日志文件数
    pub log_keep_files: u32,
    /// 界面主题：default（默认）或 mist（晨雾）
    pub theme: String,
    /// 外观：system（跟随系统）、light 或 dark
    pub appearance: String,
    /// 项目失败（放弃重启或异常退出）时弹出系统通知
    pub notify_on_failure: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            launch_on_boot: true,
            show_window_on_start: false,
            close_to_tray: true,
            stop_timeout_secs: 10,
            restart_delay_secs: 3,
            restart_max_delay_secs: 60,
            max_restart_attempts: 5,
            log_max_size_mb: 10,
            log_keep_files: 5,
            theme: "default".into(),
            appearance: "system".into(),
            notify_on_failure: true,
        }
    }
}

impl Settings {
    /// 把各项数值限制在合理范围内（手动改坏配置时也能安全使用）
    pub fn normalized(mut self) -> Self {
        self.stop_timeout_secs = self.stop_timeout_secs.clamp(1, 120);
        self.restart_delay_secs = self.restart_delay_secs.clamp(1, 300);
        self.restart_max_delay_secs = self
            .restart_max_delay_secs
            .clamp(self.restart_delay_secs, 3600);
        self.max_restart_attempts = self.max_restart_attempts.min(100);
        self.log_max_size_mb = self.log_max_size_mb.clamp(1, 1024);
        self.log_keep_files = self.log_keep_files.clamp(1, 50);
        // 未知取值（手动改坏配置或旧版本）回退为默认
        if !["default", "mist"].contains(&self.theme.as_str()) {
            self.theme = "default".into();
        }
        if !["system", "light", "dark"].contains(&self.appearance.as_str()) {
            self.appearance = "system".into();
        }
        self
    }
}

/// 配置文件的完整内容
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub version: u32,
    pub settings: Settings,
    pub projects: Vec<Project>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            settings: Settings::default(),
            projects: Vec::new(),
        }
    }
}

/// 把项目名转换为合法的文件名
pub fn safe_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if "<>:\"/\\|?*".contains(c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim_matches(|c| c == ' ' || c == '.');
    if cleaned.is_empty() {
        "project".into()
    } else {
        cleaned.into()
    }
}

/// 规范化路径：去空白、统一为反斜杠、去掉末尾分隔符（保留盘符根目录如 C:\）
fn normalize_path(text: &str) -> String {
    let replaced = text.trim().replace('/', "\\");
    let trimmed = replaced.trim_end_matches('\\');
    if trimmed.len() == 2 && trimmed.ends_with(':') {
        format!("{trimmed}\\")
    } else {
        trimmed.to_string()
    }
}

impl Project {
    /// 规范化各字段：去空白、统一路径写法、去掉名称为空的环境变量
    pub fn normalize(&mut self) {
        self.name = self.name.trim().to_string();
        self.command = self.command.trim().to_string();
        self.venv_path = normalize_path(&self.venv_path);
        self.work_dir = normalize_path(&self.work_dir);
        self.log_dir = normalize_path(&self.log_dir);
        self.url = self.url.trim().to_string();
        for var in &mut self.env {
            var.key = var.key.trim().to_string();
        }
        self.env.retain(|var| !var.key.is_empty());
    }

    /// 规范化并完整校验（含路径是否存在）；`others` 为除自身外的其他项目，用于检查重名
    pub fn normalize_and_validate<'a>(
        &mut self,
        others: impl IntoIterator<Item = &'a Project>,
    ) -> Result<(), String> {
        self.normalize();
        self.validate(others)?;
        self.validate_paths()
    }

    /// 名称转换后用作日志文件名（如“a:b”和“a_b”都是 a_b.log），Windows 文件名不区分大小写，重名按此比较
    pub fn log_key(&self) -> String {
        safe_filename(&self.name).to_lowercase()
    }

    /// 与路径无关的校验。导入配置时只做这部分：换电脑后路径可能暂时不存在，启动时会再提示
    pub fn validate<'a>(&self, others: impl IntoIterator<Item = &'a Project>) -> Result<(), String> {
        if self.name.is_empty() {
            return Err("项目名称不能为空".into());
        }
        let key = self.log_key();
        if others.into_iter().any(|p| p.log_key() == key) {
            return Err(format!("已存在名为“{}”的项目", self.name));
        }
        if !self.url.is_empty()
            && !self.url.starts_with("http://")
            && !self.url.starts_with("https://")
        {
            return Err("网址需以 http:// 或 https:// 开头".into());
        }
        if self.command.is_empty() {
            return Err("启动命令不能为空".into());
        }
        for (i, var) in self.env.iter().enumerate() {
            if var.key.contains('=') {
                return Err(format!("环境变量名不能包含“=”：{}", var.key));
            }
            if self.env[..i]
                .iter()
                .any(|v| v.key.eq_ignore_ascii_case(&var.key))
            {
                return Err(format!("环境变量重复：{}", var.key));
            }
        }
        Ok(())
    }

    /// 已填写的 venv 和工作目录必须存在
    fn validate_paths(&self) -> Result<(), String> {
        if !self.venv_path.is_empty() && !Path::new(&self.venv_path).join("Scripts").is_dir() {
            return Err(format!(
                "venv 路径无效（缺少 Scripts 目录）：{}",
                self.venv_path
            ));
        }
        if !self.work_dir.is_empty() && !Path::new(&self.work_dir).is_dir() {
            return Err(format!("工作目录不存在：{}", self.work_dir));
        }
        Ok(())
    }
}
