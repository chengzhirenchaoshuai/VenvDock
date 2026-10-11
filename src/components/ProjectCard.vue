<script setup lang="ts">
// 项目卡片：状态、命令、运行信息，以及启停等操作
import { computed, ref } from "vue";
import { FileText, Pencil, Play, RotateCw, Square, Trash } from "@lucide/vue";
import { api, type Project, type ProjectStatus } from "../api";
import { errorText, formatDuration, notify, now } from "../store";
import StatusBadge from "./StatusBadge.vue";

const props = defineProps<{ project: Project; status: ProjectStatus | undefined }>();
const emit = defineEmits<{ edit: []; delete: []; logs: [] }>();

const phase = computed(() => props.status?.phase ?? "stopped");
// 等待重启时也显示“停止”按钮，用于取消重启
const active = computed(() => ["running", "stopping", "backoff"].includes(phase.value));
const busy = ref(false);

/** 运行信息行：PID、运行时长、退出码、重启倒计时 */
const details = computed(() => {
  const s = props.status;
  const items: string[] = [];
  if (!s) return items;
  if (s.pid !== null) items.push(`PID ${s.pid}`);
  if (s.startedAt !== null) items.push(`已运行 ${formatDuration(now.value - s.startedAt)}`);
  if (s.nextRestartAt !== null) {
    items.push(`${Math.max(0, Math.ceil((s.nextRestartAt - now.value) / 1000))} 秒后重启`);
  }
  if (!active.value && s.lastExitCode !== null) items.push(`上次退出码 ${s.lastExitCode}`);
  return items;
});

/** 执行操作，失败时弹出提示 */
async function run(action: () => Promise<unknown>) {
  busy.value = true;
  try {
    await action();
  } catch (e) {
    notify(`“${props.project.name}”：${errorText(e)}`, "error");
  } finally {
    busy.value = false;
  }
}

const start = () => run(() => api.startProject(props.project.id));
const stop = () => run(() => api.stopProject(props.project.id));
const restart = () => run(() => api.restartProject(props.project.id));
</script>

<template>
  <article
    class="group rounded-xl border border-zinc-200 bg-white px-5 py-4 shadow-xs transition hover:border-zinc-300 hover:shadow-sm dark:border-zinc-800 dark:bg-zinc-900 dark:hover:border-zinc-700"
  >
    <div class="flex items-center gap-4">
      <div class="min-w-0 flex-1">
        <div class="flex items-center gap-2.5">
          <h3 class="truncate text-[15px] font-semibold">{{ project.name }}</h3>
          <StatusBadge :phase="phase" />
          <span v-if="project.autoStart" class="text-xs text-zinc-400">自动启动</span>
          <span v-if="project.autoRestart" class="text-xs text-zinc-400">自动重启</span>
        </div>
        <p class="mt-1 truncate font-mono text-xs text-zinc-500 select-text" :title="project.command">
          <span class="text-zinc-400">$</span> {{ project.command }}
        </p>
      </div>

      <div class="flex shrink-0 items-center gap-1">
        <button v-if="!active" class="btn-primary mr-1 py-1.5" :disabled="busy" @click="start">
          <Play class="size-3.5" /> 启动
        </button>
        <button
          v-else
          class="btn-secondary mr-1 py-1.5"
          :disabled="busy || phase === 'stopping'"
          @click="stop"
        >
          <Square class="size-3.5" /> 停止
        </button>
        <button class="icon-btn" title="重启" :disabled="busy || phase !== 'running'" @click="restart">
          <RotateCw class="size-4" />
        </button>
        <button class="icon-btn" title="查看日志" @click="emit('logs')">
          <FileText class="size-4" />
        </button>
        <button class="icon-btn" title="编辑" @click="emit('edit')">
          <Pencil class="size-4" />
        </button>
        <button class="icon-btn hover:text-rose-600 dark:hover:text-rose-400" title="删除" @click="emit('delete')">
          <Trash class="size-4" />
        </button>
      </div>
    </div>

    <div
      v-if="details.length || status?.message"
      class="mt-3 flex flex-wrap items-center gap-x-4 gap-y-1 border-t border-zinc-100 pt-3 text-xs text-zinc-500 dark:border-zinc-800"
    >
      <span v-for="item in details" :key="item">{{ item }}</span>
      <span
        v-if="status?.message"
        class="select-text"
        :class="phase === 'failed' ? 'text-rose-600 dark:text-rose-400' : 'text-amber-600 dark:text-amber-400'"
      >
        {{ status.message }}
      </span>
    </div>
  </article>
</template>
