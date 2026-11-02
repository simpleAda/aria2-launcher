import { createApp } from "vue";
import { createRouter, createWebHistory } from "vue-router";
import ElementPlus from "element-plus";
import "element-plus/dist/index.css";
import zhCn from "element-plus/es/locale/lang/zh-cn";
import "@fontsource/zcool-kuaile";
import "@fontsource/noto-sans-sc";
import App from "./App.vue";
import Download from "./views/Download.vue";
import Settings from "./views/Settings.vue";

const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: "/", component: Download },
    { path: "/settings", component: Settings },
  ],
});

createApp(App).use(router).use(ElementPlus, { locale: zhCn }).mount("#app");
