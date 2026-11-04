#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

// ---------- 设置 ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    aria2_path: String,
    proxy: String,
    connections: u32,
    split: u32,
    piece_length: String,
    dir: String,
    extra_args: String,
    bg_image: String,
    bg_opacity: f64,
    ui_opacity: f64,
    close_to_tray: bool,
    /// 旧字段：仅从老配置迁移，不再写回（迁移到 plugins["mascot"]）
    #[serde(skip_serializing)]
    show_drop_ball: bool,
    /// 插件开关：插件 id → 是否启用
    plugins: HashMap<String, bool>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            aria2_path: String::new(),
            proxy: "http://127.0.0.1:10808".into(),
            connections: 16,
            split: 16,
            piece_length: "1M".into(),
            dir: String::new(),
            extra_args: String::new(),
            bg_image: String::new(),
            bg_opacity: 0.5,
            ui_opacity: 1.0,
            close_to_tray: true,
            show_drop_ball: false,
            plugins: HashMap::new(),
        }
    }
}

fn settings_file(app: &AppHandle) -> PathBuf {
    let dir = app.path().app_data_dir().expect("no app data dir");
    std::fs::create_dir_all(&dir).ok();
    dir.join("settings.json")
}

#[tauri::command]
fn load_settings(app: AppHandle) -> Settings {
    let path = settings_file(&app);
    let mut s: Settings = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    // 旧配置迁移：show_drop_ball → plugins["mascot"]
    if s.show_drop_ball {
        s.plugins.entry("mascot".into()).or_insert(true);
    }
    s
}

#[tauri::command]
fn save_settings(app: AppHandle, settings: Settings) -> Result<(), String> {
    let path = settings_file(&app);
    let text = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    // 通知前端即时应用（背景图等新设置免重启）
    let _ = app.emit("settings-changed", serde_json::to_string(&settings).unwrap_or_default());
    Ok(())
}

#[tauri::command]
fn default_settings() -> Settings {
    Settings::default()
}

// ---------- 插件系统 ----------

/// 第三方插件清单：plugins/<id>/plugin.json
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct PluginManifest {
    id: String,
    name: String,
    version: String,
    author: String,
    description: String,
    icon: String,  // emoji
    entry: String, // 相对插件目录的入口 html，默认 index.html
}

