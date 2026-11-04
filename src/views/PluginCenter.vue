<template>
  <div>
    <div class="panel">
      <div class="panel-title">插件中心 ✿</div>

      <div v-if="!plugins.length" class="empty">加载中…</div>
      <div v-for="p in plugins" :key="p.id" class="plugin-card">
        <span class="plugin-icon">{{ p.icon || "🧩" }}</span>
        <div class="plugin-main">
          <div class="plugin-name">
            {{ p.name }}
            <span class="plugin-ver">v{{ p.version || "1.0.0" }}</span>
            <span class="plugin-tag" :class="{ third: !p.builtin }">{{ p.builtin ? "内置" : "第三方" }}</span>
          </div>
          <div v-if="p.description" class="plugin-desc">{{ p.description }}</div>
          <div v-if="p.author" class="plugin-author">by {{ p.author }}</div>
        </div>
        <el-switch :model-value="p.enabled" @change="(v: string | number | boolean) => toggle(p, Boolean(v))" />
      </div>
    </div>

    <div class="panel">
      <div class="panel-title">开放目录 ✿</div>
      <div class="hint-text">
        把插件文件夹放进插件目录即可被识别。文件夹内需有 plugin.json
        （字段：id / name / version / author / description / icon / entry），entry 指向入口 html，默认 index.html。
      </div>
      <div class="row" style="margin-top: 10px">
        <el-button round :icon="FolderOpened" @click="openDir">打开插件目录</el-button>
        <el-button round :icon="Refresh" @click="load">刷新列表</el-button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ElMessage } from "element-plus";
import { FolderOpened, Refresh } from "@element-plus/icons-vue";
import { openPluginWindow, closePluginWindow } from "../pluginWins";

interface PluginRow {
  id: string;
  name: string;
  version: string;
  author: string;
  description: string;
  icon: string;
  builtin: boolean;
  enabled: boolean;
  entry_path: string;
}

const plugins = ref<PluginRow[]>([]);

async function load() {
  try {
    plugins.value = await invoke("list_plugins");
  } catch (e) {
    ElMessage.error(`加载插件列表失败：${e}`);
  }
}

async function toggle(p: PluginRow, on: boolean) {
  try {
    await invoke("set_plugin_enabled", { id: p.id, enabled: on });
    p.enabled = on;
    if (!p.builtin) {
      if (on) await openPluginWindow(p.id, p.entry_path, p.name);
      else await closePluginWindow(p.id);
    }
  } catch (e) {
    ElMessage.error(`操作失败：${e}`);
  }
}

async function openDir() {
  try {
    await invoke("open_plugins_dir");
  } catch (e) {
    ElMessage.error(String(e));
  }
}

onMounted(() => {
  load();
  // 托盘里切换速降球时，同步刷新列表里的开关状态
  listen("settings-changed", () => load());
});
</script>

<style scoped>
.empty { padding: 24px; text-align: center; font-size: 13px; color: rgba(255, 255, 255, .6); }

.plugin-card {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 14px;
  border-radius: 12px;
  background: rgba(255, 255, 255, .08);
  border: 1px solid rgba(255, 255, 255, .12);
  margin-bottom: 10px;
}
.plugin-icon { font-size: 28px; flex-shrink: 0; }
.plugin-main { flex: 1; min-width: 0; }
.plugin-name { font-size: 14px; font-weight: 600; color: #fff; display: flex; align-items: center; gap: 8px; }
.plugin-ver { font-size: 11px; color: rgba(255, 255, 255, .5); font-weight: 400; }
.plugin-tag {
  font-size: 10px;
  padding: 1px 7px;
  border-radius: 999px;
  background: rgba(232, 146, 208, .35);
  border: 1px solid rgba(232, 146, 208, .5);
}
.plugin-tag.third { background: rgba(157, 124, 232, .3); border-color: rgba(157, 124, 232, .5); }
.plugin-desc { font-size: 12px; color: rgba(255, 255, 255, .75); margin-top: 3px; }
.plugin-author { font-size: 11px; color: rgba(255, 255, 255, .45); margin-top: 2px; }

.hint-text { font-size: 12px; color: rgba(255, 255, 255, .75); line-height: 1.7; }
.row { display: flex; gap: 8px; }
</style>
