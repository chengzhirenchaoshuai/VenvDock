<script setup lang="ts">
// 全局设置
import { ref } from "vue";
import { FileCog } from "@lucide/vue";
import { api, type Settings } from "../api";
import { errorText, notify } from "../store";
import Modal from "./Modal.vue";
import Toggle from "./Toggle.vue";

const props = defineProps<{ settings: Settings }>();
const emit = defineEmits<{ close: []; saved: [] }>();

const form = ref<Settings>({ ...props.settings });
const error = ref("");

/** Settings 中数值类型的字段名 */
type NumberKey = { [K in keyof Settings]: Settings[K] extends number ? K : never }[keyof Settings];

/** 数值类设置项：字段、标题、单位、说明 */
const numberFields: { key: NumberKey; label: string; unit: string; hint?: string }[] = [
  { key: "stopTimeoutSecs", label: "停止等待", unit: "秒", hint: "发送 Ctrl+C 后等待退出的时间，超时强制结束" },
  { key: "restartDelaySecs", label: "首次重启等待", unit: "秒", hint: "之后每次翻倍" },
  { key: "restartMaxDelaySecs", label: "重启等待上限", unit: "秒" },
  { key: "maxRestartAttempts", label: "最多连续重启", unit: "次", hint: "0 表示不限" },
  { key: "logMaxSizeMb", label: "单个日志上限", unit: "MB" },
  { key: "logKeepFiles", label: "保留历史日志", unit: "份" },
];

async function save() {
  error.value = "";
  // 输入框清空时值为空字符串，按 0 提交，由后端限制到合理范围
  for (const f of numberFields) form.value[f.key] = Math.max(0, Math.floor(Number(form.value[f.key]) || 0));
  try {
    await api.updateSettings(form.value);
    emit("saved");
  } catch (e) {
    error.value = errorText(e);
  }
}

async function revealConfig() {
  try {
    await api.revealConfigFile();
  } catch (e) {
    notify(errorText(e), "error");
  }
}
</script>

<template>
  <Modal title="设置" @close="emit('close')">
    <form id="settings-form" class="space-y-5" @submit.prevent="save">
      <section class="divide-y divide-line rounded-ctl border border-line px-4">
        <Toggle v-model="form.launchOnBoot" label="开机自动启动" description="登录 Windows 后在托盘中运行 VenvDock" />
        <Toggle v-model="form.showWindowOnStart" label="开机启动时显示主窗口" description="关闭则开机后只在托盘中运行；手动打开时总会显示" />
      </section>

      <section class="grid grid-cols-2 gap-x-4 gap-y-3">
        <label v-for="f in numberFields" :key="f.key" class="block">
          <span class="mb-1 block text-sm font-medium">{{ f.label }}</span>
          <div class="relative">
            <input v-model.number="form[f.key]" type="number" min="0" class="input pr-12" />
            <span class="pointer-events-none absolute inset-y-0 right-3 flex items-center text-xs text-faint">{{ f.unit }}</span>
          </div>
          <span v-if="f.hint" class="mt-1 block text-xs text-muted">{{ f.hint }}</span>
        </label>
      </section>

      <p v-if="error" class="rounded-ctl bg-fail/10 px-3 py-2 text-sm text-fail">
        {{ error }}
      </p>
    </form>

    <template #footer>
      <button class="btn mr-auto px-2 text-muted hover:text-fg" @click="revealConfig">
        <FileCog class="size-4" /> 打开配置文件位置
      </button>
      <button class="btn-secondary" @click="emit('close')">取消</button>
      <button class="btn-primary" type="submit" form="settings-form">保存</button>
    </template>
  </Modal>
</template>
