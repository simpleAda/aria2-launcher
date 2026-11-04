<template>
  <div class="history">
    <div class="history-head">
      <span>共 {{ list.length }} 条记录</span>
      <el-button size="small" circle :icon="Refresh" title="刷新" @click="reload" />
    </div>

    <div v-if="!list.length" class="history-empty">还没有下载记录 🌸</div>

    <div v-for="e in list" :key="e.id + e.finished_at" class="history-item">
      <div class="row">
        <span class="name" :title="e.urls.join('\n')">{{ entryLabel(e) }}</span>
        <el-tag size="small" effect="light" :type="tagType(e.status)">{{ statusText(e.status) }}</el-tag>
      </div>
      <div class="row meta">
        <span>{{ fmtTime(e.finished_at) }}</span>
        <span class="dir" :title="e.dir">{{ e.dir }}</span>
      </div>
      <div class="row ops">
        <el-button size="small" round :icon="RefreshRight" @click="redownload(e)">重新下载</el-button>
        <el-button size="small" round :icon="FolderOpened" @click="openDir(e)">打开文件夹</el-button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ElMessage } from "element-plus";
import { Refresh, RefreshRight, FolderOpened } from "@element-plus/icons-vue";

interface HistoryEntry {
  id: string;
  urls: string[];
  dir: string;
  status: string; // done | stopped | error
  finished_at: number;
}

const list = ref<HistoryEntry[]>([]);

async function reload() {
  try {
    list.value = await invoke("list_history");
  } catch (e) {
    console.warn("list_history failed", e);
  }
}

onMounted(async () => {
  await reload();
  // 有任务结束时实时刷新
  await listen<string>("download-exit", () => { void reload(); });
});

function shortLabel(url: string): string {
  if (url.startsWith("magnet:")) return "磁力链接";
  try {
    const u = new URL(url);
    const last = u.pathname.split("/").filter(Boolean).pop();
    return last ? decodeURIComponent(last) : u.hostname;
  } catch {
    return url.length > 30 ? url.slice(0, 30) + "…" : url;
  }
}

function entryLabel(e: HistoryEntry): string {
  const base = shortLabel(e.urls[0] ?? "");
  return e.urls.length > 1 ? `${base} 等 ${e.urls.length} 个文件` : base;
}

function statusText(s: string): string {
  return s === "done" ? "已完成" : s === "stopped" ? "已停止" : "出错";
}
function tagType(s: string): "success" | "info" | "danger" {
  return s === "done" ? "success" : s === "stopped" ? "info" : "danger";
}

function fmtTime(ms: number): string {
  return new Date(ms).toLocaleString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

async function redownload(e: HistoryEntry) {
  try {
    await invoke("start_download", { urls: e.urls });
    ElMessage.success("已重新加入下载队列 (๑•̀ㅂ•́)و✧");
  } catch (err) {
    ElMessage.error(`启动失败：${err}`);
  }
}

async function openDir(e: HistoryEntry) {
  try {
    await invoke("reveal_in_folder", { dir: e.dir });
  } catch (err) {
    ElMessage.error(`打开失败：${err}`);
  }
}
</script>

<style scoped>
.history { display: flex; flex-direction: column; gap: 8px; }
.history-head {
  display: flex; justify-content: space-between; align-items: center;
  font-size: 12.5px; color: rgba(255, 255, 255, .85);
}
.history-empty {
  padding: 36px 0; text-align: center;
  font-size: 13px; color: rgba(255, 255, 255, .75);
}
.history-item {
  padding: 10px 12px;
  background: rgba(255, 255, 255, .14);
  border: 1px solid rgba(255, 255, 255, .18);
  border-radius: 12px;
  backdrop-filter: blur(8px);
  display: flex; flex-direction: column; gap: 6px;
}
.row { display: flex; align-items: center; gap: 8px; justify-content: space-between; }
.name {
  font-size: 13px; font-weight: 600; color: #fff;
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
.meta { font-size: 11.5px; color: rgba(255, 255, 255, .72); }
.dir {
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  direction: rtl; text-align: right;
}
.ops { justify-content: flex-end; }
</style>
