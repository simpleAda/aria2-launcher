<template>
  <div class="layout">
    <!-- 樱花花瓣层 -->
    <div class="sakura-layer" aria-hidden="true">
      <span v-for="n in 14" :key="n" class="petal" :style="petalStyle(n)">🌸</span>
    </div>

    <aside class="sidebar">
      <div class="brand">
        <div class="brand-icon">
          <el-icon :size="22" color="#fff"><Download /></el-icon>
        </div>
        <div>
          <div class="brand-name">aria2 启动器</div>
          <div class="brand-sub"> sakura downloader ✿</div>
        </div>
      </div>

      <el-menu :default-active="route.path" router class="menu">
        <el-menu-item index="/">
          <el-icon><VideoPlay /></el-icon><span>下载</span>
        </el-menu-item>
        <el-menu-item index="/settings">
          <el-icon><Setting /></el-icon><span>设置</span>
        </el-menu-item>
      </el-menu>

      <!-- 下载任务列表：贴在 aria2c 状态上方，从下往上展开 -->
      <div class="sidebar-task-list" v-if="tasks.length">
        <div class="sidebar-task-header">
          <span class="sidebar-task-title">下载任务</span>
          <span class="sidebar-task-count">{{ activeTaskCount }} / {{ tasks.length }}</span>
        </div>
        <div class="sidebar-task-items">
          <div
            v-for="t in tasks"
            :key="t.id"
            :class="['sidebar-task-item', { 'sidebar-task-running': t.status === 'running' }]"
          >
            <div class="sidebar-task-row">
              <div class="sidebar-task-url" :title="t.urls.join('\n')">{{ t.label }}</div>
              <span class="sidebar-task-pct">
                {{ t.progress }}%<template v-if="t.fileCount > 1"> · {{ t.fileDone }}/{{ t.fileCount }}</template>
              </span>
            </div>
            <div class="sidebar-task-row">
              <el-progress
                class="sidebar-task-bar"
                :percentage="t.progress"
                :show-text="false"
                :color="t.status === 'running' ? ['#e892d0', '#9d7ce8'] : '#c9a7e8'"
                :stroke-width="6"
              />
              <span class="sidebar-task-meta">
                <template v-if="t.status === 'running'">{{ formatSize(t.speed) }}/s</template>
                <template v-else>已结束</template>
              </span>
              <div class="sidebar-task-actions">
                <el-button
                  size="small"
                  type="danger"
                  :disabled="t.status !== 'running'"
                  @click="stopTask(t.id)"
                  :icon="Close"
                  circle
                  title="停止"
                />
              </div>
            </div>
          </div>
        </div>
      </div>

      <div class="status">
        <div class="status-card" :class="ok ? 'ok' : 'warn'">
          <span class="status-dot"></span>
          {{ ok ? "aria2c 已就绪" : "未找到 aria2c" }}
        </div>
      </div>
    </aside>

    <main class="content">
      <RouterView />
    </main>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted, computed, watch } from "vue";
import { RouterView, useRoute, useRouter } from "vue-router";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { ElMessage } from "element-plus";
import { Download, VideoPlay, Setting, Close } from "@element-plus/icons-vue";

const route = useRoute();
const router = useRouter();
const ok = ref(false);

// 生成每片花瓣的随机轨迹参数
function petalStyle(n: number) {
  const left = (n * 67) % 100;
  const delay = (n * 1.7) % 12;
  const dur = 9 + ((n * 3.1) % 6);
  const drift = ((n % 2 === 0 ? 1 : -1) * (40 + ((n * 13) % 50)));
  const scale = 0.5 + ((n * 7) % 6) / 10;
  return {
    left: left + "%",
    animationDelay: `-${delay}s`,
    animationDuration: `${dur}s`,
    "--drift": drift + "px",
    "--scale": scale,
  };
}

// ---------- 全局下载任务状态 ----------

