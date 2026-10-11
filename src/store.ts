// 全局状态：配置、各项目运行状态、提示消息
import { reactive, ref } from "vue";
import { api, type Config, type ProjectStatus } from "./api";
import { applyTheme } from "./theme";

export const state = reactive({
  config: null as Config | null,
  statuses: {} as Record<string, ProjectStatus>,
});

/** 每秒更新的当前时间，用于显示运行时长和重启倒计时 */
export const now = ref(Date.now());
setInterval(() => (now.value = Date.now()), 1000);

/** 先订阅状态推送再拉取全量，避免两者之间的变化丢失 */
export async function init() {
  await api.onStatus((status) => (state.statuses[status.id] = status));
  await reload();
}

export async function reload() {
  const [config, statuses] = await Promise.all([api.getConfig(), api.getStatuses()]);
  state.config = config;
  applyTheme(config.settings.theme, config.settings.appearance);
  state.statuses = Object.fromEntries(statuses.map((s) => [s.id, s]));
}

export interface Toast {
  id: number;
  text: string;
  kind: "error" | "info";
}

export const toasts = ref<Toast[]>([]);
let toastSeq = 0;

export function notify(text: string, kind: Toast["kind"] = "info") {
  const id = ++toastSeq;
  toasts.value.push({ id, text, kind });
  setTimeout(() => (toasts.value = toasts.value.filter((t) => t.id !== id)), 4000);
}

/** invoke 失败时 Rust 端返回的是字符串 */
export function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
}

/** 毫秒时长转为中文描述，只保留最大的两个单位 */
export function formatDuration(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  const parts: [number, string][] = [
    [Math.floor(s / 86400), "天"],
    [Math.floor((s % 86400) / 3600), "小时"],
    [Math.floor((s % 3600) / 60), "分"],
    [s % 60, "秒"],
  ];
  const first = parts.findIndex(([n]) => n > 0);
  if (first < 0) return "0 秒";
  return parts
    .slice(first, first + 2)
    .filter(([n]) => n > 0)
    .map(([n, unit]) => `${n} ${unit}`)
    .join(" ");
}
