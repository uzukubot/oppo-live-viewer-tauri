import { loadPhoto } from "../api";
import { fetchBlob, decodeBitmap } from "../protocol";
import type { PhotoMeta } from "../types";

interface Entry {
  blob?: Blob;
  /** 适应窗口用的高质量下采样位图。 */
  preview?: ImageBitmap;
  previewTarget?: number;
  /** 100% 缩放用的原尺寸位图。 */
  full?: ImageBitmap;
  mp4Url?: string;
  loadingBlob?: Promise<Blob>;
}

const CACHE_CAP = 8;
const MEMORY_BUDGET = 320 * 1024 * 1024; // 320MB

class PhotoCache {
  private entries = new Map<number, Entry>();
  private order: number[] = [];
  private used = 0;
  private inFlight = new Set<number>();

  private touch(id: number) {
    this.order = this.order.filter((x) => x !== id);
    this.order.unshift(id);
    this.evict();
  }

  private evict() {
    while (this.order.length > CACHE_CAP) {
      const evict = this.order.pop();
      if (evict == null) break;
      this.drop(evict);
    }
  }

  private drop(id: number) {
    const e = this.entries.get(id);
    if (!e) return;
    if (e.preview) {
      this.used -= e.preview.width * e.preview.height * 4;
      e.preview.close();
    }
    if (e.full) {
      this.used -= e.full.width * e.full.height * 4;
      e.full.close();
    }
    if (e.mp4Url) URL.revokeObjectURL(e.mp4Url);
    this.entries.delete(id);
  }

  /** 确保 id 对应的 JPEG blob 已加载。 */
  async ensureBlob(meta: PhotoMeta): Promise<Blob> {
    let e = this.entries.get(meta.id);
    if (e?.blob) {
      this.touch(meta.id);
      return e.blob;
    }
    if (this.inFlight.has(meta.id)) {
      await this.entries.get(meta.id)!.loadingBlob;
      return this.entries.get(meta.id)!.blob!;
    }
    this.inFlight.add(meta.id);
    const promise = (async () => {
      await loadPhoto(meta.id);
      return fetchBlob(meta.id, "jpeg");
    })();
    let entry = this.entries.get(meta.id);
    if (!entry) {
      entry = {};
      this.entries.set(meta.id, entry);
    }
    entry.loadingBlob = promise;
    try {
      const blob = await promise;
      entry.blob = blob;
      this.touch(meta.id);
      return blob;
    } finally {
      this.inFlight.delete(meta.id);
    }
  }

  /** 确保适应窗口的预览位图已就绪（目标为设备像素最大边）。 */
  async ensurePreview(meta: PhotoMeta, target: number): Promise<ImageBitmap | null> {
    if (!this.entries.has(meta.id)) this.entries.set(meta.id, {});
    const e = this.entries.get(meta.id)!;
    if (e.preview && e.previewTarget! >= target * 0.6) {
      this.touch(meta.id);
      return e.preview;
    }
    try {
      const blob = await this.ensureBlob(meta);
      const bmp = await decodeBitmap(
        blob,
        target,
        meta.width,
        meta.height,
        meta.orientation,
      );
      if (this.used > MEMORY_BUDGET) this.evictMem();
      e.preview = bmp;
      e.previewTarget = target;
      this.used += bmp.width * bmp.height * 4;
      this.touch(meta.id);
      return bmp;
    } catch (err) {
      console.error("预览解码失败", meta.id, err);
      return null;
    }
  }

  /** 确保原尺寸位图（用于 100% 缩放）。 */
  async ensureFull(meta: PhotoMeta): Promise<ImageBitmap | null> {
    if (!this.entries.has(meta.id)) this.entries.set(meta.id, {});
    const e = this.entries.get(meta.id)!;
    if (e.full) {
      this.touch(meta.id);
      return e.full;
    }
    try {
      const blob = await this.ensureBlob(meta);
      const bmp = await decodeBitmap(blob);
      if (this.used > MEMORY_BUDGET) this.evictMem();
      e.full = bmp;
      this.used += bmp.width * bmp.height * 4;
      this.touch(meta.id);
      return bmp;
    } catch (err) {
      console.error("原图解码失败", meta.id, err);
      return null;
    }
  }

  /** 获取 MP4 的 objectURL（Live Photo）。 */
  async ensureMp4Url(meta: PhotoMeta): Promise<string | null> {
    if (!this.entries.has(meta.id)) this.entries.set(meta.id, {});
    const e = this.entries.get(meta.id)!;
    if (e.mp4Url) return e.mp4Url;
    if (!meta.is_live) return null;
    try {
      await this.ensureBlob(meta); // 确保已 load_photo
      const blob = await fetchBlob(meta.id, "mp4");
      e.mp4Url = URL.createObjectURL(blob);
      return e.mp4Url;
    } catch {
      return null;
    }
  }

  /** 释放最老的一个位图（内存压力时）。 */
  private evictMem() {
    for (let i = this.order.length - 1; i >= 0; i--) {
      const id = this.order[i];
      const e = this.entries.get(id);
      if (!e) continue;
      const full = e.full;
      if (full) {
        this.used -= full.width * full.height * 4;
        full.close();
        e.full = undefined;
      }
      const preview = e.preview;
      if (preview) {
        this.used -= preview.width * preview.height * 4;
        preview.close();
        e.preview = undefined;
      }
      if (this.used <= MEMORY_BUDGET * 0.8) break;
    }
  }

  // ---- 缩略图缓存（网格用） ----
  private thumbs = new Map<number, ImageBitmap>();
  private thumbOrder: number[] = [];

  private touchThumb(id: number) {
    this.thumbOrder = this.thumbOrder.filter((x) => x !== id);
    this.thumbOrder.unshift(id);
  }

  async ensureThumb(meta: PhotoMeta): Promise<ImageBitmap | null> {
    if (this.thumbs.has(meta.id)) {
      this.touchThumb(meta.id);
      return this.thumbs.get(meta.id)!;
    }
    try {
      const blob = await this.ensureBlob(meta);
      const bmp = await decodeBitmap(
        blob,
        THUMB_MAX,
        meta.width,
        meta.height,
        meta.orientation,
      );
      this.thumbs.set(meta.id, bmp);
      this.touchThumb(meta.id);
      while (this.thumbOrder.length > 200) {
        const ev = this.thumbOrder.pop();
        if (ev != null) {
          this.thumbs.get(ev)?.close();
          this.thumbs.delete(ev);
        }
      }
      return bmp;
    } catch (err) {
      console.error("缩略图解码失败", meta.id, err);
      return null;
    }
  }

  clear() {
    for (const id of [...this.entries.keys()]) this.drop(id);
    this.order = [];
    this.used = 0;
    for (const id of [...this.thumbs.keys()]) {
      this.thumbs.get(id)?.close();
      this.thumbs.delete(id);
    }
    this.thumbOrder = [];
  }

  /** 预取邻近照片的 blob + 预览。 */
  async prefetchNearby(photos: PhotoMeta[], index: number, radius = 2) {
    for (let d = 1; d <= radius; d++) {
      for (const i of [index - d, index + d]) {
        if (i < 0 || i >= photos.length) continue;
        const meta = photos[i];
        if (this.entries.has(meta.id) && this.entries.get(meta.id)!.blob) continue;
        this.ensureBlob(meta).catch(() => {});
      }
    }
  }
}

export const photoCache = new PhotoCache();

/** 缩略图尺寸（设备像素）。 */
export const THUMB_MAX = 360;

export interface ThumbResult {
  bitmap: ImageBitmap;
  width: number;
  height: number;
}
