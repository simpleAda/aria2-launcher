# aria2 启动器

把常用的 aria2c 命令行做成图形界面。替代：

```
aria2c --all-proxy=http://127.0.0.1:10808 -x 16 -s 16 -k 1M <链接>
```

## 功能

- 粘贴链接即可下载，支持每行一个的批量下载（一个任务内 aria2c 并发下载，进度条显示整体进度和已完成文件数）
- 侧栏实时任务列表：进度、速度、停止按钮
- 下载页实时速度 / ETA 曲线图 + 运行日志
- 设置页可改：代理、每服务器连接数 -x、分片数 -s、分片大小 -k、下载目录、额外参数
- 设置保存在 `%APPDATA%\top.phro.aria2\settings.json`

## 开发运行

```powershell
pnpm install
pnpm tauri dev
```

首次会自动编译 Rust（需要已安装 Rust 工具链），然后弹出 860×620 的窗口。

## 打包成 exe

```powershell
pnpm tauri build
```

产物在 `src-tauri\target\release\bundle\nsis\` 下，是 NSIS 安装包。

## 使用

1. 首次打开会先跳到「设置」页检测 aria2c（本机 PATH 里有即直接就绪，也可以手动浏览指定 exe）
2. 「下载」页粘贴链接（每行一个），点「开始下载」
3. 日志区实时显示 aria2c 输出；侧栏任务卡片显示整体进度和速度
4. 下载中可点任务上的 ✕ 停止（Windows 下用 taskkill 终止进程树）

## 结构

```
aria2-launcher/
├── src/                  # Vue 3 前端
│   ├── App.vue           # 布局 + 侧栏任务列表 + 全局事件监听
│   ├── main.ts           # 路由：/ 下载页  /settings 设置页
│   └── views/
│       ├── Download.vue  # 链接输入、速度图表、日志
│       └── Settings.vue  # 参数表单、aria2c 路径检测/浏览
└── src-tauri/
    └── src/main.rs       # 启动/停止 aria2c、进度行解析、设置读写
```

## 修改默认值

Rust 侧 `Settings::default()`（src-tauri/src/main.rs）是出厂默认；界面保存后以界面为准。
