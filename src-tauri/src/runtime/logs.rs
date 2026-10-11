//! 日志：按行切分子进程输出、统一转为 UTF-8，并按大小轮转写入文件
use super::win::decode_oem;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// 单行最大字节数，超过后强制断行，防止无换行输出无限占用内存
const MAX_LINE_BYTES: usize = 16 * 1024;

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

/// 字节解码：合法 UTF-8 直接使用，否则按系统 OEM 代码页解码
pub fn decode_line(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => decode_oem(bytes),
    }
}

/// 行切分器：遇到 \n 输出一行；单独的 \r（进度条刷新）表示覆盖当前行，只保留最终内容
#[derive(Default)]
pub struct LineSplitter {
    buf: Vec<u8>,
    pending_cr: bool,
}

impl LineSplitter {
    pub fn push(&mut self, data: &[u8], mut emit: impl FnMut(&[u8])) {
        for &byte in data {
            match byte {
                b'\n' => {
                    emit(&self.buf);
                    self.buf.clear();
                    self.pending_cr = false;
                }
                b'\r' => self.pending_cr = true,
                _ => {
                    if self.pending_cr {
                        self.buf.clear();
                        self.pending_cr = false;
                    }
                    self.buf.push(byte);
                    if self.buf.len() >= MAX_LINE_BYTES {
                        emit(&self.buf);
                        self.buf.clear();
                    }
                }
            }
        }
    }

    /// 输出流结束时，把残留的半行也输出
    pub fn finish(&mut self, mut emit: impl FnMut(&[u8])) {
        if !self.buf.is_empty() {
            emit(&self.buf);
            self.buf.clear();
        }
    }
}

/// 日志文件写入器：name.log 写满后依次轮转为 name.1.log、name.2.log ……
pub struct LogWriter {
    path: PathBuf,
    file: Option<File>,
    size: u64,
    max_bytes: u64,
    keep: u32,
}

fn open_append(path: &Path) -> std::io::Result<File> {
    OpenOptions::new().create(true).append(true).open(path)
}

impl LogWriter {
    pub fn open(path: PathBuf, max_bytes: u64, keep: u32) -> Result<Self, String> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)
                .map_err(|e| format!("创建日志目录失败：{} ({e})", dir.display()))?;
        }
        let file = open_append(&path)
            .map_err(|e| format!("打开日志文件失败：{} ({e})", path.display()))?;
        let size = file.metadata().map(|m| m.len()).unwrap_or(0);
        let mut writer = Self {
            path,
            file: Some(file),
            size,
            max_bytes,
            keep,
        };
        if writer.size >= writer.max_bytes {
            writer.rotate();
        }
        Ok(writer)
    }

    pub fn write_line(&mut self, line: &str) {
        let Some(file) = self.file.as_mut() else {
            return;
        };
        // 写入失败（如磁盘满）时丢弃该行，不影响子进程运行
        if file
            .write_all(line.as_bytes())
            .and_then(|_| file.write_all(b"\n"))
            .is_ok()
        {
            self.size += line.len() as u64 + 1;
        }
        if self.size >= self.max_bytes {
            self.rotate();
        }
    }

    fn rotated_path(&self, index: u32) -> PathBuf {
        let stem = self.path.file_stem().unwrap_or_default().to_string_lossy();
        self.path.with_file_name(format!("{stem}.{index}.log"))
    }

    fn rotate(&mut self) {
        // Windows 上必须先关闭文件才能改名
        self.file = None;
        let _ = fs::remove_file(self.rotated_path(self.keep));
        for i in (1..self.keep).rev() {
            let _ = fs::rename(self.rotated_path(i), self.rotated_path(i + 1));
        }
        let rotated = fs::rename(&self.path, self.rotated_path(1)).is_ok();
        self.file = open_append(&self.path).ok();
        // 改名失败（文件被其他程序占用）时继续追加，写满下一轮再重试
        self.size = if rotated {
            0
        } else {
            self.size.saturating_sub(self.max_bytes)
        };
    }
}

/// 读取日志文件末尾最多 max_lines 行（只读最后 512KB，避免大文件拖慢界面）
pub fn read_tail(path: &Path, max_lines: usize) -> Vec<String> {
    use std::io::{Read, Seek, SeekFrom};
    const TAIL_BYTES: u64 = 512 * 1024;
    let Ok(mut file) = File::open(path) else {
        return Vec::new();
    };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let start = len.saturating_sub(TAIL_BYTES);
    let mut bytes = Vec::new();
    if file.seek(SeekFrom::Start(start)).is_err() || file.read_to_end(&mut bytes).is_err() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&bytes);
    let mut lines: Vec<&str> = text.lines().collect();
    // 从文件中间开始读时，第一行可能不完整
    if start > 0 && !lines.is_empty() {
        lines.remove(0);
    }
    let skip = lines.len().saturating_sub(max_lines);
    lines[skip..].iter().map(|l| l.to_string()).collect()
}