interface Task {
  id: string;
  urls: string[];
  dir: string;
  label: string;
  status: "running" | "stopped";
  progress: number; // 0-100，批量时为整体进度
  fileCount: number;
  fileDone: number;
  speed: number; // KiB/s
  eta: number; // 秒
  peers: number;
  log: string[]; // 最近日志
}

const tasks = reactive<Task[]>([]);
const showTaskPanel = ref(false);
const collapseActive = ref<string[]>([]);

// 折叠面板随开关同步
watch(() => showTaskPanel.value, (v) => {
  collapseActive.value = v ? ["tasks"] : [];
}, { immediate: true });

const activeTaskCount = computed(() => tasks.filter((t) => t.status === "running").length);

function formatSize(kbps: number) {
  if (kbps < 1024) return kbps.toFixed(1) + " KiB";
  return (kbps / 1024).toFixed(2) + " MiB";
}

function findTask(id: string) {
  return tasks.find((t) => t.id === id);
}

// 从链接提取短名：URL 取文件名，磁力取标识，兜底截断
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

async function stopTask(id: string) {
  try {
    await invoke("stop_download", { id });
    ElMessage.info(`任务 ${id} 已停止 (｡•́︿•̀｡)`);
  } catch (e) {
    ElMessage.error(`停止失败：${e}`);
  }
}

// ---------- 事件监听 ----------

onMounted(async () => {
  // 1) 检测 aria2c
  try {
    const path: string = await invoke("find_aria2");
    ok.value = true;
    console.log("aria2c path:", path);
  } catch {
    ElMessage.warning("找不到 aria2c 呢…请到「设置」页指定路径哦 (＞﹏＜)");
    router.push("/settings");
  }

  // 2) 监听：新任务开始
  await listen<string>("download-started", (ev) => {
    const payload = JSON.parse(ev.payload);
    const { id, urls, dir } = payload;
    tasks.push({
      id,
      urls,
      dir,
      label: shortLabel(urls[0]),
      status: "running",
      progress: 0,
      fileCount: urls.length,
      fileDone: 0,
      speed: 0,
      eta: 0,
      peers: 0,
      log: [],
    });
    ElMessage.success("新下载任务已加入队列 (๑•̀ㅂ•́)و✧");
  });

  // 3) 监听：进度统计（驱动任务列表进度 + 下载页图表）
  await listen<string>("download-stats", (ev) => {
    const payload = JSON.parse(ev.payload);
    const { id, stats } = payload;
    const t = findTask(id);
    if (!t) return;
    // 批量时用 Rust 侧汇总的整体进度，单文件时与 percent 一致
    t.progress = Math.round(stats.overall_percent ?? stats.percent);
    t.fileDone = stats.files_done ?? 0;
    t.speed = stats.speed_kbps;
    t.eta = stats.eta_sec;
    t.peers = stats.peers;
  });

  // 4) 监听：日志
  await listen<string>("download-log", (ev) => {
    const payload = JSON.parse(ev.payload);
    const { id, text } = payload;
    const t = findTask(id);
    if (!t) return;
    const line = text.replace(/\r/g, "").replace(/\n$/, "");
    if (line) {
      t.log.push(line);
      if (t.log.length > 200) t.log.shift();
    }
  });

  // 5) 监听：任务结束
  await listen<string>("download-exit", (ev) => {
    const payload = JSON.parse(ev.payload);
    const { id, stopped } = payload;
    const t = findTask(id);
    if (!t) return;
    t.status = "stopped";
    // 正常完成时把进度补到 100%
    if (!stopped) {
      t.progress = 100;
      ElMessage.success(`任务 ${id} 已完成！ (＾▽＾)`);
    } else {
      ElMessage.info(`任务 ${id} 已手动停止 (｡•́︿•̀｡)`);
    }
  });
});
</script>

<style>
* { box-sizing: border-box; margin: 0; padding: 0; }
html, body, #app { height: 100%; }

