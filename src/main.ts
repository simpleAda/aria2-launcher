import { createApp } from "vue";
import ElementPlus from "element-plus";
import "element-plus/dist/index.css";
import zhCn from "element-plus/es/locale/lang/zh-cn";
import "@fontsource/zcool-kuaile";
import "@fontsource/noto-sans-sc";
import App from "./App.vue";

createApp(App).use(ElementPlus, { locale: zhCn }).mount("#app");