impl Default for PluginManifest {
    fn default() -> Self {
        PluginManifest {
            id: String::new(),
            name: "未命名插件".into(),
            version: "1.0.0".into(),
            author: String::new(),
            description: String::new(),
            icon: "🧩".into(),
            entry: "index.html".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct PluginInfo {
    id: String,
    name: String,
    version: String,
    author: String,
    description: String,
    icon: String,
    builtin: bool,
    enabled: bool,
    entry_path: String, // 第三方插件入口绝对路径；内置插件为空
}

fn plugins_dir(app: &AppHandle) -> PathBuf {
    let dir = app.path().app_data_dir().expect("no app data dir").join("plugins");
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn mascot_enabled(s: &Settings) -> bool {
    s.plugins.get("mascot").copied().unwrap_or(false)
}

#[tauri::command]
fn list_plugins(app: AppHandle) -> Vec<PluginInfo> {
    let s = load_settings(app.clone());
    let mut list = vec![PluginInfo {
        id: "mascot".into(),
        name: "看板娘 · 速降球".into(),
        version: "1.0.0".into(),
        author: "内置".into(),
        description: "桌面 Live2D 看板娘，拖链接到她身上直接开始下载".into(),
        icon: "🌸".into(),
        builtin: true,
        enabled: mascot_enabled(&s),
        entry_path: String::new(),
    }];
    // 扫描开放插件目录：plugins/<id>/plugin.json
    if let Ok(rd) = std::fs::read_dir(plugins_dir(&app)) {
        for e in rd.flatten() {
            let manifest_path = e.path().join("plugin.json");
            let Ok(text) = std::fs::read_to_string(&manifest_path) else { continue };
            let Ok(m) = serde_json::from_str::<PluginManifest>(&text) else { continue };
            if m.id.is_empty() || m.id == "mascot" { continue; }
            let entry = e.path().join(&m.entry);
            if !entry.is_file() { continue; }
            let enabled = s.plugins.get(&m.id).copied().unwrap_or(false);
            list.push(PluginInfo {
                id: m.id,
                name: m.name,
                version: m.version,
                author: m.author,
                description: m.description,
                icon: m.icon,
                builtin: false,
                enabled,
                entry_path: entry.to_string_lossy().into_owned(),
            });
        }
    }
    list
}

#[tauri::command]
fn set_plugin_enabled(app: AppHandle, id: String, enabled: bool) -> Result<(), String> {
    let mut s = load_settings(app.clone());
    s.plugins.insert(id, enabled);
    // save_settings 内部会 emit settings-changed，统一同步球窗显隐与托盘勾选
    save_settings(app, s)
}

#[tauri::command]
fn open_plugins_dir(app: AppHandle) -> Result<(), String> {
    let dir = plugins_dir(&app);
    Command::new("explorer")
        .arg(&dir)
        .spawn()
        .map_err(|e| format!("打开目录失败：{e}"))?;
    Ok(())
}

// ---------- aria2c 查找 ----------

#[tauri::command]
fn find_aria2(app: AppHandle) -> Result<String, String> {
    let s = load_settings(app.clone());
    if !s.aria2_path.trim().is_empty() {
        let p = PathBuf::from(s.aria2_path.trim());
        if p.is_file() {
            return Ok(p.to_string_lossy().into_owned());
        }
    }
    if let Some(p) = which_aria2() {
        return Ok(p.to_string_lossy().into_owned());
    }
    Err("未在 PATH 中找到 aria2c，也未在设置中指定路径".into())
}

fn which_aria2() -> Option<PathBuf> {
    let path_var = std::env::var("PATH").ok()?;
    let exts = ["aria2c.exe", "aria2c.bat", "aria2c.cmd"];
    for dir in std::env::split_paths(&path_var) {
        for ext in exts {
            let candidate = dir.join(ext);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

// ---------- 进度行解析 ----------

#[derive(Debug, Clone, Serialize)]
struct ProgressStats {
    percent: f64,
    downloaded_mb: f64,
    total_mb: f64,
    speed_kbps: f64,
    eta_sec: i64,
    peers: i64,
    ts: i64,
}

fn parse_size(s: &str) -> Option<f64> {
    let s = s.trim().to_lowercase();
    let (num, unit) = match s.find(|c: char| !c.is_ascii_digit() && c != '.') {
        Some(i) => (&s[..i], &s[i..]),
        None => (s.as_str(), ""),
    };
    let n: f64 = num.parse().ok()?;
    let k = match unit {
        "kib" => 1.0 / 1024.0,
        "mib" => 1.0,
        "gib" => 1024.0,
        "tib" => 1024.0 * 1024.0,
        "b" | "" => 1.0 / (1024.0 * 1024.0),
        _ => return None,
    };
    Some(n * k)
}

fn parse_progress_line(line: &str) -> Option<ProgressStats> {
    use regex::Regex;
    let re = Regex::new(r"\[#\S+\s+([^\]]+)\]").ok()?;
    let cap = re.captures(line)?;
    let body = cap[1].trim();

    let dl_re = Regex::new(r"(\d+(?:\.\d+)?)(\w+)/(\d+(?:\.\d+)?)(\w+)").ok()?;
    let m = dl_re.captures(body)?;
    let downloaded = parse_size(&format!("{}{}", &m[1], &m[2]))?;
    let total = parse_size(&format!("{}{}", &m[3], &m[4]))?;
    let percent = if total > 0.0 { (downloaded / total * 100.0).min(100.0) } else { 0.0 };

    let dl_speed_re = Regex::new(r"DL:([^\s]+)").ok()?;
    let speed = dl_speed_re
        .captures(body)
        .and_then(|c| parse_size(&c[1]))
        .unwrap_or(0.0)
        * 1024.0;

    let eta_re = Regex::new(r"ETA:(\d+)").ok()?;
    let eta = eta_re
        .captures(body)
        .and_then(|c| c[1].parse::<i64>().ok())
        .unwrap_or(0);

    let cn_re = Regex::new(r"CN:(\d+)").ok()?;
    let peers = cn_re
        .captures(body)
        .and_then(|c| c[1].parse::<i64>().ok())
        .unwrap_or(0);

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    Some(ProgressStats {
        percent,
        downloaded_mb: downloaded,
        total_mb: total,
        speed_kbps: speed,
        eta_sec: eta,
        peers,
        ts,
    })
}

// ---------- 多任务管理 ----------

struct DownloadEntry {
    child: Mutex<Option<Child>>,
    dir: String,
    urls: Vec<String>,
    rpc_port: u16,
}

struct DownloadState {
    tasks: Mutex<HashMap<String, DownloadEntry>>,
    next_id: Mutex<u64>,
}

/// 构建并启动 aria2c 子进程，接管 stdout/stderr/退出监听，登记到任务表。
/// 返回下载目录（供事件载荷使用）。
fn spawn_aria2(app: &AppHandle, id: &str, urls: &[String]) -> Result<String, String> {
    let aria2 = find_aria2(app.clone())?;
    let s = load_settings(app.clone());
    let port = pick_rpc_port(id);

    let dir = if s.dir.trim().is_empty() {
        dirs::download_dir()
            .unwrap_or_else(|| app.path().download_dir().unwrap_or_default())
            .to_string_lossy()
            .into_owned()
    } else {
        s.dir.trim().to_string()
    };

    let mut cmd = Command::new(&aria2);
    cmd.arg(format!("--all-proxy={}", s.proxy))
        .arg(format!("-x{}", s.connections))
        .arg(format!("-s{}", s.split))
        .arg(format!("-k{}", s.piece_length))
        .arg(format!("--dir={}", dir))
        .arg("--summary-interval=1")
        .arg("--console-log-level=warn")
        .arg("--file-allocation=none")
        // 开启 RPC 供任务卡片实时调速（每个任务独立端口，密钥固定；自动避让被占端口）
        .arg("--enable-rpc")
        .arg(format!("--rpc-listen-port={}", port))
        .arg("--rpc-secret=sakura");
    if !s.extra_args.trim().is_empty() {
        let args: Vec<&str> = s.extra_args.split_whitespace().collect();
        cmd.args(args);
    }
    for u in urls {
        cmd.arg(u);
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("启动 aria2c 失败：{e}"))?;

    // 在存入 entry 前先 take 出 stdout/stderr，交给专属线程读取
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    // 存入任务表
    let entry = DownloadEntry {
        child: Mutex::new(Some(child)),
        dir: dir.clone(),
        urls: urls.to_vec(),
        rpc_port: port,
    };
    app.state::<DownloadState>()
        .tasks
        .lock()
        .map_err(|e| e.to_string())?
        .insert(id.to_string(), entry);

    // ---------- stdout 线程（日志 + 进度统计）----------
    if let Some(stdout) = stdout {
        let app_s = app.clone();
        let id_s = id.to_string();
        let file_count = urls.len();
        std::thread::spawn(move || {
            use regex::Regex;
            // 批量下载时 aria2c 每个文件一行进度，按 GID 分别记录再汇总成整体进度
            let gid_re = Regex::new(r"\[#(\S+)\s").expect("gid regex");
            let mut gid_pct: HashMap<String, f64> = HashMap::new();
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                emit_log(&app_s, &id_s, &line);
                if let Some(stats) = parse_progress_line(&line) {
                    if let Some(cap) = gid_re.captures(&line) {
                        gid_pct.insert(cap[1].to_string(), stats.percent);
                    }
                    let overall = if file_count > 0 {
                        (gid_pct.values().sum::<f64>() / file_count as f64).min(100.0)
                    } else {
                        stats.percent
                    };
                    let done = gid_pct.values().filter(|p| **p >= 100.0).count();
                    let payload = serde_json::json!({
                        "id": id_s,
                        "stats": {
                            "percent": stats.percent,
                            "overall_percent": overall,
                            "files_done": done,
                            "file_count": file_count,
                            "downloaded_mb": stats.downloaded_mb,
                            "total_mb": stats.total_mb,
                            "speed_kbps": stats.speed_kbps,
                            "eta_sec": stats.eta_sec,
                            "peers": stats.peers,
                            "ts": stats.ts,
                        }
                    }).to_string();
                    emit_event(&app_s, "download-stats", &payload);
                }
            }
        });
    }

    // ---------- stderr 线程（只记日志）----------
    if let Some(stderr) = stderr {
        let app_e = app.clone();
        let id_e = id.to_string();
        std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                emit_log(&app_e, &id_e, &format!("[stderr] {line}"));
            }
        });
    }

    // ---------- 等待线程（自然退出清理）----------
    let app_w = app.clone();
    let id_w = id.to_string();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_millis(500));
            let state = app_w.state::<DownloadState>();
            let mut tasks = match state.tasks.lock() {
                Ok(g) => g,
                Err(_) => break,
            };
            // 第一阶段：只读判定（entry 借用 tasks，期间不能 remove）
            enum Outcome {
                Running,
                Finished(std::process::ExitStatus),
                Failed(String),
                Gone,
            }
            let outcome = {
                let entry = match tasks.get(&id_w) {
                    Some(e) => e,
                    None => break,
                };
                let mut child_guard = match entry.child.lock() {
                    Ok(g) => g,
                    Err(_) => break,
                };
                match child_guard.as_mut() {
                    Some(c) => match c.try_wait() {
                        Ok(Some(status)) => Outcome::Finished(status),
                        Ok(None) => Outcome::Running,
                        Err(e) => Outcome::Failed(e.to_string()),
                    },
                    None => Outcome::Gone,
                }
            };
            // 第二阶段：entry/child 借用已释放，可安全移除
            match outcome {
                Outcome::Running => {}
                Outcome::Finished(status) => {
                    let payload = serde_json::json!({ "id": id_w, "status": format!("{status}") }).to_string();
                    emit_event(&app_w, "download-exit", &payload);
                    if let Some(entry) = tasks.remove(&id_w) {
                        push_history(
                            &app_w,
                            HistoryEntry {
                                id: id_w.clone(),
                                urls: entry.urls,
                                dir: entry.dir,
                                status: if status.success() { "done".into() } else { "error".into() },
                                finished_at: now_millis(),
                            },
                        );
                    }
                    break;
                }
                Outcome::Failed(e) => {
                    emit_log(&app_w, &id_w, &format!("[错误] {e}"));
                    tasks.remove(&id_w);
                    break;
                }
                Outcome::Gone => break,
            }
        }
    });

    Ok(dir)
}

