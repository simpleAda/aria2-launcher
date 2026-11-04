<template>
  <div class="logs">
    <div class="logs-head">
      <span>共 {{ lines.length }} 行</span>
      <el-button size="small" round :icon="Delete" @click="lines = []">清空</el-button>
    </div>

    <div v-if="!lines.length" class="logs-empty">还没有日志，开始一个下载任务试试 🌸</div>

    <div v-else ref="logEl" class="logs-body">
      <div v-for="(l, i) in lines" :key="i" class="log-line">
        <span class="log-tag">[{{ tag(l.id) }}]</span>{{ l.text }}
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick } from "vue";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { Delete } from "@element-plus/icons-vue";

interface LogLine { id: string; text: string; }

const lines = ref<LogLine[]>([]);
const logEl = ref<HTMLElement | null>(null);
// 任务 id -> 链接短名（download-started 时登记）
const labels: Record<string, string> = {};
let unLog: UnlistenFn | null = null;
let unStarted: UnlistenFn | null = null;

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

function tag(id: string): string {
  const l = labels[id] ?? id;
  return l.length > 16 ? l.slice(0, 16) + "…" : l;
}

onMounted(async () => {
  unStarted = await listen<string>("download-started", (ev) => {
    const p = JSON.parse(ev.payload);
    if (p.urls?.[0]) labels[p.id] = shortLabel(p.urls[0]);
  });
  unLog = await listen<string>("download-log", (ev) => {
    const p = JSON.parse(ev.payload);
    const text = String(p.text ?? "").replace(/\r/g, "").replace(/\n$/, "");
    if (!text) return;
    lines.value.push({ id: p.id, text });
    if (lines.value.length > 800) lines.value.splice(0, lines.value.length - 800);
    nextTick(() => {
      if (logEl.value) logEl.value.scrollTop = logEl.value.scrollHeight;
    });
  });
});

onUnmounted(() => {
  unLog?.();
  unStarted?.();
});
</script>

<style scoped>
.logs { display: flex; flex-direction: column; gap: 8px; height: 100%; }
.logs-head {
  display: flex; justify-content: space-between; align-items: center;
  font-size: 12.5px; color: rgba(255, 255, 255, .85);
}
.logs-empty {
  padding: 36px 0; text-align: center;
  font-size: 13px; color: rgba(255, 255, 255, .75);
}
.logs-body {
  max-height: 320px; overflow-y: auto;
  padding: 10px 12px;
  background: rgba(20, 12, 32, .35);
  border: 1px solid rgba(255, 255, 255, .12);
  border-radius: 12px;
  font-family: Consolas, "Noto Sans SC", monospace;
  font-size: 11.5px; line-height: 1.65;
}
.log-line { color: rgba(255, 255, 255, .88); word-break: break-all; white-space: pre-wrap; }
.log-tag { color: #ffb7e0; margin-right: 6px; }
</style>
