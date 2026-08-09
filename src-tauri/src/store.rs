//! 文件字节缓存（LRU）。自定义 URI scheme 处理器从这里取字节，
//! 避免每次 fetch 都重读磁盘。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct CachedFile {
    /// JPEG 部分（含 gain map / XMP / MPF），可直接交给浏览器。
    pub jpeg: Vec<u8>,
    /// 内嵌 MP4（若为 Live Photo）。
    pub mp4: Option<Vec<u8>>,
}

/// 流式扫描状态：记录已扫描到的位置，供后台线程继续。
pub struct ScanState {
    pub paths: Vec<PathBuf>,
    pub index: usize,
    pub generation: u64,
}

pub struct FileStore {
    map: HashMap<u64, CachedFile>,
    /// id → 磁盘路径。前端只拿不透明 id，避免路径泄漏。
    paths: HashMap<u64, String>,
    /// LRU 顺序，index 0 = 最近使用。
    order: Vec<u64>,
    cap: usize,
    next_id: u64,
    /// 进行中的扫描（若有）。新扫描会替换并递增 generation，旧扫描线程据此停止。
    pub scan: Option<ScanState>,
    pub generation: u64,
}

impl FileStore {
    pub fn new(cap: usize) -> Self {
        Self {
            map: HashMap::new(),
            paths: HashMap::new(),
            order: Vec::new(),
            cap,
            next_id: 1,
            scan: None,
            generation: 0,
        }
    }

    pub fn register_path(&mut self, path: &Path) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.paths.insert(id, path.to_string_lossy().into_owned());
        id
    }

    pub fn path(&self, id: u64) -> Option<&str> {
        self.paths.get(&id).map(|s| s.as_str())
    }

    pub fn insert(&mut self, id: u64, file: CachedFile) {
        self.map.insert(id, file);
        self.order.retain(|&x| x != id);
        self.order.insert(0, id);
        while self.order.len() > self.cap {
            if let Some(evict) = self.order.pop() {
                if evict != id {
                    self.map.remove(&evict);
                }
            }
        }
    }

    pub fn get(&mut self, id: u64) -> Option<&CachedFile> {
        if self.map.contains_key(&id) {
            self.order.retain(|&x| x != id);
            self.order.insert(0, id);
        }
        self.map.get(&id)
    }
}
