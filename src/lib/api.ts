import { invoke } from "@tauri-apps/api/core";
import type { PhotoMeta } from "./types";

/** 扫描文件夹，返回图片列表（含元数据）。 */
export async function scanFolder(folder: string): Promise<PhotoMeta[]> {
  return invoke<PhotoMeta[]>("scan_folder", { folder });
}

/** 打开单张图片：Rust 读取并切分、缓存字节。 */
export async function loadPhoto(id: number): Promise<void> {
  return invoke("load_photo", { id });
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
