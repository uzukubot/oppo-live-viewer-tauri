// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod parser;
mod store;

use std::path::PathBuf;
use std::sync::Mutex;

use parser::PhotoMeta;
use store::{CachedFile, FileStore};
use tauri::{AppHandle, Emitter, Manager};

/// 读取目录下的图片路径并自然排序（不涉及 store 锁）。
fn list_image_paths(dir: &str) -> Result<Vec<PathBuf>, String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("无法打开文件夹: {e}"))?;
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && parser::is_supported(p))
        .collect();
    // 自然排序：photo2 < photo10
    paths.sort_by(|a, b| {
        let (an, bn) = (
            a.file_name().and_then(|n| n.to_str()).unwrap_or(""),
            b.file_name().and_then(|n| n.to_str()).unwrap_or(""),
        );
        natsort::natural_compare(an, bn)
    });
    Ok(paths)
}

/// 后台扫描线程：从 store 的扫描状态中逐文件取出路径，解析元数据，
/// 每 25 个通过 `scan-batch` 事件推给前端（前端边收边追加，列表立即可用）。
/// 代际（generation）不匹配时（用户切换了文件夹）立即停止。
fn scan_stream(app: &AppHandle, folder: &str, generation: u64) {
    const BATCH: usize = 25;
    let mut batch: Vec<PhotoMeta> = Vec::with_capacity(BATCH);
    loop {
        let item = {
            let state = app.state::<Mutex<FileStore>>();
            let mut st = match state.lock() {
                Ok(s) => s,
                Err(_) => return,
            };
            let path = {
                let Some(scan) = st.scan.as_mut() else {
                    return;
                };
                if scan.generation != generation {
                    return; // 已被新扫描取代
                }
                if scan.index >= scan.paths.len() {
                    break;
                }
                let p = scan.paths[scan.index].clone();
                scan.index += 1;
                p
            };
            let id = st.register_path(&path);
            (id, path)
        };
        if let Ok(meta) = parser::scan_photo(&item.1, item.0) {
            batch.push(meta);
            if batch.len() >= BATCH {
                let _ = app.emit(
                    "scan-batch",
                    serde_json::json!({ "folder": folder, "photos": batch }),
                );
                batch.clear();
            }
        }
    }
    if !batch.is_empty() {
        let _ = app.emit(
            "scan-batch",
            serde_json::json!({ "folder": folder, "photos": batch }),
        );
    }
    let _ = app.emit("scan-done", serde_json::json!({ "folder": folder }));
}

/// 记录扫描状态并在后台启动流式扫描，立即返回总数（不阻塞、不全量扫描）。
fn begin_scan(app: &AppHandle, folder: &str, paths: Vec<PathBuf>) -> usize {
    let total = paths.len();
    let generation = {
        let state = app.state::<Mutex<FileStore>>();
        let mut st = match state.lock() {
            Ok(s) => s,
            Err(_) => return total,
        };
        st.generation += 1;
        st.scan = Some(store::ScanState {
            paths,
            index: 0,
            generation: st.generation,
        });
        st.generation
    };
    let app2 = app.clone();
    let folder2 = folder.to_string();
    tauri::async_runtime::spawn_blocking(move || scan_stream(&app2, &folder2, generation));
    total
}

#[derive(serde::Serialize)]
struct ScanStart {
    folder: String,
    total: usize,
}

/// 开始流式扫描文件夹：立即返回总数，后台逐批推送 scan-batch / scan-done 事件。
#[tauri::command]
async fn start_scan(folder: String, app: AppHandle) -> Result<ScanStart, String> {
    let folder2 = folder.clone();
    let paths = tauri::async_runtime::spawn_blocking(move || list_image_paths(&folder2))
        .await
        .map_err(|e| format!("扫描失败: {e}"))??;
    let total = begin_scan(&app, &folder, paths);
    Ok(ScanStart { folder, total })
}