#[tauri::command]
fn start_download(
    app: AppHandle,
    state: State<'_, DownloadState>,
    urls: Vec<String>,
) -> Result<String, String> {
    let mut id_guard = state.next_id.lock().map_err(|e| e.to_string())?;
    let id = format!("task_{}", *id_guard);
    *id_guard += 1;
    drop(id_guard);

    let dir = spawn_aria2(&app, &id, &urls)?;

    // 向前端发送开始事件
    let payload = serde_json::json!({ "id": id, "urls": urls, "dir": dir }).to_string();
    emit_event(&app, "download-started", &payload);

    Ok(id)
}

fn emit_log(app: &AppHandle, task_id: &str, text: &str) {
    let payload = serde_json::json!({ "id": task_id, "text": text }).to_string();
    let _ = app.emit("download-log", &payload);
}

fn emit_event(app: &AppHandle, event: &str, payload: &str) {
    let _ = app.emit(event, payload);
}

#[tauri::command]
fn stop_download(
    state: State<'_, DownloadState>,
    app: AppHandle,
    id: String,
) -> Result<(), String> {
    let mut tasks = state.tasks.lock().map_err(|e| e.to_string())?;
    let mut cleanup: Option<(String, Vec<String>)> = None;
    if let Some(entry) = tasks.get_mut(&id) {
        let mut g = entry.child.lock().map_err(|e| e.to_string())?;
        if let Some(child) = g.as_mut() {
            let pid = child.id();
            let _ = Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/T", "/F"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            *g = None;
        }
        cleanup = Some((entry.dir.clone(), entry.urls.clone()));
    }
    let removed = tasks.remove(&id).is_some();
    drop(tasks);

    // 清掉本次任务的中间文件：未完成的分片文件 + .aria2 控制文件
    // 等 400ms 让被杀掉的 aria2c 释放文件句柄
    if let Some((dir, urls)) = &cleanup {
        std::thread::sleep(std::time::Duration::from_millis(400));
        let mut cleaned = 0u32;
        for u in urls {
            if let Some(name) = guess_filename(u) {
                for cand in [
                    std::path::Path::new(&dir).join(&name),
                    std::path::Path::new(&dir).join(format!("{name}.aria2")),
                ] {
                    if cand.exists() && std::fs::remove_file(&cand).is_ok() {
                        cleaned += 1;
                    }
                }
            }
        }
        if cleaned > 0 {
            emit_log(&app, &id, &format!("已清理 {cleaned} 个残留文件"));
        }
    }

    if removed {
        // 记录到下载历史（已停止）
        if let Some((dir, urls)) = &cleanup {
            push_history(
                &app,
                HistoryEntry {
                    id: id.clone(),
                    urls: urls.clone(),
                    dir: dir.clone(),
                    status: "stopped".into(),
                    finished_at: now_millis(),
                },
            );
        }
        let payload = serde_json::json!({ "id": id, "stopped": true }).to_string();
        emit_event(&app, "download-exit", &payload);
    }
    Ok(())
}

