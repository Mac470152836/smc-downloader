use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::fs::{self, OpenOptions};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
struct DownloadMeta {
    url: String,
    token: String,
    filename: String,
    total_size: u64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DownloadInfo {
    id: String,
    url: String,
    token: String,
    filename: String,
    save_dir: String,
    total_size: u64,
    downloaded: u64,
    status: String,
    error: Option<String>,
    created_at: String,
    #[serde(default)]
    file_missing: bool,
}

struct DownloadEntry {
    info: DownloadInfo,
    cancel_flag: Arc<AtomicBool>,
    task: Option<tokio::task::JoinHandle<()>>,
}

#[derive(Clone)]
pub struct DownloadManager {
    downloads: Arc<Mutex<HashMap<String, DownloadEntry>>>,
}

impl DownloadManager {
    pub fn new() -> Self {
        Self { downloads: Arc::new(Mutex::new(HashMap::new())) }
    }
}

fn meta_path(save_dir: &str, filename: &str) -> PathBuf {
    PathBuf::from(save_dir).join(format!("{}.meta.json", filename))
}
fn part_path(save_dir: &str, filename: &str) -> PathBuf {
    PathBuf::from(save_dir).join(format!("{}.part", filename))
}
fn final_path(save_dir: &str, filename: &str) -> PathBuf {
    PathBuf::from(save_dir).join(filename)
}

fn filename_from_url(url: &str) -> String {
    let path = url.split('?').next().unwrap_or(url);
    let name = path.rsplit('/').next().unwrap_or("download");
    if name.is_empty() { "download".to_string() } else { name.to_string() }
}

fn emit_progress(app: &AppHandle, id: &str, downloaded: u64, total: u64, speed: u64) {
    println!("[{}] progress: {} / {} bytes, {} B/s", id, downloaded, total, speed);
    let _ = app.emit("download-progress", serde_json::json!({
        "id": id, "downloaded": downloaded, "total": total, "speed": speed,
    }));
}

fn emit_status(app: &AppHandle, id: &str, status: &str, error: Option<&str>) {
    println!("[{}] status: {} {}", id, status, error.unwrap_or(""));
    let _ = app.emit("download-status", serde_json::json!({
        "id": id, "status": status, "error": error,
    }));
}

fn persist_file_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("smc-downloader").join("downloads.json"))
}

async fn persist_downloads(mgr: &DownloadManager) {
    let path = match persist_file_path() { Some(p) => p, None => return };
    let infos: Vec<DownloadInfo> = {
        let d = mgr.downloads.lock().await;
        d.values().map(|e| e.info.clone()).collect()
    };
    if let Some(parent) = path.parent() { let _ = fs::create_dir_all(parent).await; }
    let _ = fs::write(&path, serde_json::to_string_pretty(&infos).unwrap_or_default()).await;
}

