<script setup lang="ts">
// 实时日志查看：右侧抽屉，显示最近日志并持续追加新输出
import { computed, nextTick, onBeforeUnmount, onMounted, ref, shallowRef } from "vue";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { ArrowDownToLine, Eraser, FolderOpen, Search, X } from "@lucide/vue";
import { api, type Project } from "../api";
import { parseAnsi, type Span } from "../ansi";
import { errorText, notify, state } from "../store";
import StatusBadge from "./StatusBadge.vue";

const props = defineProps<{ project: Project }>();
const emit = defineEmits<{ close: [] }>();

/** 界面中最多保留的行数，超出丢弃最旧的 */
const MAX_LINES = 5000;

/** 日志级别：0 普通、1 警告、2 错误 */
type Level = 0 | 1 | 2;

/** 一行日志；id 递增且不复用，丢弃旧行时其余行的 key 不变，Vue 只需增删首尾节点 */
interface Line {
  id: number;
  /** 去掉控制符后的文本，用于筛选和搜索 */
  text: string;
  /** 带 ANSI 颜色的片段；没有颜色时为 null，按内容着色 */
  spans: Span[] | null;
  level: Level;
  cls: string;
}

const lines = shallowRef<Line[]>([]);
let lineSeq = 0;
const query = ref("");
/** 只显示该级别及以上的行 */
const minLevel = ref<Level>(0);
const autoScroll = ref(true);
const box = ref<HTMLElement>();
let unlisten: UnlistenFn | undefined;

const phase = computed(() => state.statuses[props.project.id]?.phase ?? "stopped");
const ready = computed(() => state.statuses[props.project.id]?.ready ?? false);

const ERROR_RE = /\b(error|exception|traceback|critical|fatal)\b|错误|失败/i;
const WARN_RE = /\bwarn(ing)?\b|警告/i;

/** 解析一行：去掉控制符、识别级别；级别和颜色在追加时算好，渲染时不再重复计算 */
function toLine(raw: string): Line {
  const { text, spans } = parseAnsi(raw);
  const level: Level = ERROR_RE.test(text) ? 2 : WARN_RE.test(text) ? 1 : 0;
  const cls = text.startsWith("==========")
    ? "text-log-marker"
    : ["text-log-fg", "text-log-warn", "text-log-error"][level];
  return { id: ++lineSeq, text, spans, level, cls };
}

function append(newLines: string[]) {
  const merged = lines.value.concat(newLines.map(toLine));
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
  const min = minLevel.value;
  if (!q && !min) return lines.value;
  return lines.value.filter((l) => l.level >= min && (!q || l.text.toLowerCase().includes(q)));
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
    <section class="flex h-full w-[min(1100px,94vw)] flex-col bg-log-bg text-log-strong shadow-2xl">
      <header class="flex items-center gap-3 border-b border-log-line px-5 py-3">
        <h2 class="truncate text-sm font-semibold">{{ project.name }}</h2>
        <StatusBadge :phase="phase" :ready="ready" />
        <span class="text-xs text-log-muted">{{ lines.length }} 行</span>

        <select
          v-model.number="minLevel"
          class="ml-auto rounded-lg border border-log-line bg-log-panel px-2 py-1.5 text-xs text-log-fg outline-none focus:border-accent"
          title="按级别筛选"
        >
          <option :value="0">全部级别</option>
          <option :value="1">警告及错误</option>
          <option :value="2">仅错误</option>
        </select>
        <div class="relative w-64">
          <Search class="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-log-muted" />
          <input
            v-model="query"
            class="w-full rounded-lg border border-log-line bg-log-panel py-1.5 pr-3 pl-8 text-xs outline-none placeholder:text-log-muted focus:border-accent"
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
          <p v-if="!filtered.length" class="text-log-muted">
            {{ lines.length ? "没有匹配的行" : "暂无日志，启动项目后这里会实时显示输出。" }}
          </p>
          <div v-for="line in filtered" :key="line.id" class="break-all whitespace-pre-wrap" :class="line.cls">
            <!-- 搜索时显示高亮，暂不保留颜色 -->
            <template v-if="query.trim() || !line.spans">
              <template v-for="(seg, j) in segments(line.text)" :key="j">
                <mark v-if="seg.hit" class="rounded-sm bg-log-hit text-log-bg">{{ seg.text }}</mark>
                <template v-else>{{ seg.text }}</template>
              </template>
            </template>
            <template v-else>
              <span v-for="(span, j) in line.spans" :key="j" :style="span.style">{{ span.text }}</span>
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
  @apply inline-flex size-8 items-center justify-center rounded-lg text-log-dim transition hover:bg-log-line hover:text-log-strong;
}
</style>
