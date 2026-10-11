<script setup lang="ts">
// 路径输入框：右侧按钮弹出系统文件夹选择框
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen } from "@lucide/vue";

const model = defineModel<string>({ required: true });
defineProps<{ placeholder?: string }>();

async function browse() {
  const picked = await open({ directory: true, defaultPath: model.value || undefined });
  if (typeof picked === "string") model.value = picked;
}
</script>

<template>
  <div class="flex gap-2">
    <input v-model="model" class="input font-mono" :placeholder="placeholder" spellcheck="false" />
    <button type="button" class="btn-secondary shrink-0 px-2.5" title="选择文件夹" @click="browse">
      <FolderOpen class="size-4" />
    </button>
  </div>
</template>
