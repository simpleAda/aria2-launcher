# aria2 启动器

把常用的 aria2c 命令行做成图形界面。替代：

```
aria2c --all-proxy=http://127.0.0.1:10808 -x 16 -s 16 -k 1M <链接>
```

## 开发运行

```powershell
cd E:\project\aria2-launcher
npm run tauri dev
```

首次会自动编译 Rust（已缓存大半），然后弹出 860×620 的窗口。

## 打包成 exe

```powershell
npm run tauri build
```

产物在 `src-tauri\target\release\bundle\nsis\` 下，是 NSIS 安装包。

## 使用

1. 首次打开会先跳到「设置」页检测 aria2c（本机 PATH 里已有 1.37.0，直接就绪）
2. 设置页可改：代理、每服务器连接数 -x、分片数 -s、分片大小 -k、下载目录、额外参数
   —— 都存在 `%APPDATA%\com.example.aria2-launcher\settings.json`
3. 「下载」页粘贴链接（支持每行一个批量），点「开始下载」
4. 日志区实时显示 aria2c 输出；`[#xxx 12MiB/80MiB(15%)...]` 进度行会单独显示在按钮旁
5. 下载中可点「停止」（Windows 下用 taskkill 终止进程树）

## 结构

```
aria2-launcher/
├── src/                  # Vue 3 前端
│   ├── App.vue           # 布局 + 侧栏 + aria2 状态栏
│   ├── main.ts           # 路由：/ 下载页  /settings 设置页
│   └── views/
│       ├── Download.vue  # 链接输入、日志、进度
│       └── Settings.vue  # 参数表单、aria2c 路径检测/浏览
└── src-tauri/
    └── src/main.rs       # 启动/停止 aria2c、stdout 转发、设置读写
```

## 修改默认值

Rust 侧 `Settings::default()`（src-tauri/src/main.rs）是出厂默认；界面保存后以界面为准。