#[derive(serde::Serialize)]
struct OpenResult {
    folder: String,
    index: i64,
    total: usize,
}

/// 打开路径（目录或单个图片文件）：计算所在目录与目标在排序列表中的索引，
/// 但**不**启动扫描（由前端随后调用 start_scan 流式填充）。
#[tauri::command]
async fn open_path(path: String) -> Result<OpenResult, String> {
    let p = std::path::Path::new(&path);
    let (folder, target) = if p.is_dir() {
        (path.clone(), None)
    } else if p.is_file() {
        let parent = p
            .parent()
            .map(|x| x.to_string_lossy().into_owned())
            .unwrap_or_else(|| ".".to_string());
        (parent, Some(path))
    } else {
        return Err("路径不存在".to_string());
    };
    let folder2 = folder.clone();
    let paths = tauri::async_runtime::spawn_blocking(move || list_image_paths(&folder2))
        .await
        .map_err(|e| format!("扫描失败: {e}"))??;
    let total = paths.len();
    let index = match target {
        Some(t) => paths
            .iter()
            .position(|p| p.to_string_lossy() == t)
            .map(|i| i as i64)
            .unwrap_or(0),
        None => 0,
    };
    Ok(OpenResult {
        folder,
        index,
        total,
    })
}

/// 打开单张图片：全量读取并精确切分，缓存字节供 URI scheme 提供。
/// 返回全量解析的准确元数据（video_rotation / is_live / mp4_offset 等），
/// 供前端回写修正网格扫描时未填的字段。
#[tauri::command]
async fn load_photo(
    id: u64,
    store: tauri::State<'_, Mutex<FileStore>>,
) -> Result<PhotoMeta, String> {
    let path = {
        let st = store.lock().map_err(|_| "store 锁损坏".to_string())?;
        st.path(id).map(|s| s.to_string())
    }
    .ok_or_else(|| format!("未知 id: {id}"))?;
    let (meta, jpeg, mp4) = parser::full_meta(std::path::Path::new(&path), id)
        .map_err(|e| format!("读取文件失败: {e}"))?;
    let mut st = store.lock().map_err(|_| "store 锁损坏".to_string())?;
    st.insert(id, CachedFile { jpeg, mp4 });
    Ok(meta)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Mutex::new(FileStore::new(8)))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .register_uri_scheme_protocol("viewer", |ctx, request| {
            // 处理 viewer://load/{id}/{jpeg|mp4}
            // URL 只含不透明 id，不泄露文件系统路径。
            let error = |status: u16| {
                tauri::http::Response::builder()
                    .status(status)
                    .body(Vec::new())
                    .unwrap()
            };
            let decoded = percent_encoding::percent_decode_str(request.uri().path())
                .decode_utf8_lossy()
                .into_owned();
            let segs: Vec<&str> = decoded.trim_start_matches('/').split('/').collect();
            if segs.len() != 3 || segs[0] != "load" {
                return error(400);
            }
            let id: u64 = match segs[1].parse() {
                Ok(v) => v,
                Err(_) => return error(400),
            };
            let part = segs[2];
            let state = ctx.app_handle().state::<Mutex<FileStore>>();
            let mut st = match state.lock() {
                Ok(s) => s,
                Err(_) => return error(500),
            };
            let file = match st.get(id) {
                Some(f) => f,
                None => return error(404),
            };
            let (bytes, ctype) = match part {
                "jpeg" => (file.jpeg.clone(), "image/jpeg"),
                "mp4" => match file.mp4.as_ref() {
                    Some(mp4) => (mp4.clone(), "video/mp4"),
                    None => return error(404),
                },
                _ => return error(400),
            };
            tauri::http::Response::builder()
                .header("Content-Type", ctype)
                .header("Access-Control-Allow-Origin", "*")
                .body(bytes)
                .unwrap()
        })
        .invoke_handler(tauri::generate_handler![start_scan, load_photo, open_path])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
