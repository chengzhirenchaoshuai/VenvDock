<script setup lang="ts">
// 开关：带标题和说明的整行可点击开关
const model = defineModel<boolean>({ required: true });
defineProps<{ label: string; description?: string }>();
</script>

<template>
  <label class="flex cursor-pointer items-center justify-between gap-4 py-2">
    <span>
      <span class="block text-sm font-medium">{{ label }}</span>
      <span v-if="description" class="block text-xs text-muted">{{ description }}</span>
    </span>
    <input v-model="model" type="checkbox" class="peer sr-only" />
    <span
      class="relative h-5 w-9 shrink-0 rounded-full bg-line-strong transition peer-checked:bg-accent peer-focus-visible:ring-3 peer-focus-visible:ring-accent/30 after:absolute after:top-0.5 after:left-0.5 after:size-4 after:rounded-full after:bg-white after:shadow after:transition peer-checked:after:translate-x-4"
    ></span>
  </label>
</template>