/// 从 URL 推测 aria2 落盘的文件名（仅 http/https 可确定，磁链返回 None）。
/// 规则：去掉 query/fragment 后取最后一段，做百分号解码。
fn guess_filename(url: &str) -> Option<String> {
    let no_frag = url.split('#').next()?;
    let no_query = no_frag.split('?').next()?;
    let seg = no_query.rsplit('/').next()?.trim();
    if seg.is_empty() || !url.starts_with("http") {
        return None;
    }
    percent_decode(seg)
}

/// 最小百分号解码（%XX → UTF-8），失败回退原串
fn percent_decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let h = (bytes[i + 1] as char).to_digit(16)?;
            let l = (bytes[i + 2] as char).to_digit(16)?;
            out.push((h * 16 + l) as u8);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    Some(String::from_utf8_lossy(&out).into_owned())
}

#[tauri::command]
fn list_downloads(state: State<'_, DownloadState>) -> Result<Vec<String>, String> {
    let tasks = state.tasks.lock().map_err(|e| e.to_string())?;
    Ok(tasks.keys().cloned().collect())
}

// ---------- 下载历史 ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HistoryEntry {
    id: String,
    urls: Vec<String>,
    dir: String,
    status: String, // done | stopped | error
    finished_at: i64,
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn history_file(app: &AppHandle) -> PathBuf {
    let dir = app.path().app_data_dir().expect("no app data dir");
    std::fs::create_dir_all(&dir).ok();
    dir.join("history.json")
}

