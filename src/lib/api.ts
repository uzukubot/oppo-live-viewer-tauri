import { invoke } from "@tauri-apps/api/core";
import type { PhotoMeta } from "./types";

/** 扫描文件夹，返回图片列表（含元数据）。 */
export async function scanFolder(folder: string): Promise<PhotoMeta[]> {
  return invoke<PhotoMeta[]>("scan_folder", { folder });
}

/**
 * 打开单张图片：Rust 全量读取并切分、缓存字节。
 * 返回全量解析的准确元数据（video_rotation / is_live / mp4_offset 等），
 * 供前端回写修正网格扫描时的占位值。
 */
export async function loadPhoto(id: number): Promise<PhotoMeta> {
  return invoke<PhotoMeta>("load_photo", { id });
}

export interface OpenResult {
  folder: string;
  index: number;
  photos: PhotoMeta[];
}

/** 打开路径（目录或单个图片文件）。 */
export async function openPath(path: string): Promise<OpenResult> {
  return invoke<OpenResult>("open_path", { path });
}
