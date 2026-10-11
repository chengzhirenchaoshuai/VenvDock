use super::model::{CONFIG_VERSION, Config, Project, Settings};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

/// 配置存储：内存中保存一份配置，每次修改先落盘成功再更新内存
pub struct ConfigStore {
    path: PathBuf,
    config: Mutex<Config>,
}

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// 先写临时文件再替换，避免写入中途出错导致配置损坏
fn write_atomic(path: &Path, config: &Config) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("创建配置目录失败：{e}"))?;
    }
    let data = serde_json::to_vec_pretty(config).map_err(|e| format!("序列化配置失败：{e}"))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, data).map_err(|e| format!("写入配置失败：{e}"))?;
    fs::rename(&tmp, path).map_err(|e| format!("替换配置文件失败：{e}"))
}

/// 读取配置；返回值第二项表示内容被修复过、需要回写
fn read_config(path: &Path) -> (Config, bool) {
    let text = match fs::read_to_string(path) {
        Ok(text) if !text.trim().is_empty() => text,
        _ => return (Config::default(), false),
    };
    let mut config: Config = match serde_json::from_str(&text) {
        Ok(config) => config,
        Err(e) => {
            // 备份损坏的配置，避免之后保存时被覆盖而丢失
            eprintln!("配置文件解析失败，已使用默认配置：{e}");
            let _ = fs::copy(path, path.with_extension("json.bad"));
            return (Config::default(), false);
        }
    };

    let mut dirty = false;
    if config.version < CONFIG_VERSION {
        config.version = CONFIG_VERSION;
        dirty = true;
    }
    // 手动编辑可能导致 id 缺失或重复，统一补齐
    let mut seen = HashSet::new();
    for project in &mut config.projects {
        if project.id.is_empty() || !seen.insert(project.id.clone()) {
            project.id = new_id();
            seen.insert(project.id.clone());
            dirty = true;
        }
    }
    config.settings = config.settings.normalized();
    (config, dirty)
}

impl ConfigStore {
    pub fn load(path: PathBuf) -> Self {
        let (config, dirty) = read_config(&path);
        if dirty && let Err(e) = write_atomic(&path, &config) {
            eprintln!("回写修复后的配置失败：{e}");
        }
        Self {
            path,
            config: Mutex::new(config),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn lock(&self) -> MutexGuard<'_, Config> {
        // 某次修改中途 panic 不应让配置永久不可用
        self.config.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn snapshot(&self) -> Config {
        self.lock().clone()
    }

    pub fn project(&self, id: &str) -> Option<Project> {
        self.lock().projects.iter().find(|p| p.id == id).cloned()
    }

    pub fn settings(&self) -> Settings {
        self.lock().settings.clone()
    }

    /// 在副本上修改并落盘，成功后才替换内存中的配置
    fn mutate<T>(&self, f: impl FnOnce(&mut Config) -> Result<T, String>) -> Result<T, String> {
        let mut guard = self.lock();
        let mut next = guard.clone();
        let result = f(&mut next)?;
        write_atomic(&self.path, &next)?;
        *guard = next;
        Ok(result)
    }

    /// 新建（id 为空时）或更新项目，返回规范化后的项目
    pub fn upsert_project(&self, mut project: Project) -> Result<Project, String> {
        self.mutate(|config| {
            let id = project.id.clone();
            let others = config.projects.iter().filter(|p| p.id != id);
            project.normalize_and_validate(others)?;
            if project.id.is_empty() {
                project.id = new_id();
                config.projects.push(project.clone());
            } else {
                let slot = config
                    .projects
                    .iter_mut()
                    .find(|p| p.id == project.id)
                    .ok_or("项目不存在，可能已被删除")?;
                *slot = project.clone();
            }
            Ok(project)
        })
    }

    pub fn delete_project(&self, id: &str) -> Result<(), String> {
        self.mutate(|config| {
            let before = config.projects.len();
            config.projects.retain(|p| p.id != id);
            if config.projects.len() == before {
                return Err("项目不存在，可能已被删除".into());
            }
            Ok(())
        })
    }

    pub fn update_settings(&self, settings: Settings) -> Result<Settings, String> {
        self.mutate(|config| {
            config.settings = settings.normalized();
            Ok(config.settings.clone())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 读写与修复() {
        let dir = std::env::temp_dir().join(format!("venvdock-test-{}", new_id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");

        // 缺 id、重复 id 的项目在加载时被补齐，并回写到文件
        fs::write(&path, r#"{"projects":[{"name":"a","command":"x"},{"id":"1","name":"b","command":"x"},{"id":"1","name":"c","command":"x"}]}"#).unwrap();
        let store = ConfigStore::load(path.clone());
        let ids: HashSet<_> = store
            .snapshot()
            .projects
            .iter()
            .map(|p| p.id.clone())
            .collect();
        assert_eq!(ids.len(), 3);
        assert!(!ids.contains(""));

        // 重名（不区分大小写）被拒绝，且失败的修改不影响内存配置
        let dup = Project {
            name: " A ".into(),
            command: "y".into(),
            ..Default::default()
        };
        assert!(store.upsert_project(dup).is_err());
        assert_eq!(store.snapshot().projects.len(), 3);

        // 新建成功后重新加载仍能读到
        let created = store
            .upsert_project(Project {
                name: "d".into(),
                command: "y".into(),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(ConfigStore::load(path.clone()).snapshot().projects.len(), 4);
        store.delete_project(&created.id).unwrap();

        // 损坏的配置被备份，并回退到默认配置
        fs::write(&path, "{坏的").unwrap();
        assert!(
            ConfigStore::load(path.clone())
                .snapshot()
                .projects
                .is_empty()
        );
        assert!(path.with_extension("json.bad").exists());

        fs::remove_dir_all(&dir).unwrap();
    }
}
