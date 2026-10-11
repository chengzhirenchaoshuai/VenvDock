<script setup lang="ts">
// 主页头部的主题与外观切换：点击循环切换并立即保存
import { computed } from "vue";
import { Monitor, Moon, Palette, Sun } from "@lucide/vue";
import { api, type Settings } from "../api";
import { errorText, notify, state } from "../store";
import { applyTheme, type Appearance, type Skin } from "../theme";

const skins: { value: Skin; label: string }[] = [
  { value: "default", label: "默认" },
  { value: "mist", label: "晨雾" },
];
const appearances: { value: Appearance; label: string; icon: typeof Sun }[] = [
  { value: "system", label: "跟随系统", icon: Monitor },
  { value: "light", label: "浅色", icon: Sun },
  { value: "dark", label: "深色", icon: Moon },
];

const settings = computed(() => state.config?.settings);
const skin = computed(() => skins.find((s) => s.value === settings.value?.theme) ?? skins[0]);
const appearance = computed(() => appearances.find((a) => a.value === settings.value?.appearance) ?? appearances[0]);

/** 取列表中的下一项，末尾回到开头 */
function next<T extends { value: string }>(list: T[], current: T): T {
  return list[(list.indexOf(current) + 1) % list.length];
}

/** 先套用再保存；保存失败时恢复原主题 */
async function update(patch: Partial<Settings>) {
  const config = state.config;
  if (!config) return;
  const old = config.settings;
  const updated = { ...old, ...patch };
  applyTheme(updated.theme, updated.appearance);
  try {
    config.settings = await api.updateSettings(updated);
  } catch (e) {
    applyTheme(old.theme, old.appearance);
    notify(errorText(e), "error");
  }
}
</script>

<template>
  <button
    class="icon-btn size-9"
    :title="`主题：${skin.label}（点击切换）`"
    :disabled="!settings"
    @click="update({ theme: next(skins, skin).value })"
  >
    <Palette class="size-[18px]" />
  </button>
  <button
    class="icon-btn size-9"
    :title="`外观：${appearance.label}（点击切换）`"
    :disabled="!settings"
    @click="update({ appearance: next(appearances, appearance).value })"
  >
    <component :is="appearance.icon" class="size-[18px]" />
  </button>
</template>