body {
  font-family: "ZCOOL KuaiLe", "Noto Sans SC", "Segoe UI", "PingFang SC", "Microsoft YaHei", sans-serif;
  color: #4a3f63;
  background: linear-gradient(135deg, #fbc2eb 0%, #a6c1ee 55%, #b8a6f5 100%);
  background-attachment: fixed;
  overflow: hidden;
}

:root {
  --el-border-radius-base: 12px;
  --el-border-radius-small: 8px;
  --el-color-primary: #b58ae0;
  --el-color-primary-light-3: #c7a4ea;
  --el-color-primary-light-5: #d7bdf1;
  --el-color-primary-light-7: #e6d6f6;
  --el-color-primary-light-8: #eee1f9;
  --el-color-primary-light-9: #f6eefc;
  --el-color-primary-dark-2: #9a6cc9;
  --el-font-family: "ZCOOL KuaiLe", "Noto Sans SC", "Microsoft YaHei", sans-serif;
}

.el-button, .el-input__wrapper, .el-textarea__inner, .el-card,
.el-tag, .el-menu-item, .el-select__wrapper, .el-progress-bar__outer {
  backdrop-filter: blur(6px);
}

/* ============ 布局 ============ */
.layout { display: flex; height: 100vh; position: relative; }

/* 漫天花瓣 */
.sakura-layer { position: fixed; inset: 0; pointer-events: none; z-index: 9999; overflow: hidden; }
.petal {
  position: absolute;
  top: -32px;
  font-size: 18px;
  opacity: .38;
  --drift: 0px;
  --scale: 1;
  transform-origin: center;
  filter: drop-shadow(0 1px 2px #e91e6330);
  animation: fall linear infinite;
}
@keyframes fall {
  0%   { transform: translate(0, 0) rotate(0deg) scale(var(--scale)); }
  25%  { transform: translate(calc(var(--drift) * .3), 25vh) rotate(90deg) scale(var(--scale)); }
  50%  { transform: translate(calc(var(--drift) * -.4), 50vh) rotate(200deg) scale(var(--scale)); }
  75%  { transform: translate(calc(var(--drift) * .5), 75vh) rotate(300deg) scale(var(--scale)); }
  100% { transform: translate(calc(var(--drift) * 0), 105vh) rotate(400deg) scale(var(--scale)); }
}

/* 侧栏：毛玻璃 */
.sidebar {
  width: 210px; flex-shrink: 0; z-index: 2;
  display: flex; flex-direction: column;
  background: rgba(255, 255, 255, .35);
  backdrop-filter: blur(18px) saturate(1.4);
  border-right: 1px solid rgba(255, 255, 255, .55);
  box-shadow: 4px 0 24px #d8a8d844;
}
.brand {
  display: flex; align-items: center; gap: 10px;
  padding: 18px 16px 14px;
  border-bottom: 1px solid rgba(255, 255, 255, .5);
}
.brand-icon {
  width: 40px; height: 40px; border-radius: 14px;
  background: linear-gradient(135deg, #e892d0, #9d7ce8);
  display: flex; align-items: center; justify-content: center;
  box-shadow: 0 4px 12px #b98cd655;
  flex-shrink: 0;
}
.brand-name { font-size: 15px; color: #6b4d9e; font-weight: 700; }
.brand-sub { font-size: 10px; color: #a48ac2; margin-top: 2px; }

.menu { border-right: none !important; background: transparent !important; padding: 10px 8px; }
.menu .el-menu-item {
  color: #7c68a8 !important;
  margin: 6px 0; border-radius: 14px; height: 46px;
  transition: all .25s ease;
}
.menu .el-menu-item:hover {
  background: rgba(255, 255, 255, .55) !important;
  color: #8a5cc9 !important;
  transform: translateX(3px);
}
.menu .el-menu-item.is-active {
  background: linear-gradient(90deg, #e892d0cc, #9d7ce8cc) !important;
  color: #fff !important;
  box-shadow: 0 4px 14px #c58ee066;
  transform: translateX(3px);
}

/* 下载任务列表：从下往上堆叠 */
.sidebar-task-list {
  margin-top: auto;
  padding: 0 8px 8px;
  overflow-y: auto;
  max-height: 40vh;
}
.sidebar-task-header {
  display: flex; justify-content: space-between; align-items: center;
  padding: 6px 10px; font-size: 12px; font-weight: 700; color: #7b5aa6;
  border-bottom: 1px solid rgba(255,255,255,.5);
  margin-bottom: 6px;
}
.sidebar-task-count { color: #e892d0; }
.sidebar-task-items { display: flex; flex-direction: column-reverse; gap: 5px; overflow-x: hidden; }
.sidebar-task-item {
  display: flex; flex-direction: column; gap: 4px;
  padding: 6px 8px; border-radius: 10px;
  background: rgba(255,255,255,.45);
  border: 1px solid rgba(255,255,255,.5);
  min-width: 0;
}
.sidebar-task-item.sidebar-task-running {
  border-color: #e892d0;
  background: rgba(232,146,208,.12);
}
.sidebar-task-row { display: flex; align-items: center; gap: 6px; min-width: 0; }
.sidebar-task-url { flex: 1; min-width: 0; font-size: 11px; color: #6b4d9e; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.sidebar-task-pct { flex-shrink: 0; font-size: 11px; color: #9a8ab5; font-family: Consolas, monospace; }
.sidebar-task-bar { flex: 1; min-width: 0; }
.sidebar-task-meta { flex: 0 0 56px; font-size: 10px; color: #9a8ab5; text-align: right; overflow: hidden; white-space: nowrap; }
.sidebar-task-actions { flex-shrink: 0; display: flex; gap: 4px; }
.sidebar-task-actions .el-button { width: 20px; height: 20px; padding: 0; margin-left: 0 !important; }

/* 状态：aria2c 已就绪 */
.status { margin-top: auto; padding: 12px; }
.status-card {
  display: flex; align-items: center; gap: 8px;
  padding: 10px 12px; border-radius: 12px; font-size: 12px;
  background: rgba(255, 255, 255, .5);
  border: 1px solid rgba(255, 255, 255, .6);
}
.status-card.ok { color: #3f9e7d; }
.status-card.warn { color: #d4772f; }
.status-dot {
  width: 8px; height: 8px; border-radius: 50%;
  box-shadow: 0 0 6px currentColor;
  animation: pulse 2s ease-in-out infinite;
}
.status-card.ok .status-dot { background: #4ecfa3; }
.status-card.warn .status-dot { background: #f5a623; }
@keyframes pulse { 50% { opacity: .4; transform: scale(.85); } }

/* 内容区 */
.content {
  flex: 1; z-index: 1; position: relative;
  padding: 24px 30px; overflow-y: auto;
}

/* 毛玻璃卡片 */
.panel {
  background: rgba(255, 255, 255, .55);
  backdrop-filter: blur(20px) saturate(1.5);
  border: 1px solid rgba(255, 255, 255, .65);
  border-radius: 20px;
  padding: 20px 24px;
  box-shadow: 0 8px 32px #b78bc456, inset 0 1px 0 rgba(255, 255, 255, .8);
  margin-bottom: 20px;
}
.panel-title {
  font-size: 15px; font-weight: 700; margin-bottom: 14px;
  color: #7b5aa6; display: flex; align-items: center; gap: 8px;
}
.panel-title::before {
  content: "✿"; font-size: 14px; color: #e892d0;
}

/* 日志面板 */
.log {
  background: rgba(38, 28, 58, .82);
  color: #e8d8f8;
  font-family: Consolas, monospace;
  font-size: 12px; line-height: 1.65; padding: 14px;
  border-radius: 14px;
  border: 1px solid #ffffff26;
  backdrop-filter: blur(10px);
  height: 300px; overflow-y: auto; white-space: pre-wrap;
  word-break: break-all;
}

/* 滚动条 */
::-webkit-scrollbar { width: 8px; height: 8px; }
::-webkit-scrollbar-thumb { background: #c9a7e8aa; border-radius: 8px; }
::-webkit-scrollbar-track { background: transparent; }
</style>
