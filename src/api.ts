// 与 Rust 端 config::model 一一对应的类型，以及命令调用封装
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

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
}

export interface Settings {
  launchOnBoot: boolean;
  showWindowOnStart: boolean;
  stopTimeoutSecs: number;
  restartDelaySecs: number;
  restartMaxDelaySecs: number;
  maxRestartAttempts: number;
  logMaxSizeMb: number;
  logKeepFiles: number;
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
  };
}

export const api = {
  getConfig: () => invoke<Config>("get_config"),
  saveProject: (project: Project) => invoke<Project>("save_project", { project }),
  deleteProject: (id: string) => invoke<void>("delete_project", { id }),
  updateSettings: (settings: Settings) => invoke<Settings>("update_settings", { settings }),
  revealConfigFile: () => invoke<void>("reveal_config_file"),
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
  /** 订阅状态变化推送，返回取消订阅函数 */
  onStatus: (handler: (status: ProjectStatus) => void): Promise<UnlistenFn> =>
    listen<ProjectStatus>("project-status", (event) => handler(event.payload)),
};
