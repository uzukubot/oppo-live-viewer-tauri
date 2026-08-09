import { invoke } from "@tauri-apps/api/core";
import type { PhotoMeta } from "./types";

export interface ScanStart {
  folder: string;
  total: number;
}

/**
 * 开始流式扫描文件夹：Rust 立即返回总数，并在后台逐批推送
 * `scan-batch` / `scan-done` 事件（见 actions.ts），前端边收边追加。
 */
export async function startScan(folder: string): Promise<ScanStart> {
  return invoke<ScanStart>("start_scan", { folder });
}

/**
 * 打开单张图片：Rust 全量读取并切分、缓存字节。
 * 返回全量解析的准确元数据（video_rotation / is_live / mp4_offset 等），
 * 供前端回写修正扫描时的占位值。
 */
export async function loadPhoto(id: number): Promise<PhotoMeta> {
  return invoke<PhotoMeta>("load_photo", { id });
}

export interface OpenResult {
  folder: string;
  index: number;
  total: number;
}

/** 打开路径（目录或单个图片文件）：计算目录与目标索引，不启动扫描。 */
export async function openPath(path: string): Promise<OpenResult> {
  return invoke<OpenResult>("open_path", { path });
}
