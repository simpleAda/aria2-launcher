<template>
  <div class="layout" :style="glassVars">
    <!-- 背景图片层（设置页可换图、调透明度） -->
    <div class="bg-layer" :style="bgStyle" aria-hidden="true"></div>

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
        <div class="status-card" :class="[ok ? 'ok' : 'warn', { checking }]"
             title="点击重新检测 aria2c" @click="recheckAria2">
          <span class="status-dot"></span>
          {{ checking ? "检测中…" : ok ? "aria2c 已就绪" : "未找到 aria2c" }}
          <el-icon class="recheck-icon" :class="{ spin: checking }" :size="13"><Refresh /></el-icon>
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
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { ElMessage } from "element-plus";
import { Download, VideoPlay, Setting, Close, Refresh } from "@element-plus/icons-vue";

const route = useRoute();
const router = useRouter();
const ok = ref(false);
const checking = ref(false);

// 点击状态卡重新检测 aria2c（集成自原设置页的检测按钮）
async function recheckAria2() {
  if (checking.value) return;
  checking.value = true;
  try {
    await invoke("find_aria2");
    ok.value = true;
    ElMessage.success("aria2c 检测通过 ✿");
  } catch {
    ok.value = false;
    ElMessage.warning("找不到 aria2c，请到「设置」页指定路径");
  } finally {
    checking.value = false;
  }
}

// ---------- 背景图片 + 页面透明度 ----------
const bgImage = ref("");
const bgOpacity = ref(0.5);
const uiOpacity = ref(1);
const bgStyle = computed(() =>
  bgImage.value
    ? {
        backgroundImage: `url("${convertFileSrc(bgImage.value)}")`,
        opacity: bgOpacity.value,
      }
    : {}
);

function applyBg(s: { bg_image?: string; bg_opacity?: number; ui_opacity?: number }) {
  bgImage.value = s.bg_image ?? "";
  bgOpacity.value = s.bg_opacity ?? 0.5;
  uiOpacity.value = s.ui_opacity ?? 1;
}