async fn download_worker(app: AppHandle, mgr: DownloadManager, id: String) {
    println!("[{}] worker started", id);
    let (info, cancel_flag) = {
        let downloads = mgr.downloads.lock().await;
        match downloads.get(&id) {
            Some(e) => (e.info.clone(), e.cancel_flag.clone()),
            None => { println!("[{}] entry not found", id); return; }
        }
    };

    let save_dir = info.save_dir.clone();
    let filename = info.filename.clone();
    let url = info.url.clone();
    let token = info.token.clone();
    println!("[{}] url={}, filename={}, save_dir={}", id, url, filename, save_dir);

    let meta_file = meta_path(&save_dir, &filename);
    let part_file = part_path(&save_dir, &filename);
    let final_file = final_path(&save_dir, &filename);

    if final_file.exists() {
        println!("[{}] already complete", id);
        let file_len = fs::metadata(&final_file).await.map(|m| m.len()).unwrap_or(0);
        { let mut d = mgr.downloads.lock().await;
          if let Some(e) = d.get_mut(&id) { e.info.status = "completed".into(); e.info.total_size = file_len; e.info.downloaded = file_len; } }
        emit_progress(&app, &id, file_len, file_len, 0);
        emit_status(&app, &id, "completed", None);
        persist_downloads(&mgr).await;
        return;
    }

    let mut resume_offset: u64 = 0;
    let mut total_size: u64 = 0;
    if meta_file.exists() && part_file.exists() {
        println!("[{}] resuming from .meta + .part", id);
        let meta_str = match fs::read_to_string(&meta_file).await {
            Ok(s) => s,
            Err(e) => { let m = e.to_string(); set_status(&mgr, &id, "error", Some(m.clone()), 0).await; emit_status(&app, &id, "error", Some(&m)); persist_downloads(&mgr).await; return; }
        };
        let meta: DownloadMeta = match serde_json::from_str(&meta_str) {
            Ok(m) => m,
            Err(e) => { let m = e.to_string(); set_status(&mgr, &id, "error", Some(m.clone()), 0).await; emit_status(&app, &id, "error", Some(&m)); persist_downloads(&mgr).await; return; }
        };
        total_size = meta.total_size;
        resume_offset = fs::metadata(&part_file).await.map(|m| m.len()).unwrap_or(0);
        println!("[{}] resume_offset={}, total={}", id, resume_offset, total_size);
    }

    let client = reqwest::Client::new();
    let mut req = client.get(&url).header("Authorization", format!("Bearer {}", token));
    if resume_offset > 0 {
        println!("[{}] sending Range: bytes={}-", id, resume_offset);
        req = req.header("Range", format!("bytes={}-", resume_offset));
    }

    let resp = match req.send().await {
        Ok(r) => { println!("[{}] HTTP {}", id, r.status()); r }
        Err(e) => { let m = e.to_string(); set_status(&mgr, &id, "error", Some(m.clone()), 0).await; emit_status(&app, &id, "error", Some(&m)); persist_downloads(&mgr).await; return; }
    };

    let sc = resp.status();
    if !sc.is_success() && sc != reqwest::StatusCode::PARTIAL_CONTENT {
        let msg = format!("HTTP {}", sc);
        println!("[{}] error: {}", id, msg);
        set_status(&mgr, &id, "error", Some(msg.clone()), 0).await;
        emit_status(&app, &id, "error", Some(&msg));
        persist_downloads(&mgr).await;
        return;
    }

    let total_from_cl = resp.headers().get("content-length")
        .and_then(|v| v.to_str().ok()).and_then(|s| s.parse::<u64>().ok());
    let total_from_cr = resp.headers().get("content-range")
        .and_then(|v| v.to_str().ok()).and_then(|s| s.rsplit('/').next()).and_then(|s| s.parse::<u64>().ok());
    if let Some(t) = total_from_cr.or(total_from_cl) {
        if t > total_size { total_size = t; }
        println!("[{}] total_size from headers: {}", id, total_size);
    }

    let mut current = if sc == reqwest::StatusCode::OK && resume_offset > 0 {
        println!("[{}] server returned 200 not 206, restarting", id);
        0u64
    } else { resume_offset };

    let meta = DownloadMeta { url: url.clone(), token: token.clone(), filename: filename.clone(), total_size };
    let _ = fs::write(&meta_file, serde_json::to_string(&meta).unwrap_or_default()).await;

    { let mut d = mgr.downloads.lock().await;
      if let Some(e) = d.get_mut(&id) { e.info.total_size = total_size; e.info.downloaded = current; } }

    emit_progress(&app, &id, current, total_size, 0);
    emit_status(&app, &id, "downloading", None);

    let file = if current > 0 {
        OpenOptions::new().append(true).create(true).open(&part_file).await
    } else {
        OpenOptions::new().write(true).create(true).truncate(true).open(&part_file).await
    };
    let mut file = match file {
        Ok(f) => f,
        Err(e) => { let m = e.to_string(); set_status(&mgr, &id, "error", Some(m.clone()), current).await; emit_status(&app, &id, "error", Some(&m)); persist_downloads(&mgr).await; return; }
    };

    let mut stream = resp.bytes_stream();
    let mut last_emit = Instant::now();
    let mut last_bytes = current;
    let mut speed = 0u64;

    while let Some(chunk_result) = stream.next().await {
        if cancel_flag.load(Ordering::Relaxed) {
            let _ = file.flush().await;
            set_status(&mgr, &id, "paused", None, current).await;
            emit_status(&app, &id, "paused", None);
            persist_downloads(&mgr).await;
            return;
        }
        let chunk = match chunk_result {
            Ok(c) => c,
            Err(e) => { let _ = file.flush().await; let m = e.to_string(); set_status(&mgr, &id, "error", Some(m.clone()), current).await; emit_status(&app, &id, "error", Some(&m)); persist_downloads(&mgr).await; return; }
        };
        if file.write_all(&chunk).await.is_err() {
            set_status(&mgr, &id, "error", Some("write failed".into()), current).await;
            emit_status(&app, &id, "error", Some("write failed"));
            persist_downloads(&mgr).await;
            return;
        }
        current += chunk.len() as u64;

        if last_emit.elapsed() >= Duration::from_millis(500) {
            let elapsed = last_emit.elapsed().as_secs_f64();
            if elapsed > 0.0 { speed = ((current - last_bytes) as f64 / elapsed) as u64; }
            last_bytes = current;
            last_emit = Instant::now();
            { let mut d = mgr.downloads.lock().await; if let Some(e) = d.get_mut(&id) { e.info.downloaded = current; } }
            emit_progress(&app, &id, current, total_size, speed);
        }
    }

    let _ = file.flush().await;

    if total_size == 0 || current >= total_size {
        println!("[{}] complete, renaming .part -> final", id);
        let _ = fs::rename(&part_file, &final_file).await;
        let _ = fs::remove_file(&meta_file).await;
        { let mut d = mgr.downloads.lock().await; if let Some(e) = d.get_mut(&id) { e.info.status = "completed".into(); e.info.downloaded = current; } }
        emit_progress(&app, &id, current, total_size, 0);
        emit_status(&app, &id, "completed", None);
        persist_downloads(&mgr).await;
    } else {
        set_status(&mgr, &id, "paused", None, current).await;
        emit_status(&app, &id, "paused", None);
        persist_downloads(&mgr).await;
    }
}

