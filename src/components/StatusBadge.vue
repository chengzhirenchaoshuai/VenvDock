<script setup lang="ts">
// 状态徽标：圆点 + 文字，运行中时圆点带呼吸动画
import { computed } from "vue";
import type { Phase } from "../api";

const props = defineProps<{ phase: Phase }>();

const STYLES: Record<Phase, { text: string; badge: string; dot: string }> = {
  running: {
    text: "运行中",
    badge: "bg-ok/10 text-ok ring-ok/25",
    dot: "bg-ok",
  },
  stopping: {
    text: "停止中",
    badge: "bg-warn/10 text-warn ring-warn/25",
    dot: "bg-warn",
  },
  backoff: {
    text: "等待重启",
    badge: "bg-warn/10 text-warn ring-warn/25",
    dot: "bg-warn",
  },
  failed: {
    text: "失败",
    badge: "bg-fail/10 text-fail ring-fail/25",
    dot: "bg-fail",
  },
  stopped: {
    text: "已停止",
    badge: "bg-subtle text-muted ring-line-strong",
    dot: "bg-faint",
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
