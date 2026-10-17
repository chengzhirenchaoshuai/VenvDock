<script setup lang="ts">
// 新建 / 编辑项目
import { computed, ref } from "vue";
import { Plus, X } from "@lucide/vue";
import { api, emptyProject, type Project } from "../api";
import { errorText } from "../store";
import Modal from "./Modal.vue";
import PathInput from "./PathInput.vue";
import Toggle from "./Toggle.vue";

const props = defineProps<{ project: Project | null; running: boolean }>();
const emit = defineEmits<{ close: []; saved: [] }>();

// 编辑副本（深拷贝，响应式代理不能用 structuredClone），保存成功前不影响列表
const form = ref<Project>(props.project ? JSON.parse(JSON.stringify(props.project)) : emptyProject());
const isNew = computed(() => !form.value.id);
const error = ref("");
const saving = ref(false);

async function save() {
  saving.value = true;
  error.value = "";
  try {
    await api.saveProject(form.value);
    emit("saved");
  } catch (e) {
    error.value = errorText(e);
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <Modal :title="isNew ? '添加项目' : '编辑项目'" width="max-w-2xl" @close="emit('close')">
    <form id="project-form" class="space-y-4" @submit.prevent="save">
      <div v-if="running" class="rounded-ctl bg-warn/10 px-3 py-2 text-xs text-warn">
        项目正在运行，修改将在下次启动时生效。
      </div>

      <label class="block">
        <span class="mb-1.5 block text-sm font-medium">项目名称</span>
        <input v-model="form.name" class="input" placeholder="例如：Open WebUI" autofocus />
      </label>

      <label class="block">
        <span class="mb-1.5 block text-sm font-medium">启动命令</span>
        <input
          v-model="form.command"
          class="input font-mono"
          placeholder="例如：open-webui serve、python app.py、uvicorn main:app"
          spellcheck="false"
        />
        <span class="mt-1 block text-xs text-muted">通过 cmd 执行，支持 &&、管道和 .bat。</span>
      </label>

      <div class="block">
        <span class="mb-1.5 block text-sm font-medium">虚拟环境 <span class="font-normal text-faint">可选</span></span>
        <PathInput v-model="form.venvPath" placeholder="venv 目录，如 F:\AI\OpenWebUI\venv" />
        <span class="mt-1 block text-xs text-muted">相当于运行前先激活该 venv。</span>
      </div>

      <label class="block">
        <span class="mb-1.5 block text-sm font-medium">网址 <span class="font-normal text-faint">可选</span></span>
        <input v-model="form.url" class="input font-mono" placeholder="如 http://127.0.0.1:8080" spellcheck="false" />
        <span class="mt-1 block text-xs text-muted">运行时卡片和托盘菜单里会出现“打开”。留空则从日志中自动识别本机地址。</span>
      </label>

      <div class="grid grid-cols-2 gap-4">
        <div>
          <span class="mb-1.5 block text-sm font-medium">工作目录 <span class="font-normal text-faint">可选</span></span>
          <PathInput v-model="form.workDir" placeholder="默认为 venv 的上级目录" />
        </div>
        <div>
          <span class="mb-1.5 block text-sm font-medium">日志目录 <span class="font-normal text-faint">可选</span></span>
          <PathInput v-model="form.logDir" placeholder="默认为应用数据目录" />
        </div>
      </div>

      <div>
        <div class="mb-1.5 flex items-center justify-between">
          <span class="text-sm font-medium">环境变量</span>
          <button type="button" class="btn px-2 py-1 text-xs text-accent hover:bg-accent/10" @click="form.env.push({ key: '', value: '' })">
            <Plus class="size-3.5" /> 添加
          </button>
        </div>
        <p v-if="!form.env.length" class="text-xs text-muted">未设置。可用于指定端口、模型目录等，如 PORT=8080。</p>
        <div v-for="(v, i) in form.env" :key="i" class="mb-2 flex items-center gap-2">
          <input v-model="v.key" class="input w-2/5 font-mono" placeholder="名称" spellcheck="false" />
          <span class="text-faint">=</span>
          <input v-model="v.value" class="input flex-1 font-mono" placeholder="值" spellcheck="false" />
          <button type="button" class="icon-btn shrink-0" title="移除" @click="form.env.splice(i, 1)">
            <X class="size-4" />
          </button>
        </div>
      </div>

      <div class="divide-y divide-line rounded-ctl border border-line px-4">
        <Toggle v-model="form.autoStart" label="自动启动" description="VenvDock 启动时自动启动该项目" />
        <Toggle v-model="form.autoRestart" label="自动重启" description="非手动停止的退出后，按退避策略自动重新启动" />
        <Toggle
          v-model="form.healthCheck"
          label="健康检查"
          description="每 15 秒访问一次网址（未填写则用日志识别到的地址）；服务响应过后连续 3 次无响应，就结束进程并按退避策略重启"
        />
      </div>

      <p v-if="error" class="rounded-ctl bg-fail/10 px-3 py-2 text-sm text-fail select-text">
        {{ error }}
      </p>
    </form>

    <template #footer>
      <button class="btn-secondary" @click="emit('close')">取消</button>
      <button class="btn-primary" type="submit" form="project-form" :disabled="saving">保存</button>
    </template>
  </Modal>
</template>
