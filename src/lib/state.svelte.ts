import type { PhotoMeta } from "./types";

/** 全局应用状态（Svelte 5 runes）。 */
export const app = $state({
  photos: [] as PhotoMeta[],
  folder: "",
  index: 0,
  view: "grid" as "grid" | "viewer",
  loading: false,
  error: "" as string,
  lastOpened: "" as string,
  /** 扫描进度（来自 Rust scan-progress 事件）。 */
  scanProgress: null as { scanned: number; total: number } | null,
  /** 文件夹浏览模式：默认文件名列表（省资源），缩略图网格需手动开启。 */
  viewMode: "list" as "list" | "grid",
});

export function setPhotos(folder: string, photos: PhotoMeta[]) {
  app.photos = photos;
  app.folder = folder;
  app.index = 0;
  app.error = "";
}

export function show(index: number) {
  if (index < 0 || index >= app.photos.length) return;
  app.index = index;
  app.view = "viewer";
}

export function next() {
  if (app.photos.length === 0) return;
  app.index = (app.index + 1) % app.photos.length;
  if (app.view !== "viewer") app.view = "viewer";
}

export function prev() {
  if (app.photos.length === 0) return;
  app.index = (app.index - 1 + app.photos.length) % app.photos.length;
  if (app.view !== "viewer") app.view = "viewer";
}