fn load_history(app: &AppHandle) -> Vec<HistoryEntry> {
    std::fs::read_to_string(history_file(app))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn push_history(app: &AppHandle, entry: HistoryEntry) {
    let mut list = load_history(app);
    list.insert(0, entry); // 新的在最前
    list.truncate(100);
    if let Ok(text) = serde_json::to_string_pretty(&list) {
        let _ = std::fs::write(history_file(app), text);
    }
}

#[tauri::command]
fn list_history(app: AppHandle) -> Vec<HistoryEntry> {
    load_history(&app)
}

#[tauri::command]
fn reveal_in_folder(dir: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer").arg(&dir).spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(&dir).spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open").arg(&dir).spawn().map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ---------- 任务限速（aria2 RPC）----------

/// 每个任务的 aria2c 用独立 RPC 端口：16800 + 任务序号为基准，
/// 若被占用（比如上次运行残留的 aria2c 进程）则向上探测空闲端口
fn pick_rpc_port(id: &str) -> u16 {
    let base = 16800 + id.trim_start_matches("task_").parse::<u16>().unwrap_or(0);
    (base..base.saturating_add(100))
        .find(|p| std::net::TcpListener::bind(("127.0.0.1", *p)).is_ok())
        .unwrap_or(base)
}

/// 最小 JSON-RPC over HTTP 客户端（localhost、明文、Connection: close）
fn rpc_call(port: u16, method: &str, params: serde_json::Value) -> Result<(), String> {
    use std::io::{Read, Write};
    let body = serde_json::json!({
        "jsonrpc": "2.0", "id": "sakura", "method": method, "params": params
    })
    .to_string();
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(2))
        .map_err(|e| format!("连接 aria2 RPC 失败（任务可能刚启动）：{e}"))?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(2))).ok();
    stream.set_write_timeout(Some(std::time::Duration::from_secs(2))).ok();
    let req = format!(
        "POST /jsonrpc HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    stream.flush().ok();
    let mut resp = String::new();
    let _ = stream.read_to_string(&mut resp); // 读超时/对端不关连接都不致命
    if resp.contains("\"error\"") {
        return Err("aria2 拒绝了限速设置".to_string());
    }
    Ok(())
}

