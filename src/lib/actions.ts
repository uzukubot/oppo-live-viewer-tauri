import { open } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { openPath as apiOpenPath, scanFolder } from "./api";
import { app, setPhotos } from "./state.svelte";
import { photoCache } from "./viewer/photoCache";

/** 弹出文件夹选择框并打开。 */
export async function pickFolder() {
  const dir = await open({
    directory: true,
    title: "选择照片文件夹",
  });
  if (typeof dir === "string") {
    await openFolder(dir);
  }
}

/** 在扫描期间监听进度事件，并在结束后清理。 */
async function withScanProgress<T>(fn: () => Promise<T>): Promise<T> {
  const unlisten = await listen<{ scanned: number; total: number }>(
    "scan-progress",
    (e) => {
      app.scanProgress = e.payload;
    },
  );
  try {
    return await fn();
  } finally {
    app.scanProgress = null;
    unlisten();
  }
}

/** 打开指定文件夹并扫描图片。 */
export async function openFolder(folder: string) {
  app.loading = true;
  app.error = "";
  try {
    const photos = await withScanProgress(() => scanFolder(folder));
    photoCache.clear();
    setPhotos(folder, photos);
    rememberFolder(folder);
    if (photos.length === 0) {
      app.error = "该文件夹中没有支持的图片";
    }
  } catch (e) {
    app.error = e instanceof Error ? e.message : String(e);
  } finally {
    app.loading = false;
  }
}

function rememberFolder(folder: string) {
  app.lastOpened = folder;
  try {
    localStorage.setItem("lastFolder", folder);
  } catch {
    /* 忽略存储失败 */
  }
}

/** 打开路径（拖拽或命令行）：目录则进网格，文件则直接进查看器。 */
export async function openPath(path: string) {
  app.loading = true;
  app.error = "";
  try {
    const res = await withScanProgress(() => apiOpenPath(path));
    photoCache.clear();
    setPhotos(res.folder, res.photos);
    rememberFolder(res.folder);
    if (res.photos.length === 0) {
      app.error = "该文件夹中没有支持的图片";
    } else {
      app.index = res.index;
      app.view = "viewer";
    }
  } catch (e) {
    app.error = e instanceof Error ? e.message : String(e);
  } finally {
    app.loading = false;
  }
}
