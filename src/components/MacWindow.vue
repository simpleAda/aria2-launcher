<template>
  <div class="mac-win" :class="{ max: win.max, moving }" :style="winStyle" @pointerdown="emit('focus')">
    <!-- 标题栏：红绿灯 + 标题，按住拖动 -->
    <div class="mac-win-bar" @pointerdown="startDrag" @dblclick="toggleMax">
      <div class="mac-lights">
        <span class="mac-light red" title="关闭" @click.stop="emit('close')"></span>
        <span class="mac-light yellow" title="最小化到 Dock" @click.stop="emit('minimize')"></span>
        <span class="mac-light green" title="最大化/还原" @click.stop="toggleMax"></span>
      </div>
      <span class="mac-win-title">{{ title }}</span>
    </div>
    <div class="mac-win-body"><slot /></div>
    <div v-if="!win.max" class="mac-win-resize" @pointerdown.stop="startResize"></div>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, ref } from "vue";

interface WinState {
  x: number; y: number; w: number; h: number;
  z: number; open: boolean; min: boolean; max: boolean;
}

const props = defineProps<{ win: WinState; title: string }>();
const emit = defineEmits<{ (e: "close"): void; (e: "minimize"): void; (e: "focus"): void }>();

const winStyle = computed(() =>
  props.win.max
    ? { left: "8px", top: "8px", width: "calc(100% - 16px)", height: "calc(100% - 96px)", zIndex: props.win.z }
    : { left: props.win.x + "px", top: props.win.y + "px", width: props.win.w + "px", height: props.win.h + "px", zIndex: props.win.z }
);

// 拖动/缩放进行中：关闭几何过渡，避免跟手延迟
const moving = ref(false);

// ---------- 拖动 ----------
function startDrag(e: PointerEvent) {
  if (props.win.max) return;                    // 最大化时不拖动
  if ((e.target as HTMLElement).closest(".mac-light")) return; // 点到红绿灯不拖
  const bar = e.currentTarget as HTMLElement;
  bar.setPointerCapture(e.pointerId);
  moving.value = true;
  const dx = e.clientX - props.win.x;
  const dy = e.clientY - props.win.y;
  const move = (ev: PointerEvent) => {
    props.win.x = Math.min(Math.max(ev.clientX - dx, -props.win.w + 120), window.innerWidth - 80);
    props.win.y = Math.min(Math.max(ev.clientY - dy, 0), window.innerHeight - 60);
  };
  const up = () => {
    moving.value = false;
    bar.removeEventListener("pointermove", move);
    bar.removeEventListener("pointerup", up);
  };
  bar.addEventListener("pointermove", move);
  bar.addEventListener("pointerup", up);
}

// ---------- 缩放（右下角手柄） ----------
function startResize(e: PointerEvent) {
  const handle = e.currentTarget as HTMLElement;
  handle.setPointerCapture(e.pointerId);
  moving.value = true;
  const sw = props.win.w, sh = props.win.h;
  const sx = e.clientX, sy = e.clientY;
  const move = (ev: PointerEvent) => {
    props.win.w = Math.max(360, sw + ev.clientX - sx);
    props.win.h = Math.max(240, sh + ev.clientY - sy);
  };
  const up = () => {
    moving.value = false;
    handle.removeEventListener("pointermove", move);
    handle.removeEventListener("pointerup", up);
  };
  handle.addEventListener("pointermove", move);
  handle.addEventListener("pointerup", up);
}

// ---------- 最大化 / 还原 ----------
const prev = reactive({ x: 0, y: 0, w: 0, h: 0 });
function toggleMax() {
  const w = props.win;
  if (w.max) {
    Object.assign(w, { x: prev.x, y: prev.y, w: prev.w, h: prev.h, max: false });
  } else {
    Object.assign(prev, { x: w.x, y: w.y, w: w.w, h: w.h });
    w.max = true;
  }
}
</script>

<style>
.mac-win {
  position: absolute;
  display: flex;
  flex-direction: column;
  border-radius: 14px;
  overflow: hidden;
  background: var(--glass-panel);
  backdrop-filter: blur(16px) saturate(1.5) brightness(1.08);
  border: 1px solid rgba(255, 255, 255, .22);
  box-shadow: 0 16px 50px rgba(60, 30, 90, .28);
  transform-origin: 50% 100%; /* 动画以底部为轴，向 Dock 收放 */
  transition: left .22s ease, top .22s ease, width .22s ease, height .22s ease; /* 最大化/还原平滑 */
}
.mac-win.moving { transition: none; } /* 拖动/缩放跟手，不过渡 */

/* mac 风出现/消失动画（配合 App.vue 的 <Transition name="mac">） */
.mac-enter-active { transition: opacity .3s ease, transform .34s cubic-bezier(.22, 1.15, .32, 1); }
.mac-leave-active {
  transition: opacity .18s ease, transform .22s cubic-bezier(.5, 0, .75, 0);
  pointer-events: none; /* 消失途中不挡点击 */
}
.mac-enter-from, .mac-leave-to { opacity: 0; transform: translateY(110px) scale(.8); }

/* 标题栏 */
.mac-win-bar {
  display: flex; align-items: center; gap: 10px;
  height: 34px; padding: 0 12px; flex-shrink: 0;
  background: rgba(255, 255, 255, .1);
  border-bottom: 1px solid rgba(255, 255, 255, .12);
  cursor: grab; user-select: none; touch-action: none;
}
.mac-win-bar:active { cursor: grabbing; }
.mac-win-title {
  flex: 1; text-align: center; font-size: 13px;
  color: rgba(255, 255, 255, .9);
  text-shadow: 0 1px 3px rgba(0, 0, 0, .4);
  margin-right: 52px; /* 抵消红绿灯宽度，视觉居中 */
}

/* 红绿灯 */
.mac-lights { display: flex; gap: 8px; }
.mac-light {
  width: 12px; height: 12px; border-radius: 50%;
  cursor: pointer; transition: filter .15s ease, transform .1s ease;
}
.mac-light:hover { filter: brightness(1.15); }
.mac-light:active { transform: scale(.85); }
.mac-light.red { background: #ff5f57; }
.mac-light.yellow { background: #febc2e; }
.mac-light.green { background: #28c840; }

/* 内容区 */
.mac-win-body {
  flex: 1; overflow-y: auto; padding: 14px 16px;
  min-height: 0;
}
/* 组件页自带的 H1 与窗口标题重复，隐藏 */
.mac-win-body h1 { display: none; }

/* 右下角缩放手柄 */
.mac-win-resize {
  position: absolute; right: 0; bottom: 0;
  width: 18px; height: 18px; cursor: nwse-resize;
  touch-action: none;
}
.mac-win-resize::after {
  content: ""; position: absolute; right: 4px; bottom: 4px;
  width: 8px; height: 8px;
  border-right: 2px solid rgba(255, 255, 255, .5);
  border-bottom: 2px solid rgba(255, 255, 255, .5);
  border-radius: 1px;
}
</style>
