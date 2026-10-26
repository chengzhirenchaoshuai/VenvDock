# VenvDock

Windows 托盘工具：托管 venv 中的服务或任意命令（启停、崩溃退避重启、实时日志、开机自启）。

## 技术栈与结构

- Tauri 2 + Rust（edition 2024）后端；Vue 3 + TypeScript + Tailwind CSS 4 前端，图标用 `@lucide/vue`
- `src-tauri/src/config/`：配置模型（camelCase 序列化）与存储（先写盘再改内存、原子替换）
- `src-tauri/src/runtime/`：`runner` 单进程、`supervisor` 状态机、`loghub` 日志推送、`logs` 轮转、`win` Win32 封装
- `src-tauri/src/window.rs`：主窗口不随启动创建（`create: false`），按需创建；关闭先隐藏，30 秒后销毁 WebView（设置 `closeToTray` 为 false 时关闭按钮改为 `quit_app` 退出）；`RunEvent::ExitRequested{code: None}` 必须 `prevent_exit`
- `src-tauri/src/paths.rs`：默认数据放 `%APPDATA%`/`%LOCALAPPDATA%` 下的 `com.venvdock.app`；exe 旁有 `data/` 目录即便携模式，配置、日志、WebView2 数据都放其中。免安装版就是单个 exe（前端已内嵌），不带 `data/`
- 主题：`src/style.css` 中 `--vd-*` 变量按 `<html data-skin data-mode>` 取值（default / mist × light / dark），组件只用语义类（`bg-surface`、`text-muted`、`bg-accent` 等），不要再写死 zinc/indigo 颜色
- `src-tauri/src/tray.rs`：托盘菜单列出运行中服务的“打开”；状态/配置事件触发后台线程重建菜单（事件可能在持锁时发出，不能就地重建）。网址优先用项目配置，否则用 `logs::detect_local_url` 从日志识别
- `src/api.ts` 的类型与 Rust 端一一对应，改字段时两边同步
- `ProjectStatus.ready`：配置了网址的项目启动后由 `supervisor::ready_loop` 每 2 秒探测，连上前为 false（界面和托盘显示“启动中”、不给“打开”）；未配置网址的启动即为 true。手动停止 / 重启会清空 `last_exit_code`
- 健康检查（`supervisor::health_loop`）由本程序结束进程时写 `kill_reason`，退出回调据此总是走退避重启；服务首次响应前不计失败
- 通知（`notify.rs`）：安装版靠开始菜单快捷方式上的 AUMID；免安装版在 `HKCU\Software\Classes\AppUserModelId` 登记；开发版借用 PowerShell 的 AUMID
- 自动更新（`updater.rs`）：前端从 GitHub API 取到发布文件列表后调用 `install_update`；后端用 `URLDownloadToFileW` 下载安装包及其 `.sig`，用 `minisign-verify` 和内置公钥校验签名（可信注释里的文件名须一致），只接受本仓库 Release 地址；私钥在 `..\ProjectKey\VenvDock\venvdock.key`，CI 用 secret `TAURI_SIGNING_PRIVATE_KEY`，`scripts/package.ps1` 负责签名；把自身复制到 `%TEMP%\venvdock-update` 作为助手（`--apply-update`，在 `main.rs` 中先于 Tauri 处理），本程序 `quit_app` 后由助手替换 exe（免安装版，先写 `.new` 再改名）或运行安装程序 `/P /UPDATE`（安装版，判断依据是 exe 旁有 `uninstall.exe`），再启动新版本；更新前运行中的项目记在 `resume.json`，新版本启动时恢复
- 检查更新在前端直接请求 GitHub API（`src/update.ts`），发布由 `.github/workflows/release.yml` 在推送 `v*` 标签时完成
- `src-tauri/src/applog.rs`：发布版没有控制台，程序自身的错误用 `applog::write` 记录（同时安装 panic 钩子），不要用 `eprintln!`

## 命令

```bash
pnpm tauri dev                          # 开发
pnpm build                              # 前端类型检查 + 构建
cd src-tauri && cargo clippy --all-targets && cargo test --lib
pnpm package                            # 打包安装版 + 免安装版（单 exe），输出到 release/
```

## 关键约定

- 子进程经 `cmd /d /s /c` 启动，`CREATE_SUSPENDED` 创建 → 加入 Job Object → 恢复，防止子孙进程逃逸
- 优雅停止靠 `venvdock.exe --send-ctrl-c <pid>` 助手进程（`main.rs` 中先于 Tauri 处理）；“忽略 Ctrl+C”标志会被继承，spawn 前必须 `SetConsoleCtrlHandler(None, FALSE)`
- 验证助手前先 `cargo build`：`cargo test` 不会重编主程序
- supervisor 用 `generation` 识别过期的退出回调与重启定时，锁顺序为 supervisor → ConfigStore
- 开发版（debug）不写开机自启注册表；带 `--autostart` 启动时默认不显示窗口
- 界面验证前先确认没有其他 VenvDock 在运行（用户可能常驻便携版）：单实例会把新启动的实例转给已有窗口，点击脚本就会操作到用户的程序。纯样式检查可用 mock 后端 + 无头 Edge 截图，不启动程序
- 界面验证：用 `PrintWindow` 截窗口、UI Automation 点按钮，按窗口类 `Tauri Window` 定位（`MainWindowHandle` 会指向单实例插件的消息窗口）
- 测试尽量少：现有 `config/store.rs`、`runtime/runner.rs` 各一个
