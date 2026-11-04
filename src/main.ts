import { createApp } from "vue";
import ElementPlus from "element-plus";
import "element-plus/dist/index.css";
import zhCn from "element-plus/es/locale/lang/zh-cn";
import "@fontsource/zcool-kuaile";
import "@fontsource/noto-sans-sc";

// 速降球是独立小窗口：通过 URL 参数识别，渲染轻量组件并切透明底。
// 动态 import：Live2D/Pixi 只有球窗口才加载，不拖慢主窗口
const isBall = new URLSearchParams(location.search).get("win") === "dropball";
if (isBall) document.documentElement.classList.add("ball-mode");

const root = isBall
  ? (await import("./DropBall.vue")).default
  : (await import("./App.vue")).default;

createApp(root).use(ElementPlus, { locale: zhCn }).mount("#app");
