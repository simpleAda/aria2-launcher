import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// Tauri 前端配置：固定端口，dev 模式加载 http://localhost:1420
export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: "chrome110",
    minify: "esbuild",
    sourcemap: false,
    outDir: "dist",
  },
});