// 玻璃色值：JS 直接算好 rgba（calc 套 var 在部分 WebView2 里不生效）
const glassVars = computed(() => {
  const a = uiOpacity.value;
  const w = (x: number) => `rgba(255,255,255,${(x * a).toFixed(3)})`;
  return {
    "--glass-panel": w(0.08),
    "--glass-sidebar": w(0.06),
    "--glass-item": w(0.1),
    "--glass-input": w(0.12),
    "--glass-btn": w(0.16),
    "--glass-log": `rgba(38,28,58,${(0.78 * a).toFixed(3)})`,
  };
});

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
  // 0) 应用已保存的背景设置，并监听设置页保存后的即时生效
  try {
    applyBg(await invoke("load_settings"));
  } catch { /* 忽略，使用默认 */ }
  await listen<string>("settings-changed", (ev) => applyBg(JSON.parse(ev.payload)));

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
  /* 白字 + 深色投影：任何壁纸上都清晰（字幕式） */
  color: #ffffff;
  text-shadow: 0 1px 4px rgba(0, 0, 0, .45);
  /* 高饱和色块铺满屏幕（含中间内容区），玻璃面板下有鲜艳颜色可透 */
  background:
    radial-gradient(80% 80% at 30% 25%, #ff9ed8 0%, transparent 78%),
    radial-gradient(75% 80% at 75% 40%, #8fc2ff 0%, transparent 78%),
    radial-gradient(85% 80% at 50% 85%, #b38cff 0%, transparent 80%),
    radial-gradient(55% 55% at 90% 90%, #8fe3d0 0%, transparent 72%),
    linear-gradient(135deg, #ffeaf5 0%, #e8f1ff 55%, #f1eaff 100%);
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

/* 小组件不再额外模糊（模糊会让其中的文字发虚） */
.el-button, .el-input__wrapper, .el-textarea__inner, .el-card,
.el-tag, .el-menu-item, .el-select__wrapper, .el-progress-bar__outer {
  backdrop-filter: none;
}

/* 输入类控件去掉实心底，融入玻璃面板 */
.el-input__wrapper, .el-textarea__inner, .el-select__wrapper {
  background: var(--glass-input) !important;
  box-shadow: 0 0 0 1px rgba(255, 255, 255, .15) inset !important;
}
.el-input-number .el-input__wrapper { background: transparent !important; }
.el-input-number { background: var(--glass-input); border-radius: 12px; }
/* 标签幽灵化：透明底 + 荧光字 */
.el-tag {
  background: rgba(255, 255, 255, .14) !important;
  border-color: rgba(255, 255, 255, .25) !important;
}
.el-tag--success { color: #7ce8bd !important; }
.el-tag--danger { color: #ff9ba8 !important; }

/* 数字框的 −/＋ 按钮去掉白底，融入整体 */
.el-input-number__decrease, .el-input-number__increase {
  background: transparent !important;
  color: rgba(255, 255, 255, .85) !important;
  border-color: rgba(255, 255, 255, .15) !important;
}
.el-input-number__decrease:hover, .el-input-number__increase:hover { color: #ffb3e2 !important; }
/* 滑块：细跑道 + 淡填充，尽量隐形 */
.el-slider__runway { height: 4px; background: rgba(255, 255, 255, .08); }
.el-slider__bar { height: 4px; background: rgba(232, 146, 208, .5); }
.el-slider__button-wrapper { top: -15px; }
.el-slider__button { width: 12px; height: 12px; border: 2px solid rgba(255, 255, 255, .9); background: #e892d0; }

/* 普通按钮去掉实心底（渐变主按钮不受影响，它有更高优先级的自己的渐变） */
.el-button {
  background: var(--glass-btn) !important;
  border-color: rgba(255, 255, 255, .25) !important;
  color: #fff !important;
}
.el-button.is-text, .el-button.is-link { background: transparent !important; }

/* 弹出层（下拉/提示框）玻璃化：EP 用变量上白底，直接改变量最可靠 */
:root {
  /* 弹出层统一透明玻璃（下拉、滑块气泡、提示框共用这两个变量） */
  --el-bg-color-overlay: rgba(255, 255, 255, .45);
  --el-popper-bg-color-light: rgba(255, 255, 255, .45);
  /* EP 文字颜色同步成白色系（标签、输入文字、占位符） */
  --el-text-color-primary: #ffffff;
  --el-text-color-regular: rgba(255, 255, 255, .92);
  --el-text-color-secondary: rgba(255, 255, 255, .75);
  --el-text-color-placeholder: rgba(255, 255, 255, .55);
  --el-border-color: rgba(255, 255, 255, .3);
  --el-border-color-light: rgba(255, 255, 255, .22);
  --el-border-color-lighter: rgba(255, 255, 255, .16);
}
.el-popper.is-light, .el-select__popper.el-popper {
  backdrop-filter: blur(20px) saturate(1.5);
  border: 1px solid rgba(255, 255, 255, .18) !important;
  border-radius: 14px !important;
  box-shadow: 0 8px 28px rgba(20, 10, 40, .35) !important;
}
/* 弹出层真玻璃：透壁纸 + 模糊兜底，箭头同步 */
.el-select__popper.el-popper.is-light {
  backdrop-filter: blur(16px) saturate(1.4);
  border: 1px solid rgba(255, 255, 255, .4) !important;
}
.el-popper.is-light .el-popper__arrow:before { background: rgba(255, 255, 255, .45); border-color: rgba(255, 255, 255, .4); }
/* 浅玻璃底上用深字才清楚 */
.el-select-dropdown__item { color: #4a3a5c; }
.el-select-dropdown__item.is-hovering { background: rgba(255, 255, 255, .4); }
.el-select-dropdown__item.is-selected { color: #d669a8; font-weight: 600; }

/* 消息提示框同样用浅玻璃 + 深字（共用 overlay 变量，会被染成浅底） */
.el-message {
  background: rgba(255, 255, 255, .5) !important;
  backdrop-filter: blur(12px) saturate(1.4);
  border-color: rgba(255, 255, 255, .5) !important;
}
.el-message .el-message__content { color: #4a3a5c !important; }

/* 滑块百分比气泡：透明玻璃 + 深色粗体字（两条滑块同一类名，一并生效）
   注意必须带 .is-light 才能拼过 EP 自带的双类规则 */
.el-tooltip__popper.is-light, .el-slider__popper.is-light,
.el-tooltip__popper.is-dark, .el-slider__popper.is-dark {
  background: rgba(255, 255, 255, .5) !important;
  backdrop-filter: blur(12px) saturate(1.4);
  color: #4a3a5c !important;
  font-weight: 700;
  border: 1px solid rgba(255, 255, 255, .5) !important;
}
.el-tooltip__popper.is-light .el-popper__arrow:before,
.el-slider__popper.is-light .el-popper__arrow:before,
.el-tooltip__popper.is-dark .el-popper__arrow:before,
.el-slider__popper.is-dark .el-popper__arrow:before {
  background: rgba(255, 255, 255, .5) !important;
  border-color: rgba(255, 255, 255, .5) !important;
}

/* ============ 布局 ============ */
.layout { display: flex; height: 100vh; position: relative; }

/* 背景图片层：垫在最底下，樱花和界面都在其上 */
/* contain 完整展示图片不裁切，留白处透出底层渐变 */
.bg-layer {
  position: fixed; inset: 0; z-index: 0; pointer-events: none;
  background-size: contain; background-position: center; background-repeat: no-repeat;
}

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
  /* 悬浮圆角卡片式导航栏，与面板风格统一 */
  margin: 12px 0 12px 12px;
  border-radius: 20px;
  background: var(--glass-sidebar);
  backdrop-filter: blur(16px) saturate(1.5) brightness(1.08);
  border: 1px solid rgba(255, 255, 255, .12);
}
.brand {
  display: flex; align-items: center; gap: 10px;
  padding: 18px 16px 14px;
  border-bottom: 1px solid rgba(255, 255, 255, .12);
}
.brand-icon {
  width: 40px; height: 40px; border-radius: 14px;
  background: linear-gradient(135deg, #e892d0, #9d7ce8);
  display: flex; align-items: center; justify-content: center;
  box-shadow: 0 4px 12px #b98cd655;
  flex-shrink: 0;
}
.brand-name { font-size: 15px; color: #fff; font-weight: 700; }
.brand-sub { font-size: 10px; color: rgba(255, 255, 255, .65); margin-top: 2px; }

.menu { border-right: none !important; background: transparent !important; padding: 10px 8px; }
.menu .el-menu-item {
  color: rgba(255, 255, 255, .85) !important;
  margin: 6px 0; border-radius: 14px; height: 46px;
  transition: all .25s ease;
}
.menu .el-menu-item:hover {
  background: rgba(255, 255, 255, .18) !important;
  color: #fff !important;
  transform: translateX(3px);
}
.menu .el-menu-item.is-active {
  background: rgba(232, 146, 208, .28) !important;
  border: 1px solid rgba(255, 255, 255, .25);
  color: #fff !important;
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
  padding: 6px 10px; font-size: 12px; font-weight: 700; color: #fff;
  border-bottom: 1px solid rgba(255,255,255,.12);
  margin-bottom: 6px;
}
.sidebar-task-count { color: #e892d0; }
.sidebar-task-items { display: flex; flex-direction: column-reverse; gap: 5px; overflow-x: hidden; }
.sidebar-task-item {
  display: flex; flex-direction: column; gap: 4px;
  padding: 6px 8px; border-radius: 10px;
  background: var(--glass-item);
  border: 1px solid rgba(255,255,255,.55);
  min-width: 0;
}
.sidebar-task-item.sidebar-task-running {
  border-color: #e892d0;
  background: rgba(232,146,208,.12);
}
.sidebar-task-row { display: flex; align-items: center; gap: 6px; min-width: 0; }
.sidebar-task-url { flex: 1; min-width: 0; font-size: 11px; color: #fff; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.sidebar-task-pct { flex-shrink: 0; font-size: 11px; color: rgba(255,255,255,.75); font-family: Consolas, monospace; }
.sidebar-task-bar { flex: 1; min-width: 0; }
.sidebar-task-meta { flex: 0 0 56px; font-size: 10px; color: rgba(255,255,255,.75); text-align: right; overflow: hidden; white-space: nowrap; }
.sidebar-task-actions { flex-shrink: 0; display: flex; gap: 4px; }
.sidebar-task-actions .el-button { width: 20px; height: 20px; padding: 0; margin-left: 0 !important; }

/* 状态：aria2c 已就绪 */
.status { margin-top: auto; padding: 12px; }
.status-card {
  display: flex; align-items: center; gap: 8px;
  padding: 10px 12px; border-radius: 12px; font-size: 12px;
  background: var(--glass-item);
  border: 1px solid rgba(255, 255, 255, .15);
  cursor: pointer; user-select: none;
  transition: all .25s ease;
}
.status-card:hover { background: rgba(255, 255, 255, .18); transform: translateY(-1px); }
.status-card:active { transform: scale(.96); }
.recheck-icon { margin-left: auto; opacity: .55; transition: opacity .25s; }
.status-card:hover .recheck-icon { opacity: 1; }
.recheck-icon.spin { animation: recheck-spin 1s linear infinite; opacity: 1; }
@keyframes recheck-spin { to { transform: rotate(360deg); } }
.status-card.ok { color: #7ce8bd; }
.status-card.warn { color: #ffc069; }
.status-dot {
  width: 8px; height: 8px; border-radius: 50%;
  box-shadow: 0 0 6px currentColor;
  animation: pulse 2s ease-in-out infinite;
}
.status-card.ok .status-dot { background: #4ecfa3; }
.status-card.warn .status-dot { background: #f5a623; }
@keyframes pulse { 50% { opacity: .4; transform: scale(.85); } }

/* 内容区：居中列布局，两侧对称露出壁纸 */
.content {
  flex: 1; z-index: 1; position: relative;
  padding: 24px 30px; overflow-y: auto;
}
.content > * {
  max-width: 860px;
  margin-left: auto;
  margin-right: auto;
}

/* 毛玻璃卡片 */
.panel {
  background: var(--glass-panel);
  backdrop-filter: blur(16px) saturate(1.5) brightness(1.08);
  border: 1px solid rgba(255, 255, 255, .1);
  border-radius: 20px;
  padding: 20px 24px;
  margin-bottom: 20px;
}
.panel-title {
  font-size: 15px; font-weight: 700; margin-bottom: 14px;
  color: #fff; display: flex; align-items: center; gap: 8px;
}
.panel-title::before {
  content: "✿"; font-size: 14px; color: #e892d0;
}

/* 日志面板：深色玻璃要够深，浅色文字才清晰 */
.log {
  background: var(--glass-log);
  backdrop-filter: blur(14px) saturate(1.3);
  color: #f6edff;
  text-shadow: 0 1px 2px rgba(0, 0, 0, .35);
  font-family: Consolas, monospace;
  font-size: 12px; line-height: 1.65; padding: 14px;
  border-radius: 14px;
  border: 1px solid #ffffff26;
  backdrop-filter: blur(10px);
  height: 300px; overflow-y: auto; white-space: pre-wrap;
  word-break: break-all;
}

/* 滚动条：透明玻璃风 */
::-webkit-scrollbar { width: 6px; height: 6px; }
::-webkit-scrollbar-thumb { background: rgba(255, 255, 255, .22); border-radius: 6px; }
::-webkit-scrollbar-thumb:hover { background: rgba(255, 255, 255, .4); }
::-webkit-scrollbar-track { background: transparent; }
</style>
