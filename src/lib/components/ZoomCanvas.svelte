<script lang="ts">
  import { displayDims } from "$lib/types";
  import type { PhotoMeta } from "$lib/types";
  import { photoCache } from "$lib/viewer/photoCache";

  interface Props {
    photo: PhotoMeta | null;
    /** 相对 fit 的缩放倍数（1 = 适应窗口）。 */
    zoom: number;
    /** 平移偏移（css px，相对居中 fit 位置）。 */
    pan: { x: number; y: number };
    cssW: number;
    cssH: number;
    dpr: number;
  }
  let { photo, zoom, pan, cssW, cssH, dpr }: Props = $props();

  let canvas: HTMLCanvasElement;
  let ctx: CanvasRenderingContext2D | null = null;
  let drawSeq = 0;

  function requestDraw() {
    const seq = ++drawSeq;
    requestAnimationFrame(() => {
      if (seq === drawSeq) draw();
    });
  }

  $effect(() => {
    // 依赖跟踪：photo / zoom / pan / 视口
    void photo;
    void zoom;
    void pan.x;
    void pan.y;
    void cssW;
    void cssH;
    void dpr;
    if (!canvas) return;
    canvas.width = Math.max(1, Math.round(cssW * dpr));
    canvas.height = Math.max(1, Math.round(cssH * dpr));
    ctx = ctx ?? canvas.getContext("2d");
    requestDraw();
  });

  async function draw() {
    const seq = drawSeq;
    if (!ctx || !canvas) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, cssW, cssH);
    if (!photo || cssW <= 0 || cssH <= 0) return;
    const { w: iw, h: ih } = displayDims(photo);
    if (iw <= 0 || ih <= 0) return;
    const fitScale = Math.min(cssW / iw, cssH / ih);

    if (zoom <= 1) {
      // fit：用设备像素精度的预下采样位图（高质量）
      const target = Math.round(Math.max(iw, ih) * fitScale * dpr);
      const bmp = await photoCache.ensurePreview(photo, target);
      if (seq !== drawSeq) return;
      if (!bmp) return;
      const dw = iw * fitScale;
      const dh = ih * fitScale;
      const x = (cssW - dw) / 2;
      const y = (cssH - dh) / 2;
      ctx.imageSmoothingEnabled = true;
      ctx.imageSmoothingQuality = "high";
      ctx.drawImage(bmp, x, y, dw, dh);
    } else {
      // 放大：用原尺寸位图
      const bmp = await photoCache.ensureFull(photo);
      if (seq !== drawSeq) return;
      if (!bmp) return;
      const scale = fitScale * zoom;
      const dw = iw * scale;
      const dh = ih * scale;
      const x = (cssW - dw) / 2 + pan.x;
      const y = (cssH - dh) / 2 + pan.y;
      ctx.imageSmoothingEnabled = true;
      ctx.imageSmoothingQuality = "high";
      ctx.drawImage(bmp, x, y, dw, dh);
    }
  }
</script>

<canvas bind:this={canvas} class="zoom-canvas"></canvas>

<style>
  .zoom-canvas {
    width: 100%;
    height: 100%;
    display: block;
  }
</style>
