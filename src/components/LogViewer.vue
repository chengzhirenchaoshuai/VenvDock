<script setup lang="ts">
// 实时日志查看：右侧抽屉，显示最近日志并持续追加新输出
import { computed, nextTick, onBeforeUnmount, onMounted, ref, shallowRef } from "vue";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { ArrowDownToLine, Eraser, FolderOpen, Search, X } from "@lucide/vue";
import { api, type Project } from "../api";
import { errorText, notify, state } from "../store";
import StatusBadge from "./StatusBadge.vue";

const props = defineProps<{ project: Project }>();
const emit = defineEmits<{ close: [] }>();

/** 界面中最多保留的行数，超出丢弃最旧的 */
const MAX_LINES = 5000;

const lines = shallowRef<string[]>([]);
const query = ref("");
const autoScroll = ref(true);
const box = ref<HTMLElement>();
let unlisten: UnlistenFn | undefined;

const phase = computed(() => state.statuses[props.project.id]?.phase ?? "stopped");

/** 去掉 ANSI 颜色控制符 */
const stripAnsi = (s: string) => s.replace(/\x1b\[[0-9;?]*[A-Za-z]/g, "");

function append(newLines: string[]) {
  const merged = lines.value.concat(newLines.map(stripAnsi));
  lines.value = merged.length > MAX_LINES ? merged.slice(merged.length - MAX_LINES) : merged;
  if (autoScroll.value) nextTick(scrollToBottom);
}

function scrollToBottom() {
  if (box.value) box.value.scrollTop = box.value.scrollHeight;
  autoScroll.value = true;
}

/** 用户向上滚动时暂停自动滚动，回到底部时恢复 */
function onScroll() {
  const el = box.value;
  if (el) autoScroll.value = el.scrollHeight - el.scrollTop - el.clientHeight < 24;
}

const filtered = computed(() => {
  const q = query.value.trim().toLowerCase();
  return q ? lines.value.filter((l) => l.toLowerCase().includes(q)) : lines.value;
});

/** 把一行按搜索词拆成普通片段和高亮片段 */
function segments(line: string): { text: string; hit: boolean }[] {
  const q = query.value.trim();
  if (!q) return [{ text: line, hit: false }];
  const parts: { text: string; hit: boolean }[] = [];
  const lower = line.toLowerCase();
  const ql = q.toLowerCase();
  let pos = 0;
  for (let i = lower.indexOf(ql); i >= 0; i = lower.indexOf(ql, pos)) {
    if (i > pos) parts.push({ text: line.slice(pos, i), hit: false });
    parts.push({ text: line.slice(i, i + q.length), hit: true });
    pos = i + q.length;
  }
  if (pos < line.length) parts.push({ text: line.slice(pos), hit: false });
  return parts;
}

/** 按内容给行着色：分隔行、错误、警告 */
function lineClass(line: string): string {
  if (line.startsWith("==========")) return "text-indigo-300";
  if (/\b(error|exception|traceback|critical|fatal)\b|错误|失败/i.test(line)) return "text-rose-300";
  if (/\bwarn(ing)?\b|警告/i.test(line)) return "text-amber-300";
  return "text-zinc-300";
}

async function revealLog() {
  try {
    await api.revealLog(props.project.id);
  } catch (e) {
    notify(errorText(e), "error");
  }
}

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape") emit("close");
}

onMounted(async () => {
  window.addEventListener("keydown", onKey);
  // 先订阅再拉取历史，避免中间的输出丢失
  unlisten = await api.onLog((batch) => {
    if (batch.id === props.project.id) append(batch.lines);
  });
  try {
    append(await api.openLog(props.project.id));
  } catch (e) {
    notify(errorText(e), "error");
  }
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKey);
  unlisten?.();
  api.closeLog();
});
</script>

<template>
  <div class="fixed inset-0 z-40 flex justify-end bg-black/40" @mousedown.self="emit('close')">
    <section class="flex h-full w-[min(1100px,94vw)] flex-col bg-log-bg text-zinc-100 shadow-2xl">
      <header class="flex items-center gap-3 border-b border-zinc-800 px-5 py-3">
        <h2 class="truncate text-sm font-semibold">{{ project.name }}</h2>
        <StatusBadge :phase="phase" />
        <span class="text-xs text-zinc-500">{{ lines.length }} 行</span>

        <div class="relative ml-auto w-64">
          <Search class="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-zinc-500" />
          <input
            v-model="query"
            class="w-full rounded-lg border border-zinc-800 bg-zinc-900 py-1.5 pr-3 pl-8 text-xs outline-none placeholder:text-zinc-500 focus:border-accent"
            placeholder="筛选日志"
            spellcheck="false"
          />
        </div>
        <button class="log-btn" title="清屏（不影响日志文件）" @click="lines = []">
          <Eraser class="size-4" />
        </button>
        <button class="log-btn" title="在资源管理器中打开" @click="revealLog">
          <FolderOpen class="size-4" />
        </button>
        <button class="log-btn" title="关闭" @click="emit('close')">
          <X class="size-4" />
        </button>
      </header>

      <div class="relative min-h-0 flex-1">
        <div ref="box" class="h-full overflow-y-auto px-5 py-3 font-mono text-xs leading-5 select-text" @scroll="onScroll">
          <p v-if="!filtered.length" class="text-zinc-500">
            {{ lines.length ? "没有匹配的行" : "暂无日志，启动项目后这里会实时显示输出。" }}
          </p>
          <div v-for="(line, i) in filtered" :key="i" class="break-all whitespace-pre-wrap" :class="lineClass(line)">
            <template v-for="(seg, j) in segments(line)" :key="j">
              <mark v-if="seg.hit" class="rounded-sm bg-amber-400/80 text-zinc-950">{{ seg.text }}</mark>
              <template v-else>{{ seg.text }}</template>
            </template>
          </div>
        </div>
        <button
          v-if="!autoScroll"
          class="absolute right-6 bottom-5 inline-flex items-center gap-1.5 rounded-full bg-accent px-3 py-1.5 text-xs font-medium text-accent-fg shadow-lg hover:bg-accent-hover"
          @click="scrollToBottom"
        >
          <ArrowDownToLine class="size-3.5" /> 回到底部
        </button>
      </div>
    </section>
  </div>
</template>

<style scoped>
@reference "../style.css";
.log-btn {
  @apply inline-flex size-8 items-center justify-center rounded-lg text-zinc-400 transition hover:bg-zinc-800 hover:text-zinc-100;
}
</style>
