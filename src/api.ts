// 与 Rust 端 config::model 一一对应的类型，以及命令调用封装
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { Appearance, Skin } from "./theme";

export interface EnvVar {
  key: string;
  value: string;
}

export interface Project {
  /** 为空字符串表示新建 */
  id: string;
  name: string;
  command: string;
  venvPath: string;
  workDir: string;
  logDir: string;
  env: EnvVar[];
  autoStart: boolean;
  autoRestart: boolean;
  /** 网页地址（可选），为空时使用从日志识别到的地址 */
  url: string;
  /** 定时访问网址，服务起来后连续无响应则结束进程并重启 */
  healthCheck: boolean;
}

export interface Settings {
  launchOnBoot: boolean;
  showWindowOnStart: boolean;
  /** 点击窗口关闭按钮时最小化到托盘，否则退出程序 */
  closeToTray: boolean;
  stopTimeoutSecs: number;
  restartDelaySecs: number;
  restartMaxDelaySecs: number;
  maxRestartAttempts: number;
  logMaxSizeMb: number;
  logKeepFiles: number;
  theme: Skin;
  appearance: Appearance;
  /** 项目失败时弹出系统通知 */
  notifyOnFailure: boolean;
}

export interface Config {
  version: number;
  settings: Settings;
  projects: Project[];
}

/** 运行阶段：已停止、运行中、停止中、等待自动重启、失败 */
export type Phase = "stopped" | "running" | "stopping" | "backoff" | "failed";

/** 项目运行状态，时间为 Unix 毫秒时间戳 */
export interface ProjectStatus {
  id: string;
  phase: Phase;
  pid: number | null;
  startedAt: number | null;
  lastExitCode: number | null;
  restartCount: number;
  nextRestartAt: number | null;
  message: string | null;
  /** 本次运行从日志识别到的本机网址 */
  detectedUrl: string | null;
  /** 服务已可访问：配置了网址的项目要等网址能连上，否则启动即为 true */
  ready: boolean;
}

/** 实时日志推送的一批行 */
export interface LogBatch {
  id: string;
  lines: string[];
}

/** 新建项目表单的初始值 */
export function emptyProject(): Project {
  return {
    id: "",
    name: "",
    command: "",
    venvPath: "",
    workDir: "",
    logDir: "",
    env: [],
    autoStart: false,
    autoRestart: true,
    url: "",
    healthCheck: false,
  };
}

export const api = {
  getConfig: () => invoke<Config>("get_config"),
  saveProject: (project: Project) => invoke<Project>("save_project", { project }),
  deleteProject: (id: string) => invoke<void>("delete_project", { id }),
  updateSettings: (settings: Settings) => invoke<Settings>("update_settings", { settings }),
  revealConfigFile: () => invoke<void>("reveal_config_file"),
  exportConfig: (path: string) => invoke<void>("export_config", { path }),
  /** 导入项目，返回新增和覆盖的数量 */
  importConfig: (path: string) => invoke<{ added: number; updated: number }>("import_config", { path }),
  revealLog: (id: string) => invoke<void>("reveal_log", { id }),
  /** 开始实时查看日志，返回最近的行；之后的新行通过 onLog 推送 */
  openLog: (id: string) => invoke<string[]>("open_log", { id }),
  closeLog: () => invoke<void>("close_log"),
  onLog: (handler: (batch: LogBatch) => void): Promise<UnlistenFn> =>
    listen<LogBatch>("project-log", (event) => handler(event.payload)),
  getStatuses: () => invoke<ProjectStatus[]>("get_statuses"),
  startProject: (id: string) => invoke<void>("start_project", { id }),
  stopProject: (id: string) => invoke<void>("stop_project", { id }),
  restartProject: (id: string) => invoke<void>("restart_project", { id }),
  startAll: () => invoke<void>("start_all"),
  stopAll: () => invoke<void>("stop_all"),
  /** 订阅状态变化推送，返回取消订阅函数 */
  onStatus: (handler: (status: ProjectStatus) => void): Promise<UnlistenFn> =>
    listen<ProjectStatus>("project-status", (event) => handler(event.payload)),
};
