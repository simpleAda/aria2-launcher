<script setup lang="ts">
import { ref, reactive, computed, watch, onMounted, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { PhysicalPosition, LogicalSize } from "@tauri-apps/api/window";

// 纯浏览器预览（pnpm dev 直接开 ?win=dropball）时 Tauri API 不存在，全部降级为 no-op
const inTauri = "__TAURI_INTERNALS__" in window;
const win = inTauri ? getCurrentWebviewWindow() : null;

const cheer = ref(false);     // 收到链接：轻轻蹦一下
const loadError = ref("");
const canvasRef = ref<HTMLCanvasElement>();
const speeds = reactive<Record<string, number>>({});
let cheerTimer = 0;

// 头顶对话框
type BubbleMode = "hot" | "cheer" | "status" | "done" | "error";
const bubbleMode = ref<BubbleMode>("status");
const bubbleShow = ref(false);
const bubbleMsg = ref("");
let bubbleSticky = false;
let hideTimer = 0;

// Live2D 实例（any 避开动态 import 的类型摩擦）
/* eslint-disable @typescript-eslint/no-explicit-any */
let pixiApp: any = null;
let model: any = null;
let lastTapMotion = 0;

// 窗口尺寸：上面留出对话框的位置，模型固定在底部，永不动摇
const WIN_W = 150;
const WIN_H = 134;
// 取景参数：1.0 = 完整显示整个半身形象，不裁切；>1 则放大裁切
const HEAD_ZOOM = 1.0;
const HEAD_OFFSET_Y = 0;

const activeCount = computed(() => Object.keys(speeds).length);
const totalSpeed = computed(() => Object.values(speeds).reduce((a, b) => a + b, 0));
function fmtSpeed(kib: number): string {
  return kib >= 1024 ? (kib / 1024).toFixed(1) + " MB/s" : Math.round(kib) + " KB/s";
}

const bubbleText = computed(() => {
  switch (bubbleMode.value) {
    case "hot": return "松手就开始下载 ~";
    case "cheer": return "收到，开始下载啦 ✿";
    case "done": return "全部下载完成 ✿";
    case "error": return bubbleMsg.value;
    default: return `下载中 ${activeCount.value} 个 · ${fmtSpeed(totalSpeed.value)}`;
  }
});

// 定时隐藏；过渡性气泡（庆祝/完成/出错）超时后若还有任务则回落为状态气泡
function armHide(ms: number) {
  window.clearTimeout(hideTimer);
  hideTimer = window.setTimeout(() => {
    if (bubbleMode.value !== "status" && activeCount.value > 0) {
      bubbleMode.value = "status";
      armHide(6000);
    } else {
      bubbleShow.value = false;
      bubbleSticky = false;
    }
  }, ms);
}
function showBubble(mode: BubbleMode, ms = 6000, sticky = false, msg = "") {
  bubbleMode.value = mode;
  bubbleMsg.value = msg;
  bubbleShow.value = true;
  bubbleSticky = sticky;
  if (sticky) window.clearTimeout(hideTimer);
  else armHide(ms);
}

// 新任务出现 → 弹状态气泡（6s 后自动消失）；全部完成 → 撒花气泡
watch(activeCount, (n, o) => {
  if (n > 0 && o === 0) showBubble("status");
  else if (n === 0 && o > 0) showBubble("done", 4000);
});

// 鼠标移到球上 → 有任务时重新弹出状态气泡；移走 2.5s 后收起
function onStageEnter() {
  if (activeCount.value > 0) showBubble("status", 0, true);
}
function onStageLeave() {
  if (bubbleSticky) armHide(2500);
}

// 只有左键拖动球体
function onPointerDown(e: PointerEvent) {
  if (e.button === 0) void win?.startDragging();
}

function extractUrls(text: string): string[] {
  return text.split(/[\s"'<>]+/).filter((s) => /^(https?:\/\/|magnet:\?)/i.test(s));
}

async function onDrop(e: DragEvent) {
  const text = e.dataTransfer?.getData("text/uri-list") || e.dataTransfer?.getData("text/plain") || "";
  const urls = extractUrls(text);
  if (!urls.length) {
    showBubble("error", 3000, false, "没有识别到下载链接");
    return;
  }
  try {
    if (inTauri) await invoke("start_download", { urls });
    cheer.value = true;
    playMotion("Tap");                          // 高兴动作
    showBubble("cheer", 2600);
    window.clearTimeout(cheerTimer);
    cheerTimer = window.setTimeout(() => (cheer.value = false), 800);
  } catch (err) {
    showBubble("error", 3000, false, "启动失败：" + err);
  }
}

// 拖拽中的链接移动时，让她一直盯着链接看，并用气泡提示
function onDragOver(e: DragEvent) {
  showBubble("hot", 0, true);
  try { model?.focus(e.clientX, e.clientY); } catch { /* 忽略 */ }
}
function onDragLeave() {
  if (activeCount.value > 0) showBubble("status", 3000);
  else { bubbleShow.value = false; bubbleSticky = false; }
}

function playMotion(group: string) {
  try { model?.motion(group); } catch { /* 动作组不存在时忽略 */ }
}

// Live2D Cubism Core 是非模块化脚本，运行时注入（只在这个小窗口里加载）
async function ensureCubismCore() {
  if ((window as unknown as { Live2DCubismCore?: unknown }).Live2DCubismCore) return;
  await new Promise<void>((resolve, reject) => {
    const s = document.createElement("script");
    s.src = "/live2d/live2dcubismcore.min.js";
    s.onload = () => resolve();
    s.onerror = () => reject(new Error("live2dcubismcore.min.js 加载失败"));
    document.head.appendChild(s);
  });
}

async function setupLive2D() {
  await ensureCubismCore();
  const [PIXI, { Live2DModel }] = await Promise.all([
    import("pixi.js"),
    // 只走 Cubism4 子入口：全量入口还会强检 Cubism2 的 live2d.min.js（我们用不到）
    import("pixi-live2d-display/cubism4"),
  ]);
  (window as unknown as { PIXI?: unknown }).PIXI = PIXI;

  pixiApp = new PIXI.Application({
    view: canvasRef.value!,
    width: 88,
    height: 92,
    backgroundAlpha: 0,
    antialias: true,
    resolution: window.devicePixelRatio || 1,
    autoDensity: true,
    preserveDrawingBuffer: true,   // 便于调试用 toDataURL 读取画布内容
  });

  model = await Live2DModel.from("/live2d/haru/haru_greeter_t03.model3.json", {
    autoInteract: true,   // 自动视线跟随 + 命中检测
  });

  pixiApp.stage.addChild(model);
  model.autoUpdate = true;

  // 取景：以实际渲染包围盒为准（这个库的 width/height 会虚报）
  let b = model.getBounds();
  const k = (92 * HEAD_ZOOM) / b.height;
  model.scale.set(model.scale.x * k);
  b = model.getBounds();
  model.x += (88 - b.width) / 2 - b.x;      // 水平居中
  model.y += -b.height * HEAD_OFFSET_Y - b.y;
  playMotion("Idle");

  // 点头/点身体 → 随机动作（带节流，防止连点鬼畜）
  model.on("hit", () => {
    const now = Date.now();
    if (now - lastTapMotion < 800) return;
    lastTapMotion = now;
    playMotion("Tap");
  });
}

onMounted(async () => {
  if (win) {
    await win.setSize(new LogicalSize(WIN_W, WIN_H));

    // 位置记忆
    const px = Number(localStorage.getItem("drop-ball-x"));
    const py = Number(localStorage.getItem("drop-ball-y"));
    if (px || py) await win.setPosition(new PhysicalPosition(px, py));
    await win.onMoved(({ payload }) => {
      localStorage.setItem("drop-ball-x", String(payload.x));
      localStorage.setItem("drop-ball-y", String(payload.y));
    });

    // 与主界面同一事件流：payload 是 JSON 字符串，速度在 stats.speed_kbps（KiB/s）
    await listen<string>("download-stats", (ev) => {
      const { id, stats } = JSON.parse(ev.payload);
      if (id && stats) speeds[id] = stats.speed_kbps ?? 0;
    });
    await listen<string>("download-exit", (ev) => {
      const { id } = JSON.parse(ev.payload);
      if (id) delete speeds[id];
    });
  }

  try {
    await setupLive2D();
  } catch (e) {
    loadError.value = String(e);
    console.error("Live2D 初始化失败", e);
  }
});

onUnmounted(() => {
  window.clearTimeout(cheerTimer);
  window.clearTimeout(hideTimer);
  try {
    pixiApp?.destroy(true, { children: true, texture: true, baseTexture: true });
  } catch { /* 忽略 */ }
});
</script>

<template>
  <div class="ball-stage" @pointerenter="onStageEnter" @pointerleave="onStageLeave">
    <!-- 头顶对话框：固定槽位，显隐不影响模型位置 -->
    <div class="bubble-slot">
      <div class="bubble" :class="{ off: !bubbleShow }">{{ bubbleText }}</div>
    </div>

    <div class="mascot" :class="{ cheer }"
         title="拖链接到这里开始下载"
         @pointerdown="onPointerDown"
         @contextmenu.prevent
         @dragover.prevent="onDragOver"
         @dragleave="onDragLeave"
         @drop.prevent="onDrop">
      <canvas ref="canvasRef" class="live2d-canvas"></canvas>
      <div v-if="loadError" class="load-error" :title="loadError">加载失败</div>
      <i v-if="activeCount" class="ball-badge">{{ activeCount }}</i>
    </div>
  </div>
</template>

<style>
html.ball-mode, html.ball-mode body { background: transparent !important; }
html.ball-mode body { margin: 0; overflow: hidden; }
html.ball-mode #app { height: 100vh; overflow: hidden; }

.ball-stage {
  width: 100vw; height: 100vh; box-sizing: border-box; overflow: hidden;
  display: flex; flex-direction: column; align-items: center;
  padding-top: 2px;
  font-family: "Noto Sans SC", sans-serif;
}

/* 头顶对话框：槽位固定，只做淡入淡出 + 轻微上浮 */
.bubble-slot {
  height: 40px; flex: none;
  display: flex; align-items: flex-end; justify-content: center;
}
.bubble {
  position: relative;
  max-width: 138px; padding: 5px 10px;
  font-size: 11.5px; line-height: 1.4; color: #4a3a5c;
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  background: rgba(255, 255, 255, .72);
  backdrop-filter: blur(14px) saturate(1.6);
  border: 1px solid rgba(255, 255, 255, .55);
  border-radius: 11px;
  box-shadow: 0 6px 18px rgba(60, 30, 90, .18);
  opacity: 1; transform: translateY(0);
  transition: opacity .25s ease, transform .25s ease;
}
.bubble::after {
  content: ""; position: absolute; left: 50%; bottom: -5px;
  transform: translateX(-50%);
  border: 5px solid transparent; border-bottom: 0;
  border-top-color: rgba(255, 255, 255, .72);
}
.bubble.off { opacity: 0; transform: translateY(5px); pointer-events: none; }

/* 角色本体：无玻璃球、无背景，透明 die-cut */
.mascot {
  position: relative;
  width: 88px; height: 92px; flex: none;
  cursor: grab; user-select: none; touch-action: none;
  filter: drop-shadow(0 6px 14px rgba(60, 30, 90, .35));
}
.mascot:active { cursor: grabbing; }
.live2d-canvas { width: 100%; height: 100%; display: block; }

/* 收到链接：轻轻蹦一下（只位移，不缩放） */
.mascot.cheer { animation: cheer-bounce .8s ease; }
@keyframes cheer-bounce {
  0%, 100% { transform: translateY(0); }
  35%      { transform: translateY(-7px); }
  65%      { transform: translateY(0); }
  82%      { transform: translateY(-3px); }
}

.load-error {
  position: absolute; inset: 0;
  display: flex; align-items: center; justify-content: center;
  font-size: 11px; color: #fff; text-align: center;
  background: rgba(200, 80, 120, .55); border-radius: 12px;
}

.ball-badge {
  position: absolute; top: -5px; right: -5px; z-index: 2;
  min-width: 18px; height: 18px; padding: 0 5px; border-radius: 9px;
  background: #ff6f9f; color: #fff;
  font-size: 11px; font-style: normal; line-height: 18px; text-align: center;
  box-shadow: 0 2px 6px rgba(255, 111, 159, .5);
}
</style>