async fn set_status(mgr: &DownloadManager, id: &str, status: &str, error: Option<String>, downloaded: u64) {
    let mut d = mgr.downloads.lock().await;
    if let Some(e) = d.get_mut(id) { e.info.status = status.into(); e.info.error = error; e.info.downloaded = downloaded; }
}

#[tauri::command]
async fn start_download(app: AppHandle, mgr: State<'_, DownloadManager>, url: String, token: String, save_dir: String, filename: Option<String>) -> Result<DownloadInfo, String> {
    let id = Uuid::new_v4().to_string()[..8].to_string();
    let fname = filename.unwrap_or_else(|| filename_from_url(&url));
    let info = DownloadInfo { id: id.clone(), url: url.clone(), token: token.clone(), filename: fname, save_dir: save_dir.clone(), total_size: 0, downloaded: 0, status: "downloading".into(), error: None, created_at: chrono::Utc::now().to_rfc3339(), file_missing: false };
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let mgr_clone = mgr.inner().clone();
    { let mut d = mgr.downloads.lock().await; d.insert(id.clone(), DownloadEntry { info: info.clone(), cancel_flag, task: None }); }
    let app_c = app.clone(); let id_c = id.clone();
    let task = tokio::spawn(async move { download_worker(app_c, mgr_clone, id_c).await; });
    { let mut d = mgr.downloads.lock().await; if let Some(e) = d.get_mut(&id) { e.task = Some(task); } }
    persist_downloads(&mgr.inner()).await;
    Ok(info)
}

#[tauri::command]
async fn pause_download(id: String, mgr: State<'_, DownloadManager>) -> Result<(), String> {
    let task = { let mut d = mgr.downloads.lock().await; if let Some(e) = d.get_mut(&id) { e.cancel_flag.store(true, Ordering::Relaxed); e.task.take() } else { None } };
    if let Some(t) = task { let _ = tokio::time::timeout(Duration::from_secs(5), t).await; }
    persist_downloads(&mgr.inner()).await;
    Ok(())
}

#[tauri::command]
async fn resume_download(app: AppHandle, mgr: State<'_, DownloadManager>, id: String) -> Result<(), String> {
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let mgr_clone = mgr.inner().clone();
    { let mut d = mgr.downloads.lock().await;
      match d.get_mut(&id) {
        Some(e) if e.info.status == "paused" || e.info.status == "error" => { e.cancel_flag = cancel_flag.clone(); e.info.status = "downloading".into(); e.info.error = None; }
        Some(_) => return Err("download is not paused".into()),
        None => return Err("download not found".into()),
      }
    }
    let app_c = app.clone(); let id_c = id.clone();
    let task = tokio::spawn(async move { download_worker(app_c, mgr_clone, id_c).await; });
    { let mut d = mgr.downloads.lock().await; if let Some(e) = d.get_mut(&id) { e.task = Some(task); } }
    persist_downloads(&mgr.inner()).await;
    Ok(())
}

#[tauri::command]
async fn cancel_download(id: String, mgr: State<'_, DownloadManager>) -> Result<(), String> {
    let task = { let mut d = mgr.downloads.lock().await; if let Some(e) = d.get_mut(&id) { e.cancel_flag.store(true, Ordering::Relaxed); e.task.take() } else { None } };
    if let Some(t) = task { let _ = tokio::time::timeout(Duration::from_secs(5), t).await; }
    let dn = { let d = mgr.downloads.lock().await; d.get(&id).map(|e| (e.info.save_dir.clone(), e.info.filename.clone())) };
    if let Some((dir, name)) = dn { let _ = fs::remove_file(part_path(&dir, &name)).await; let _ = fs::remove_file(meta_path(&dir, &name)).await; }
    mgr.downloads.lock().await.remove(&id);
    persist_downloads(&mgr.inner()).await;
    Ok(())
}

#[tauri::command]
async fn list_downloads(mgr: State<'_, DownloadManager>) -> Result<Vec<DownloadInfo>, String> {
    let mut d = mgr.downloads.lock().await;
    for entry in d.values_mut() {
        if entry.info.status == "completed" {
            let fp = final_path(&entry.info.save_dir, &entry.info.filename);
            entry.info.file_missing = !fp.exists();
        }
    }
    Ok(d.values().map(|e| e.info.clone()).collect())
}

