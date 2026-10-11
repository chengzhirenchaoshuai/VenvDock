# VenvDock

Windows 托盘工具：托管 venv 中的服务或任意命令（启停、崩溃退避重启、实时日志、开机自启）。

## 技术栈与结构

- Tauri 2 + Rust（edition 2024）后端；Vue 3 + TypeScript + Tailwind CSS 4 前端，图标用 `@lucide/vue`
- `src-tauri/src/config/`：配置模型（camelCase 序列化）与存储（先写盘再改内存、原子替换）
- `src-tauri/src/runtime/`：`runner` 单进程、`supervisor` 状态机、`loghub` 日志推送、`logs` 轮转、`win` Win32 封装
- `src/api.ts` 的类型与 Rust 端一一对应，改字段时两边同步

## 命令

```bash
pnpm tauri dev                          # 开发
pnpm build                              # 前端类型检查 + 构建
cd src-tauri && cargo clippy --all-targets && cargo test --lib
pnpm tauri build                        # 打包 NSIS 安装包
```

## 关键约定

- 子进程经 `cmd /d /s /c` 启动，`CREATE_SUSPENDED` 创建 → 加入 Job Object → 恢复，防止子孙进程逃逸
- 优雅停止靠 `venvdock.exe --send-ctrl-c <pid>` 助手进程（`main.rs` 中先于 Tauri 处理）；“忽略 Ctrl+C”标志会被继承，spawn 前必须 `SetConsoleCtrlHandler(None, FALSE)`
- 验证助手前先 `cargo build`：`cargo test` 不会重编主程序
- supervisor 用 `generation` 识别过期的退出回调与重启定时，锁顺序为 supervisor → ConfigStore
- 开发版（debug）不写开机自启注册表；带 `--autostart` 启动时默认不显示窗口
- 界面验证：用 `PrintWindow` 截窗口、UI Automation 点按钮，按窗口类 `Tauri Window` 定位（`MainWindowHandle` 会指向单实例插件的消息窗口）
- 测试尽量少：现有 `config/store.rs`、`runtime/runner.rs` 各一个
