// 第三方插件窗口管理：主窗口 JS 上下文内共享，跟踪已打开的插件窗口
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { convertFileSrc } from "@tauri-apps/api/core";

const wins = new Map<string, WebviewWindow>();

export async function openPluginWindow(id: string, entryPath: string, title: string) {
  const label = "plugin-" + id;
  const existing = wins.get(label);
  if (existing) {
    await existing.show();
    await existing.setFocus();
    return;
  }
  const w = new WebviewWindow(label, {
    url: convertFileSrc(entryPath),
    title,
    width: 860,
    height: 620,
  });
  wins.set(label, w);
  // 窗口被手动关闭后从表里移除，下次启用可重新创建
  void w.once("tauri://destroyed", () => wins.delete(label));
}

export async function closePluginWindow(id: string) {
  const label = "plugin-" + id;
  const w = wins.get(label);
  if (w) {
    try { await w.close(); } catch { /* 窗口可能已不存在 */ }
    wins.delete(label);
  }
}
