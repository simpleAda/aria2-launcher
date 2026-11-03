<template>
  <div>
    <div class="panel">
      <div class="panel-title">新建下载 ✿</div>

      <el-input
        v-model="links"
        type="textarea"
        :rows="4"
        placeholder="https://example.com/file.iso&#10;magnet:?xt=urn:btih:...&#10;支持每行一个链接，批量下载"
        style="margin-bottom: 16px"
      />

      <div class="actions">
        <el-button
          type="primary"
          size="large"
          round
          class="pink-btn"
          :icon="VideoPlay"
          :disabled="!links.trim()"
          @click="start"
        >
          开始下载
        </el-button>
        <el-button size="large" round :icon="Delete" @click="clearLog">清空日志</el-button>
      </div>
    </div>

    <div class="panel">
      <div class="panel-title">下载速度 ✿</div>
      <div class="chart-wrap" ref="chartEl" style="width: 100%; height: 200px"></div>
    </div>

    <div class="panel" style="padding-bottom: 14px">
      <div class="panel-title">运行日志 ✿</div>
      <div class="log" ref="logEl">{{ logText }}</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted, onUnmounted, nextTick, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { useRouter } from "vue-router";
import { ElMessage } from "element-plus";
import { VideoPlay, Delete } from "@element-plus/icons-vue";
// 按需引入，冷启动不加载整个 echarts
import * as echarts from "echarts/core";
import { LineChart } from "echarts/charts";
import { GridComponent, TooltipComponent, LegendComponent } from "echarts/components";
import { CanvasRenderer } from "echarts/renderers";
echarts.use([LineChart, GridComponent, TooltipComponent, LegendComponent, CanvasRenderer]);

const links = ref("");
const logEl = ref<HTMLElement | null>(null);
const chartEl = ref<HTMLElement | null>(null);

const router = useRouter();

const logText = ref("");
let unlistenLog: UnlistenFn | null = null;
let unlistenStats: UnlistenFn | null = null;
let unlistenStarted: UnlistenFn | null = null;
let chart: ReturnType<typeof echarts.init> | null = null;

// 当前关注的任务 id（用于日志过滤）
const currentTaskId = ref<string | null>(null);

// 下载速度/ETA 时序数据：按任务 id 分组，每个任务不同颜色
const chartDatasets = reactive<Record<string, { time: string[]; speed: number[]; eta: number[] }>>({});

// 任务 id -> 链接短名（图表图例用，与侧栏一致）
const taskLabels: Record<string, string> = {};

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

function labelOf(id: string): string {
  const l = taskLabels[id] ?? id;
  return l.length > 14 ? l.slice(0, 14) + "…" : l;
}

const taskColors: Record<string, string> = {};
const colorPalette = ["#e892d0", "#9d7ce8", "#ff9ff3", "#a29bfe", "#81ecec", "#55efc4", "#ffeaa7", "#fab1a0"];
let colorIndex = 0;

function getColorForTask(id: string): string {
  if (!taskColors[id]) {
    taskColors[id] = colorPalette[colorIndex % colorPalette.length];
    colorIndex++;
  }
  return taskColors[id];
}

async function start() {
  const items = links.value.split(/\r?\n/).map((s) => s.trim()).filter(Boolean);
  if (!items.length) return;

  try {
    logText.value = "";
    currentTaskId.value = null;
    nextTick(() => { updateChart(); });

    // Rust 侧返回 task_id
    const id: string = await invoke("start_download", { urls: items });
    currentTaskId.value = id;
    ElMessage.success("新下载任务已启动 (๑•̀ㅂ•́)و✧");
  } catch (e) {
    ElMessage.error(`启动失败：${e}`);
  }
}

function clearLog() {
  logText.value = "";
}

function pushLog(id: string, text: string) {
  // 只显示当前关注任务的日志（或没有指定任务时显示最近一条）
  if (currentTaskId.value && currentTaskId.value !== id) return;
  const line = text.replace(/\r/g, "").replace(/\n$/, "");
  if (!line) return;
  logText.value = logText.value ? logText.value + "\n" + line : line;
  nextTick(() => {
    if (logEl.value) logEl.value.scrollTop = logEl.value.scrollHeight;
  });
}

function initChart() {
  if (!chartEl.value) return;
  chart = echarts.init(chartEl.value);
  updateChart();
}

