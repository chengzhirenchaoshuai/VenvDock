<script setup lang="ts">
// 状态徽标：圆点 + 文字，运行中时圆点带呼吸动画
import { computed } from "vue";
import type { Phase } from "../api";

const props = defineProps<{ phase: Phase }>();

const STYLES: Record<Phase, { text: string; badge: string; dot: string }> = {
  running: {
    text: "运行中",
    badge: "bg-emerald-50 text-emerald-700 ring-emerald-600/20 dark:bg-emerald-500/10 dark:text-emerald-400 dark:ring-emerald-400/20",
    dot: "bg-emerald-500",
  },
  stopping: {
    text: "停止中",
    badge: "bg-amber-50 text-amber-700 ring-amber-600/20 dark:bg-amber-500/10 dark:text-amber-400 dark:ring-amber-400/20",
    dot: "bg-amber-500",
  },
  backoff: {
    text: "等待重启",
    badge: "bg-amber-50 text-amber-700 ring-amber-600/20 dark:bg-amber-500/10 dark:text-amber-400 dark:ring-amber-400/20",
    dot: "bg-amber-500",
  },
  failed: {
    text: "失败",
    badge: "bg-rose-50 text-rose-700 ring-rose-600/20 dark:bg-rose-500/10 dark:text-rose-400 dark:ring-rose-400/20",
    dot: "bg-rose-500",
  },
  stopped: {
    text: "已停止",
    badge: "bg-zinc-100 text-zinc-600 ring-zinc-500/20 dark:bg-zinc-800 dark:text-zinc-400 dark:ring-zinc-400/20",
    dot: "bg-zinc-400",
  },
};

const style = computed(() => STYLES[props.phase]);
</script>

<template>
  <span
    class="inline-flex shrink-0 items-center gap-1.5 rounded-full px-2 py-0.5 text-xs font-medium ring-1 ring-inset"
    :class="style.badge"
  >
    <span class="relative flex size-1.5">
      <span
        v-if="phase === 'running'"
        class="absolute inline-flex size-full animate-ping rounded-full opacity-60"
        :class="style.dot"
      ></span>
      <span class="relative inline-flex size-1.5 rounded-full" :class="style.dot"></span>
    </span>
    {{ style.text }}
  </span>
</template>
