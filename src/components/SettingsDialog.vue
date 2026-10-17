<script setup lang="ts">
// 全局设置
import { onMounted, ref } from "vue";
import { getVersion } from "@tauri-apps/api/app";
import { open, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Download, FileCog, RefreshCw, Upload } from "@lucide/vue";
import { api, type Settings } from "../api";
import { errorText, notify, reload } from "../store";
import { checkUpdate, type UpdateInfo } from "../update";
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

const JSON_FILTER = [{ name: "VenvDock 配置", extensions: ["json"] }];

async function exportConfig() {
  const path = await saveDialog({ defaultPath: "venvdock-config.json", filters: JSON_FILTER });
  if (!path) return;
  try {
    await api.exportConfig(path);
    notify("配置已导出");
  } catch (e) {
    notify(errorText(e), "error");
  }
}

/** 导入只影响项目列表，不改动当前表单中的设置 */
async function importConfig() {
  const path = await open({ filters: JSON_FILTER });
  if (!path) return;
  try {
    const { added, updated } = await api.importConfig(path);
    await reload();
    notify(`导入完成：新增 ${added} 个项目，覆盖 ${updated} 个`);
  } catch (e) {
    notify(errorText(e), "error");
  }
}

const version = ref("");
const update = ref<UpdateInfo | null>(null);
const checking = ref(false);
onMounted(async () => (version.value = await getVersion()));

async function check() {
  checking.value = true;
  try {
    update.value = await checkUpdate();
  } catch (e) {
    notify(errorText(e), "error");
  } finally {
    checking.value = false;
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
        <Toggle v-model="form.closeToTray" label="关闭时最小化到系统托盘" description="关闭此项则点击窗口关闭按钮会退出 VenvDock，并先停止所有项目" />
        <Toggle v-model="form.notifyOnFailure" label="失败时通知" description="项目异常退出或放弃自动重启时，弹出 Windows 通知" />
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

      <section class="space-y-2 rounded-ctl border border-line px-4 py-3">
        <div class="flex items-center gap-2">
          <span class="mr-auto text-sm font-medium">配置</span>
          <button type="button" class="btn-secondary py-1.5" @click="importConfig">
            <Download class="size-3.5" /> 导入项目
          </button>
          <button type="button" class="btn-secondary py-1.5" @click="exportConfig">
            <Upload class="size-3.5" /> 导出配置
          </button>
        </div>
        <p class="text-xs text-muted">导入时同一项目会被覆盖，其余作为新项目加入；不导入全局设置。可用于换电脑或从旧的便携版迁移。</p>
      </section>

      <section class="flex items-center gap-3 rounded-ctl border border-line px-4 py-3 text-sm">
        <span class="font-medium">VenvDock {{ version && `v${version}` }}</span>
        <span v-if="update && update.hasUpdate" class="text-accent">
          发现新版本 v{{ update.latest }}
          <button type="button" class="ml-1 underline" @click="openUrl(update.url)">前往下载</button>
        </span>
        <span v-else-if="update" class="text-muted">已是最新版本</span>
        <button type="button" class="btn-secondary ml-auto py-1.5" :disabled="checking" @click="check">
          <RefreshCw class="size-3.5" :class="{ 'animate-spin': checking }" /> 检查更新
        </button>
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