function updateChart() {
  if (!chart || !chartEl.value) return;

  const ids = Object.keys(chartDatasets);
  const allTimes = new Set<string>();
  for (const id of ids) {
    for (const t of chartDatasets[id].time) allTimes.add(t);
  }
  // 时间轴按时间顺序排序
  const timeAxis = Array.from(allTimes).sort();

  const speedSeries: any[] = [];
  const etaSeries: any[] = [];
  let hasData = false;

  for (const id of ids) {
    const d = chartDatasets[id];
    const label = labelOf(id);
    const color = getColorForTask(id);
    const speedMap = new Map<string, number>();
    for (let i = 0; i < d.time.length; i++) speedMap.set(d.time[i], d.speed[i]);
    const etaMap = new Map<string, number>();
    for (let i = 0; i < d.time.length; i++) etaMap.set(d.time[i], d.eta[i]);

    if (d.speed.some((v) => v > 0)) hasData = true;

    speedSeries.push({
      name: `速度 - ${label}`,
      type: "line",
      smooth: true,
      yAxisIndex: 0,
      itemStyle: { color },
      lineStyle: { color, width: 2 },
      areaStyle: {
        color: new (echarts as any).graphic.LinearGradient(0, 0, 0, 1, [
          { offset: 0, color: `${color}99` },
          { offset: 1, color: `${color}1a` },
        ]),
      },
      data: timeAxis.map((t) => speedMap.get(t) ?? 0),
      z: 2,
    });
    etaSeries.push({
      name: `ETA - ${label}`,
      type: "line",
      smooth: true,
      yAxisIndex: 1,
      itemStyle: { color },
      lineStyle: { color, width: 2, type: "dashed" },
      data: timeAxis.map((t) => etaMap.get(t) ?? 0),
      z: 1,
    });
  }

  chart.setOption({
    backgroundColor: "rgba(255,255,255,0)",
    tooltip: { trigger: "axis", backgroundColor: "rgba(255,255,255,0.9)", textStyle: { color: "#4a3f63" } },
    grid: { left: 40, right: 30, top: 20, bottom: 80 },
    xAxis: {
      type: "category",
      data: timeAxis,
      axisLine: { lineStyle: { color: "#c9a7e8" } },
      axisLabel: { color: "#9a8ab5", fontSize: 9 },
      splitLine: { show: false },
    },
    yAxis: [
      {
        type: "value",
        name: "速度 (KiB/s)",
        nameLocation: "end",
        nameTextStyle: { color: "#e892d0", fontSize: 10 },
        axisLine: { lineStyle: { color: "#e892d0" } },
        axisLabel: { color: "#e892d0", fontSize: 9 },
        splitLine: { lineStyle: { color: "#f3e8fb", type: "dashed" } },
        min: 0,
      },
      {
        type: "value",
        name: "ETA (s)",
        nameLocation: "end",
        nameTextStyle: { color: "#9d7ce8", fontSize: 10 },
        axisLine: { lineStyle: { color: "#9d7ce8" } },
        axisLabel: { color: "#9d7ce8", fontSize: 9 },
        splitLine: { show: false },
        min: 0,
      },
    ],
    series: [...speedSeries, ...etaSeries],
    animation: !hasData ? false : true,
    legend: {
      show: ids.length > 0,
      textStyle: { color: "#9a8ab5", fontSize: 10 },
      bottom: 10,
      data: ids.flatMap((id) => [`速度 - ${labelOf(id)}`, `ETA - ${labelOf(id)}`]),
    },
  });

  if (!hasData && timeAxis.length === 0) {
    chart.setOption({
      graphic: [
        {
          type: "text",
          left: "center",
          top: "center",
          style: {
            text: "下载开始后会在这里显示速度曲线 ✿",
            fill: "#c9a7e8",
            font: "14px 'ZCOOL KuaiLe'",
          },
        },
      ],
    });
  }
}

function resizeChart() {
  nextTick(() => { chart?.resize(); });
}

onMounted(async () => {
  // 1) 初始化图表（必须在 DOM 挂载后）
  initChart();

  // 窗口缩放时重绘图表
  window.addEventListener("resize", resizeChart);

  // 2) 新任务开始：记录当前任务 id（供图表/日志过滤）
  unlistenStarted = await listen<string>("download-started", (ev) => {
    const payload = JSON.parse(ev.payload);
    currentTaskId.value = payload.id;
    if (payload.urls?.[0]) taskLabels[payload.id] = shortLabel(payload.urls[0]);
    // 为新任务准备数据集
    if (!chartDatasets[payload.id]) {
      chartDatasets[payload.id] = { time: [], speed: [], eta: [] };
    }
    logText.value = "";
    nextTick(() => { updateChart(); });
  });

  // 3) 进度统计：驱动图表（所有任务都显示）
  unlistenStats = await listen<string>("download-stats", (ev) => {
    const payload = JSON.parse(ev.payload);
    const { id, stats } = payload;
    if (!chartDatasets[id]) {
      chartDatasets[id] = { time: [], speed: [], eta: [] };
    }
    const d = chartDatasets[id];
    const now = new Date();
    const timeStr = `${now.getHours()}:${String(now.getMinutes()).padStart(2, "0")}:${String(now.getSeconds()).padStart(2, "0")}`;
    d.time.push(timeStr);
    d.speed.push(stats.speed_kbps || 0);
    d.eta.push(stats.eta_sec || 0);
    if (d.time.length > 40) {
      d.time.splice(0, d.time.length - 40);
      d.speed.splice(0, d.speed.length - 40);
      d.eta.splice(0, d.eta.length - 40);
    }
    nextTick(() => { updateChart(); });
  });

  // 4) 日志：驱动日志面板
  unlistenLog = await listen<string>("download-log", (ev) => {
    const payload = JSON.parse(ev.payload);
    pushLog(payload.id, payload.text);
  });

  try {
    await invoke("find_aria2");
  } catch {
    router.push("/settings");
  }
});

onUnmounted(() => {
  window.removeEventListener("resize", resizeChart);
  unlistenStarted?.();
  unlistenStats?.();
  unlistenLog?.();
  if (chart) chart.dispose();
});
</script>

<style scoped>
.actions { display: flex; gap: 4px; align-items: center; flex-wrap: wrap; }

/* 主按钮：淡粉玻璃，与幽灵风统一 */
.pink-btn {
  background: rgba(232, 146, 208, .28) !important;
  border: 1px solid rgba(255, 255, 255, .3) !important;
  color: #fff !important;
  transition: all .2s ease !important;
}
.pink-btn:hover:not(:disabled) {
  background: rgba(232, 146, 208, .42) !important;
  transform: translateY(-1px);
}
.pink-btn:active:not(:disabled) { transform: translateY(0) scale(.98); }

.chart-wrap { margin-top: 8px; }
</style>
