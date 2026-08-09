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

/// 扫描目录并返回元数据。运行在后台线程（由 async 命令 + spawn_blocking 调用），
/// 每 50 个文件通过 `scan-progress` 事件上报进度，UI 不会冻结。
fn scan_dir(dir: &str, app: &AppHandle) -> Vec<PhotoMeta> {
    let Ok(paths) = list_image_paths(dir) else {
        return Vec::new();
    };
    let total = paths.len();
    let mut photos = Vec::with_capacity(total);
    for (i, p) in paths.iter().enumerate() {
        let store = app.state::<Mutex<FileStore>>();
        let id = {
            let mut st = match store.lock() {
                Ok(s) => s,
                Err(_) => return photos,
            };
            st.register_path(p)
        };
        if let Ok(meta) = parser::scan_photo(p, id) {
            photos.push(meta);
        }
        // 进度事件（每 50 个或最后一批）
        if i % 50 == 49 || i + 1 == total {
            let _ = app.emit(
                "scan-progress",
                serde_json::json!({ "scanned": i + 1, "total": total }),
            );
        }
    }
    photos
}

/// 扫描文件夹，返回图片列表（含元数据）。只做轻量解析，不缓存字节。
/// async + spawn_blocking：不阻塞主线程，几千张图也不会卡死 UI。
#[tauri::command]
async fn scan_folder(folder: String, app: AppHandle) -> Result<Vec<PhotoMeta>, String> {
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || scan_dir(&folder, &app2))
        .await
        .map_err(|e| format!("扫描失败: {e}"))
}

#[derive(serde::Serialize)]
struct OpenResult {
    folder: String,
    index: i64,
    photos: Vec<PhotoMeta>,
}

/// 打开路径（目录或单个图片文件）：目录则扫描；文件则扫描其所在目录并定位到该文件。
#[tauri::command]
async fn open_path(path: String, app: AppHandle) -> Result<OpenResult, String> {
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
    let app2 = app.clone();
    let folder2 = folder.clone();
    let photos = tauri::async_runtime::spawn_blocking(move || scan_dir(&folder2, &app2))
        .await
        .map_err(|e| format!("扫描失败: {e}"))?;
    let index = match target {
        Some(t) => photos
            .iter()
            .position(|m| m.path == t)
            .map(|i| i as i64)
            .unwrap_or(0),
        None => 0,
    };
    Ok(OpenResult {
        folder,
        index,
        photos,
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
        .invoke_handler(tauri::generate_handler![scan_folder, load_photo, open_path])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
