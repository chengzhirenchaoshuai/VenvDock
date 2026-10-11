<script setup lang="ts">
// 通用模态框：点击遮罩或按 Esc 关闭
import { onBeforeUnmount, onMounted } from "vue";
import { X } from "@lucide/vue";

defineProps<{ title: string; width?: string }>();
const emit = defineEmits<{ close: [] }>();

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape") emit("close");
}
onMounted(() => window.addEventListener("keydown", onKey));
onBeforeUnmount(() => window.removeEventListener("keydown", onKey));
</script>

<template>
  <div
    class="fixed inset-0 z-40 flex items-center justify-center bg-black/40 p-6 backdrop-blur-[2px]"
    @mousedown.self="emit('close')"
  >
    <div
      class="flex max-h-full w-full flex-col overflow-hidden rounded-2xl border border-line bg-surface shadow-2xl"
      :class="width ?? 'max-w-xl'"
    >
      <header class="flex items-center justify-between px-6 pt-5 pb-3">
        <h2 class="text-base font-semibold">{{ title }}</h2>
        <button class="icon-btn -mr-2" title="关闭" @click="emit('close')">
          <X class="size-4" />
        </button>
      </header>
      <div class="flex-1 overflow-y-auto px-6 pb-2">
        <slot />
      </div>
      <footer class="flex items-center justify-end gap-2 px-6 py-4">
        <slot name="footer" />
      </footer>
    </div>
  </div>
</template>
