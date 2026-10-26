<div align="center">

<img src="src-tauri/icons/128x128.png" width="96" alt="VenvDock" />

# VenvDock

**Windows 本地服务托管面板**

在托盘常驻，托管 Python 虚拟环境中的服务（如 Open WebUI）或任意命令行程序：<br/>
一键启停、崩溃自动重启、实时日志、开机自启。

![Platform](https://img.shields.io/badge/平台-Windows%2010%20%2F%2011-0078D4?logo=windows)
![Tauri](https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri&logoColor=white)
![Vue](https://img.shields.io/badge/Vue-3-4FC08D?logo=vuedotjs&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-2024-000000?logo=rust)
![Size](https://img.shields.io/badge/安装包-1.5%20MB-6366F1)

[下载安装包](../../releases/latest)

</div>

<p align="center">
  <img src="docs/main.png" width="49%" alt="项目列表" />
  <img src="docs/logs.png" width="49%" alt="实时日志" />
</p>

## ✨ 功能

| | |
|---|---|
| 🚀 **启停与重启** | 停止时先发送 Ctrl+C 让服务优雅退出，超时再强制结束整个进程树 |
| 🧹 **不留孤儿进程** | 子进程全部放入 Job Object，VenvDock 退出或崩溃时一并回收 |
| 🔁 **崩溃自动重启** | 指数退避（默认 3 秒起、上限 60 秒），连续快速失败超过次数后放弃并弹出系统通知 |
| 🩺 **健康检查** | 可选：每 15 秒访问服务网址，服务响应过后连续 3 次无响应即结束进程并重启，处理“进程还在但已卡死” |
| 📜 **实时日志** | 界面内实时查看、搜索、按级别筛选，显示 ANSI 颜色；按大小轮转写入文件；自动处理 GBK / UTF-8 混合输出 |
| 🐍 **venv 支持** | 指定虚拟环境即等同于先激活它；可设置工作目录、日志目录、环境变量 |
| 🖥️ **托盘常驻** | 开机自启（默认只在托盘）、单实例、关闭窗口即最小化到托盘；托盘菜单可直接启停各项目或全部启停 |
| 📦 **导入导出** | 导出配置、导入项目，方便换电脑 |
| ⬆️ **一键更新** | 自动检查新版本，确认后下载、校验并替换，重启后恢复更新前正在运行的项目 |
| 🪶 **低占用** | 窗口关闭 30 秒后释放 WebView，后台常驻约 12 MB 内存；开机启动时不加载界面，约 4 MB |
| 💼 **免安装版** | 单个 exe 直接运行；需要便携时在 exe 旁建 `data` 文件夹即可 |

## 📦 安装

从 [Releases](../../releases/latest) 下载：

- **安装版** `VenvDock_x.y.z_x64-setup.exe`：按当前用户安装，无需管理员权限
- **免安装版** `VenvDock_x.y.z_x64-portable.exe`：单个文件，放到任意位置直接运行

> 依赖系统自带的 WebView2（Windows 10/11 均已预装）。

## 🧭 使用

1. 点击 **添加项目**，填写名称和启动命令，例如：

   | 字段 | 示例 |
   |---|---|
   | 启动命令 | `open-webui serve` |
   | 虚拟环境 | `F:\AI\OpenWebUI\venv` |
   | 环境变量 | `PORT=8080` |

2. 勾选 **自动启动** 后，VenvDock 启动时会自动拉起该项目；勾选 **自动重启** 后，意外退出会按退避策略重新启动。
3. 点击卡片上的日志图标查看实时输出。

命令通过 `cmd /c` 执行，支持 `&&`、管道和 `.bat`。

## 📁 文件位置

安装版和免安装版默认都保存在当前用户目录下：

| 内容 | 默认位置 | 便携模式 |
|---|---|---|
| 配置 | `%APPDATA%\com.venvdock.app\config.json` | `data\config.json` |
| 日志（默认） | `%APPDATA%\com.venvdock.app\logs\<项目名>.log` | `data\logs\<项目名>.log` |
| 界面缓存 | `%LOCALAPPDATA%\com.venvdock.app` | `data\webview` |
| 程序日志 | `%APPDATA%\com.venvdock.app\venvdock.log` | `data\venvdock.log` |

> 在 exe 所在目录新建 `data` 文件夹即进入便携模式，所有数据都写在其中。

## 🛠️ 开发

需要 Rust、Node.js、pnpm 和 MSVC 构建工具。

```bash
pnpm install
pnpm tauri dev                # 开发运行（开发版不会写入开机自启注册表项）
pnpm package                  # 打包安装版和免安装版（单个 exe），统一输出到 release/
```

发布：推送与 `tauri.conf.json` 版本一致的标签（如 `git tag v0.3.0 && git push origin v0.3.0`），GitHub Actions 会自动构建并上传到 Releases。

自动更新靠 minisign 签名防止安装包被替换：打包时给安装版和免安装版各生成一个 `.sig`，程序里只有公钥（`src-tauri/src/updater.rs` 的 `PUBLIC_KEY`），校验通过才会更新。私钥放在与项目文件夹同级的 `ProjectKey\VenvDock\venvdock.key`（不在仓库里），CI 使用仓库 secret `TAURI_SIGNING_PRIVATE_KEY`（同一私钥）。**务必另外备份私钥**，丢了以后已安装的版本就无法自动更新。发布必须是“最新正式版”，并带上两个 exe 及其 `.sig`。

```text
src/                  前端（Vue 3 + Tailwind CSS 4）
src-tauri/src/
├─ config/            配置模型与原子存储
├─ runtime/
│  ├─ runner.rs       单个进程：启动、输出采集、优雅停止
│  ├─ supervisor.rs   状态机、退避重启、健康检查
│  ├─ health.rs       健康检查的 HTTP 探测
│  ├─ loghub.rs       实时日志推送
│  ├─ logs.rs         行切分、编码处理、日志轮转
│  └─ win.rs          Win32 封装（Job Object、Ctrl+C 等）
├─ window.rs          主窗口按需创建、隐藏后销毁
├─ tray.rs            托盘菜单
├─ notify.rs          失败时的系统通知
├─ applog.rs          程序自身的运行日志
├─ paths.rs           数据目录（安装版 / 便携版）
└─ commands.rs        前端调用的命令
```