#[tauri::command]
async fn set_speed_limit(
    app: AppHandle,
    state: State<'_, DownloadState>,
    id: String,
    bytes: u64,
) -> Result<(), String> {
    let port = {
        let tasks = state.tasks.lock().map_err(|e| e.to_string())?;
        tasks.get(&id).map(|e| e.rpc_port).ok_or("任务不存在或已结束")?
    };
    // 阻塞式 TCP 挪到后台线程：之前同步 read 会卡住整个窗口，看起来像“下载停了”
    let result = tauri::async_runtime::spawn_blocking(move || {
        // 每个任务一个 aria2c 进程：改全局限速 = 改该任务的限速，立即生效
        rpc_call(
            port,
            "aria2.changeGlobalOption",
            serde_json::json!(["token:sakura", { "max-download-limit": bytes.to_string() }]),
        )
    })
    .await
    .map_err(|e| e.to_string())?;
    match &result {
        Ok(()) => emit_log(&app, &id, &format!("[限速] 已设置为 {}", fmt_limit(bytes))),
        Err(e) => emit_log(&app, &id, &format!("[限速] 设置失败：{e}")),
    }
    result
}

/// 限速值的人类可读格式（0 = 不限速，aria2 协议定义）
fn fmt_limit(bytes: u64) -> String {
    if bytes == 0 {
        "不限速".to_string()
    } else if bytes % 1_048_576 == 0 {
        format!("{} MB/s", bytes / 1_048_576)
    } else if bytes % 1024 == 0 {
        format!("{} KB/s", bytes / 1024)
    } else {
        format!("{bytes} B/s")
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(DownloadState {
            tasks: Mutex::new(HashMap::new()),
            next_id: Mutex::new(0),
        })
        .invoke_handler(tauri::generate_handler![
            load_settings,
            save_settings,
            default_settings,
            find_aria2,
            start_download,
            stop_download,
            list_downloads,
            list_history,
            reveal_in_folder,
            set_speed_limit,
            list_plugins,
            set_plugin_enabled,
            open_plugins_dir
        ])
        // ---------- 系统托盘 ----------
        .setup(|app| {
            use tauri::menu::{CheckMenuItem, Menu, MenuItem};
            use tauri::tray::TrayIconBuilder;
            use tauri::Listener;

            let show = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
            let ball_on = mascot_enabled(&load_settings(app.handle().clone()));
            let ball = CheckMenuItem::with_id(app, "ball", "显示速降球", true, ball_on, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &ball, &quit])?;

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("aria2 启动器")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.unminimize();
                            let _ = w.set_focus();
                        }
                    }
                    "ball" => {
                        // 勾选状态由系统自动切换；写回设置并走 settings-changed 统一同步窗口显隐
                        let mut s = load_settings(app.clone());
                        let on = !mascot_enabled(&s);
                        s.plugins.insert("mascot".into(), on);
                        let _ = save_settings(app.clone(), s);
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            // 启动时按设置恢复速降球
            if ball_on {
                if let Some(w) = app.get_webview_window("dropball") { let _ = w.show(); }
            }

            // 设置保存后：同步球窗显隐 + 托盘勾选
            let ball_sync = ball.clone();
            let app_sync = app.handle().clone();
            app.listen("settings-changed", move |ev| {
                // 注意：emit 的载荷是 String，Rust 侧拿到的是再序列化一层的 JSON，需要先解出内层字符串
                let json = serde_json::from_str::<String>(ev.payload()).unwrap_or_else(|_| ev.payload().to_string());
                if let Ok(s) = serde_json::from_str::<Settings>(&json) {
                    let on = mascot_enabled(&s);
                    if let Some(w) = app_sync.get_webview_window("dropball") {
                        let vis = w.is_visible().unwrap_or(false);
                        if on != vis {
                            if on { let _ = w.show(); } else { let _ = w.hide(); }
                        }
                    }
                    let _ = ball_sync.set_checked(on);
                }
            });

            // 球上点「隐藏」：持久化关闭 + 取消托盘勾选
            let ball_hide = ball.clone();
            let app_hide = app.handle().clone();
            app.listen("dropball-hide", move |_| {
                let mut s = load_settings(app_hide.clone());
                s.plugins.insert("mascot".into(), false);
                let _ = save_settings(app_hide.clone(), s);
                let _ = ball_hide.set_checked(false);
            });
            Ok(())
        })
        // 点窗口关闭按钮：按设置决定「最小化到托盘」或「直接退出」
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() != "main" { return; }
                if load_settings(window.app_handle().clone()).close_to_tray {
                    let _ = window.hide();
                    api.prevent_close();
                } else if let Some(ball) = window.get_webview_window("dropball") {
                    // 直接退出时把速降球一起关掉，否则进程不会结束
                    let _ = ball.close();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
