<script setup lang="ts">
// 主页面：项目列表、添加/编辑/设置对话框、提示消息
import { computed, onMounted, ref } from "vue";
import { ask } from "@tauri-apps/plugin-dialog";
import { Boxes, Play, Plus, Settings as SettingsIcon, Square } from "@lucide/vue";
import { api, type Project } from "./api";
import { errorText, init, notify, reload, state, toasts } from "./store";
import ProjectCard from "./components/ProjectCard.vue";
import ProjectDialog from "./components/ProjectDialog.vue";
import LogViewer from "./components/LogViewer.vue";
import SettingsDialog from "./components/SettingsDialog.vue";
import ThemeSwitcher from "./components/ThemeSwitcher.vue";
import { autoCheckUpdate } from "./update";

const projects = computed(() => state.config?.projects ?? []);
const runningCount = computed(
  () => projects.value.filter((p) => state.statuses[p.id]?.phase === "running").length,
);

// 项目对话框：undefined 为关闭，null 为新建，否则为编辑
const editing = ref<Project | null | undefined>(undefined);
const showSettings = ref(false);
const viewingLog = ref<Project | null>(null);
const loadError = ref("");

onMounted(async () => {
  try {
    await init();
  } catch (e) {
    loadError.value = errorText(e);
  }
  // 每天最多自动检查一次更新，有新版本时提示
  const update = await autoCheckUpdate();
  if (update) notify(`发现新版本 v${update.latest}，可在“设置”中一键更新`);
});

async function onProjectSaved() {
  editing.value = undefined;
  await reload();
}

async function onSettingsSaved() {
  showSettings.value = false;
  await reload();
  notify("设置已保存");
}

async function runAll(action: () => Promise<void>) {
  try {
    await action();
  } catch (e) {
    notify(errorText(e), "error");
  }
}

async function remove(project: Project) {
  const running = state.statuses[project.id]?.phase === "running";
  const confirmed = await ask(
    `确定删除项目“${project.name}”吗？${running ? "\n项目正在运行，删除时将停止它。" : ""}\n日志文件会保留。`,
    { title: "删除项目", kind: "warning", okLabel: "删除", cancelLabel: "取消" },
  );
  if (!confirmed) return;
  try {
    await api.deleteProject(project.id);
    await reload();
  } catch (e) {
    notify(errorText(e), "error");
  }
}
</script>

<template>
  <div class="flex h-full flex-col">
    <header class="flex items-center justify-between border-b border-line bg-surface/70 px-6 py-4 backdrop-blur">
      <div class="flex items-center gap-3">
        <div class="flex size-9 items-center justify-center rounded-ctl text-logo-fg shadow-sm [background:var(--vd-logo-bg)]">
          <Boxes class="size-5" />
        </div>
        <div>
          <h1 class="text-base leading-tight font-semibold">VenvDock</h1>
          <p class="text-xs text-muted">
            <template v-if="projects.length">{{ runningCount }} / {{ projects.length }} 个服务运行中</template>
            <template v-else>本地服务托管面板</template>
          </p>
        </div>
      </div>
      <div class="flex items-center gap-2">
        <template v-if="projects.length">
          <button class="icon-btn size-9" title="全部启动" @click="runAll(api.startAll)">
            <Play class="size-[18px]" />
          </button>
          <button class="icon-btn size-9" title="全部停止" @click="runAll(api.stopAll)">
            <Square class="size-[18px]" />
          </button>
          <span class="mx-1 h-5 w-px bg-line"></span>
        </template>
        <ThemeSwitcher />
        <button class="icon-btn size-9" title="设置" :disabled="!state.config" @click="showSettings = true">
          <SettingsIcon class="size-[18px]" />
        </button>
        <button class="btn-primary" :disabled="!state.config" @click="editing = null">
          <Plus class="size-4" /> 添加项目
        </button>
      </div>
    </header>

    <main class="flex-1 overflow-y-auto px-6 py-5">
      <p v-if="loadError" class="rounded-ctl bg-fail/10 px-4 py-3 text-sm text-fail">
        加载失败：{{ loadError }}
      </p>

      <div v-else-if="state.config && !projects.length" class="flex h-full flex-col items-center justify-center text-center">
        <div class="mb-4 flex size-14 items-center justify-center rounded-card bg-subtle text-faint">
          <Boxes class="size-7" />
        </div>
        <p class="text-base font-medium">还没有项目</p>
        <p class="mt-1 mb-5 text-sm text-muted">添加一个服务，比如 venv 里的 Open WebUI，VenvDock 会在后台托管它。</p>
        <button class="btn-primary" @click="editing = null"><Plus class="size-4" /> 添加项目</button>
      </div>

      <div v-else class="mx-auto max-w-5xl space-y-3">
        <ProjectCard
          v-for="p in projects"
          :key="p.id"
          :project="p"
          :status="state.statuses[p.id]"
          @edit="editing = p"
          @delete="remove(p)"
          @logs="viewingLog = p"
        />
      </div>
    </main>

    <ProjectDialog
      v-if="editing !== undefined"
      :project="editing"
      :running="!!editing && state.statuses[editing.id]?.phase === 'running'"
      @close="editing = undefined"
      @saved="onProjectSaved"
    />
    <LogViewer v-if="viewingLog" :project="viewingLog" @close="viewingLog = null" />
    <SettingsDialog
      v-if="showSettings && state.config"
      :settings="state.config.settings"
      @close="showSettings = false"
      @saved="onSettingsSaved"
    />

    <!-- 提示消息 -->
    <div class="pointer-events-none fixed right-6 bottom-6 z-50 flex w-80 flex-col gap-2">
      <TransitionGroup
        enter-from-class="translate-y-2 opacity-0"
        leave-to-class="opacity-0"
        enter-active-class="transition duration-200"
        leave-active-class="transition duration-200"
      >
        <div
          v-for="t in toasts"
          :key="t.id"
          class="pointer-events-auto rounded-ctl bg-surface px-4 py-3 text-sm shadow-lg ring-1 select-text"
          :class="t.kind === 'error' ? 'text-fail ring-fail/30' : 'text-fg ring-line'"
        >
          {{ t.text }}
        </div>
      </TransitionGroup>
    </div>
  </div>
</template>