#[tauri::command]
async fn remove_download(id: String, mgr: State<'_, DownloadManager>) -> Result<(), String> {
    mgr.downloads.lock().await.remove(&id);
    persist_downloads(&mgr.inner()).await;
    Ok(())
}

#[tauri::command]
async fn redownload_download(app: AppHandle, mgr: State<'_, DownloadManager>, id: String) -> Result<DownloadInfo, String> {
    // Stop current task if running
    let task = {
        let mut d = mgr.downloads.lock().await;
        if let Some(e) = d.get_mut(&id) { e.cancel_flag.store(true, Ordering::Relaxed); e.task.take() } else { None }
    };
    if let Some(t) = task { let _ = tokio::time::timeout(Duration::from_secs(5), t).await; }

    let (save_dir, filename) = {
        let d = mgr.downloads.lock().await;
        match d.get(&id).map(|e| (e.info.save_dir.clone(), e.info.filename.clone())) {
            Some(v) => v,
            None => return Err("download not found".into()),
        }
    };

    // Delete old .part, .meta.json, and final file
    let _ = fs::remove_file(part_path(&save_dir, &filename)).await;
    let _ = fs::remove_file(meta_path(&save_dir, &filename)).await;
    let _ = fs::remove_file(final_path(&save_dir, &filename)).await;

    // Reset info and spawn new worker
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let mgr_clone = mgr.inner().clone();
    {
        let mut d = mgr.downloads.lock().await;
        if let Some(e) = d.get_mut(&id) {
            e.cancel_flag = cancel_flag.clone();
           e.info.downloaded = 0;
           e.info.total_size = 0;
           e.info.status = "downloading".into();
           e.info.error = None;
           e.info.file_missing = false;
       }
   }
   let app_c = app.clone(); let id_c = id.clone();
   let task = tokio::spawn(async move { download_worker(app_c, mgr_clone, id_c).await; });
   { let mut d = mgr.downloads.lock().await; if let Some(e) = d.get_mut(&id) { e.task = Some(task); } }

    let info = { let d = mgr.downloads.lock().await; d.get(&id).map(|e| e.info.clone()) };
    persist_downloads(&mgr.inner()).await;
    Ok(info.unwrap())
}

#[tauri::command]
async fn open_download_dir(save_dir: String, filename: String) -> Result<(), String> {
    let dir = PathBuf::from(&save_dir);
    let file_path = dir.join(&filename);

    if file_path.exists() {
        #[cfg(target_os = "macos")]
        std::process::Command::new("open")
            .args(["-R", &file_path.to_string_lossy()])
            .spawn().map_err(|e| e.to_string())?;
        #[cfg(target_os = "windows")]
        std::process::Command::new("explorer")
            .args([format!("/select,{}", file_path.to_string_lossy())])
            .spawn().map_err(|e| e.to_string())?;
        #[cfg(target_os = "linux")]
        std::process::Command::new("xdg-open")
            .arg(&save_dir)
            .spawn().map_err(|e| e.to_string())?;
    } else {
        #[cfg(target_os = "macos")]
        std::process::Command::new("open")
            .arg(&save_dir)
            .spawn().map_err(|e| e.to_string())?;
        #[cfg(target_os = "windows")]
        std::process::Command::new("explorer")
            .arg(&save_dir)
            .spawn().map_err(|e| e.to_string())?;
        #[cfg(target_os = "linux")]
        std::process::Command::new("xdg-open")
            .arg(&save_dir)
            .spawn().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn get_default_download_dir() -> Result<String, String> {
    if let Some(home) = dirs::home_dir() {
        let dl = home.join("Downloads");
        if dl.exists() { return Ok(dl.to_string_lossy().to_string()); }
        return Ok(home.to_string_lossy().to_string());
    }
    Ok("/tmp".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(DownloadManager::new())
        .setup(|app| {
            let mgr = app.state::<DownloadManager>().inner().clone();
            let path = persist_file_path();
            let records: Vec<DownloadInfo> = match path {
                Some(p) => std::fs::read_to_string(&p).ok()
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_default(),
                None => vec![],
            };
            let mut downloads = mgr.downloads.blocking_lock();
           for mut info in records {
               if info.status == "downloading" {
                   info.status = "paused".into();
               }
                if info.status == "completed" {
                    let fp = final_path(&info.save_dir, &info.filename);
                    info.file_missing = !fp.exists();
                }
               let id = info.id.clone();
                let cancel_flag = Arc::new(AtomicBool::new(false));
                downloads.insert(id, DownloadEntry { info, cancel_flag, task: None });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_download, pause_download, resume_download,
            cancel_download, list_downloads, remove_download, redownload_download,
            open_download_dir, get_default_download_dir,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
