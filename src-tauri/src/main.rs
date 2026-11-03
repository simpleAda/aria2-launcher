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
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

#[tauri::command]
fn save_settings(app: AppHandle, settings: Settings) -> Result<(), String> {
    let path = settings_file(&app);
    let text = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| e.to_string())
}

#[tauri::command]
fn default_settings() -> Settings {
    Settings::default()
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
        .arg("--file-allocation=none");
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
                Finished(String),
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
                        Ok(Some(status)) => Outcome::Finished(format!("{status}")),
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
                    let payload = serde_json::json!({ "id": id_w, "status": status }).to_string();
                    emit_event(&app_w, "download-exit", &payload);
                    tasks.remove(&id_w);
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
    }
    let removed = tasks.remove(&id).is_some();
    drop(tasks);
    if removed {
        let payload = serde_json::json!({ "id": id, "stopped": true }).to_string();
        emit_event(&app, "download-exit", &payload);
    }
    Ok(())
}

#[tauri::command]
fn list_downloads(state: State<'_, DownloadState>) -> Result<Vec<String>, String> {
    let tasks = state.tasks.lock().map_err(|e| e.to_string())?;
    Ok(tasks.keys().cloned().collect())
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
            list_downloads
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
