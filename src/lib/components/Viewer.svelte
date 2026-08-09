<script lang="ts">
  import { onMount } from "svelte";
  import { app, next, prev } from "$lib/state.svelte";
  import { displayDims } from "$lib/types";
  import type { PhotoMeta } from "$lib/types";
  import { photoCache } from "$lib/viewer/photoCache";
  import ZoomCanvas from "./ZoomCanvas.svelte";

  const photo = $derived(app.photos[app.index] ?? null);

  let stage: HTMLDivElement;
  let cssW = $state(0);
  let cssH = $state(0);
  let dpr = $state(1);
  let zoom = $state(1);
  let pan = $state({ x: 0, y: 0 });
  let dragging = $state(false);
  let lastPoint = $state({ x: 0, y: 0 });

  let videoUrl = $state<string | null>(null);
  let videoError = $state(false);
  let videoMuted = $state(true);
  let videoDims = $state<{ w: number; h: number } | null>(null);
  let videoEl = $state<HTMLVideoElement | null>(null);

  function clamp(v: number, lo: number, hi: number) {
    return Math.min(hi, Math.max(lo, v));
  }

  function fitScaleOf(p: PhotoMeta | null) {
    if (!p) return 1;
    const { w: iw, h: ih } = displayDims(p);
    if (iw <= 0 || ih <= 0) return 1;
    return Math.min(cssW / iw, cssH / ih);
  }

  // 切换照片时重置视图状态
  $effect(() => {
    const p = photo;
    void p;
    zoom = 1;
    pan = { x: 0, y: 0 };
    videoUrl = null;
    videoError = false;
    videoDims = null;
    if (p?.is_live) {
      const pid = p.id;
      photoCache.ensureMp4Url(p).then((u) => {
        if (u && photo?.id === pid) videoUrl = u;
      });
    }
    // 预取相邻照片，快速翻看
    photoCache.prefetchNearby(app.photos, app.index, 2);
  });

  function measure() {
    if (!stage) return;
    const rect = stage.getBoundingClientRect();
    cssW = rect.width;
    cssH = rect.height;
    dpr = window.devicePixelRatio || 1;
  }

  onMount(() => {
    const ro = new ResizeObserver(measure);
    ro.observe(stage);
    measure();
    window.addEventListener("keydown", onKey);
    return () => {
      ro.disconnect();
      window.removeEventListener("keydown", onKey);
    };
  });

  function onKey(e: KeyboardEvent) {
    const tag = (e.target as HTMLElement)?.tagName;
    if (tag === "INPUT" || tag === "TEXTAREA") return;
    switch (e.key) {
      case "ArrowRight":
      case " ":
        e.preventDefault();
        next();
        break;
      case "ArrowLeft":
        e.preventDefault();
        prev();
        break;
      case "Escape":
        app.view = "grid";
        break;
      case "Home":
        e.preventDefault();
        app.index = 0;
        break;
      case "End":
        e.preventDefault();
        app.index = app.photos.length - 1;
        break;
      case "+":
      case "=":
        zoomAt(1.25, cssW / 2, cssH / 2);
        break;
      case "-":
      case "_":
        zoomAt(0.8, cssW / 2, cssH / 2);
        break;
      case "0":
        zoom = 1;
        pan = { x: 0, y: 0 };
        break;
    }
  }

  function zoomAt(factor: number, cx: number, cy: number) {
    if (!photo) return;
    const { w: iw, h: ih } = displayDims(photo);
    const fitScale = Math.min(cssW / iw, cssH / ih);
    const oldScale = fitScale * zoom;
    const newZoom = clamp(zoom * factor, 1, 64);
    const newScale = fitScale * newZoom;
    // 保持光标下的图像点不动
    const ox = (cssW - iw * oldScale) / 2 + pan.x;
    const oy = (cssH - ih * oldScale) / 2 + pan.y;
    const relX = (cx - ox) / oldScale;
    const relY = (cy - oy) / oldScale;
    const nx = (cssW - iw * newScale) / 2;
    const ny = (cssH - ih * newScale) / 2;
    zoom = newZoom;
    pan = {
      x: cx - nx - relX * newScale,
      y: cy - ny - relY * newScale,
    };
    clampPan(newScale);
  }

  function clampPan(scale?: number) {
    if (!photo) return;
    const { w: iw, h: ih } = displayDims(photo);
    const fitScale = Math.min(cssW / iw, cssH / ih);
    const s = scale ?? fitScale * zoom;
    const dw = iw * s;
    const dh = ih * s;
    const maxX = Math.max(0, (dw - cssW) / 2);
    const maxY = Math.max(0, (dh - cssH) / 2);
    pan = {
      x: clamp(pan.x, -maxX - cssW * 0.15, maxX + cssW * 0.15),
      y: clamp(pan.y, -maxY - cssH * 0.15, maxY + cssH * 0.15),
    };
  }

  function toggleZoom() {
    if (!photo) return;
    if (zoom <= 1.001) {
      const fs = fitScaleOf(photo);
      zoom = clamp(1 / (dpr * fs), 1, 64);
      pan = { x: 0, y: 0 };
    } else {
      zoom = 1;
      pan = { x: 0, y: 0 };
    }
  }

  function onWheel(e: WheelEvent) {
    e.preventDefault();
    const rect = stage.getBoundingClientRect();
    const cx = e.clientX - rect.left;
    const cy = e.clientY - rect.top;
    if (e.ctrlKey || e.shiftKey) {
      zoomAt(e.deltaY < 0 ? 1.12 : 1 / 1.12, cx, cy);
    } else {
      // 滚轮 = 浏览上一张/下一张
      if (e.deltaY > 0) next();
      else prev();
    }
  }

  function onPointerDown(e: PointerEvent) {
    if (zoom <= 1.001) return;
    dragging = true;
    lastPoint = { x: e.clientX, y: e.clientY };
    stage.setPointerCapture(e.pointerId);
  }
  function onPointerMove(e: PointerEvent) {
    if (!dragging) return;
    pan = {
      x: pan.x + (e.clientX - lastPoint.x),
      y: pan.y + (e.clientY - lastPoint.y),
    };
    lastPoint = { x: e.clientX, y: e.clientY };
    clampPan();
  }
  function onPointerUp() {
    dragging = false;
  }

  function onVideoMetadata() {
    if (!videoEl) return;
    videoDims = { w: videoEl.videoWidth, h: videoEl.videoHeight };
    videoEl.play().catch(() => {});
  }
  function onVideoError() {
    videoError = true;
  }
  function toggleMute() {
    if (!videoEl) return;
    videoMuted = !videoMuted;
    if (!videoMuted) videoEl.play().catch(() => {});
  }

  function videoLayout() {
    if (!videoDims) return null;
    const deg = photo?.video_rotation ?? 0;
    let dw = videoDims.w;
    let dh = videoDims.h;
    if (deg === 90 || deg === 270) {
      const t = dw;
      dw = dh;
      dh = t;
    }
    const scale = Math.min(cssW / dw, cssH / dh);
    const boxW = dw * scale;
    const boxH = dh * scale;
    if (deg === 90 || deg === 270) {
      return { boxW, boxH, vidW: boxH, vidH: boxW, deg };
    }
    return { boxW, boxH, vidW: boxW, vidH: boxH, deg };
  }

  const vl = $derived(videoLayout());
  const at100 = $derived(zoom > 1.001);
</script>

<div
  class="stage"
  bind:this={stage}
  onwheel={onWheel}
  onpointerdown={onPointerDown}
  onpointermove={onPointerMove}
  onpointerup={onPointerUp}
  ondblclick={toggleZoom}
  role="application"
  aria-label="照片查看区域"
>
  <ZoomCanvas {photo} {zoom} {pan} {cssW} {cssH} {dpr} />

  {#if videoUrl && !videoError}
    <div
      class="video-box"
      class:hidden={!vl}
      style={vl ? "width:{vl.boxW}px;height:{vl.boxH}px;" : ""}
      onclick={toggleMute}
      onkeydown={(e) => e.key === "Enter" && toggleMute()}
      role="button"
      tabindex="0"
      title={videoMuted ? "点击开启声音" : "点击静音"}
    >
      <video
        bind:this={videoEl}
        src={videoUrl}
        autoplay
        loop
        muted={videoMuted}
        playsinline
        onloadedmetadata={onVideoMetadata}
        onerror={onVideoError}
        style={vl
          ? "width:{vl.vidW}px;height:{vl.vidH}px;transform:translate(-50%,-50%) rotate({vl.deg}deg);"
          : "width:1px;height:1px;transform:translate(-50%,-50%);"}
      ></video>
    </div>
  {/if}

  <div class="badges">
    {#if photo?.is_live}
      <span class="badge live">LIVE</span>
    {/if}
    {#if photo?.ultra_hdr}
      <span class="badge hdr">HDR</span>
    {/if}
  </div>

  {#if at100}
    <div class="zoom-hint">100%</div>
  {/if}
</div>

<style>
  .stage {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow: hidden;
    cursor: default;
    background:
      radial-gradient(circle at 50% 40%, #1a1a1a 0%, #0c0c0c 100%);
    user-select: none;
    touch-action: none;
  }

  .video-box {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    overflow: hidden;
  }

  .video-box.hidden {
    visibility: hidden;
  }

  .video-box video {
    position: absolute;
    left: 50%;
    top: 50%;
    cursor: pointer;
  }

  .badges {
    position: absolute;
    top: 14px;
    left: 14px;
    display: flex;
    gap: 8px;
    pointer-events: none;
  }

  .badge {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 1px;
    padding: 4px 10px;
    border-radius: 999px;
    backdrop-filter: blur(8px);
  }

  .badge.live {
    background: rgba(255, 59, 92, 0.85);
    color: #fff;
  }

  .badge.hdr {
    background: rgba(24, 160, 251, 0.85);
    color: #fff;
  }

  .zoom-hint {
    position: absolute;
    top: 14px;
    right: 14px;
    font-size: 12px;
    color: #bbb;
    background: rgba(0, 0, 0, 0.55);
    padding: 4px 10px;
    border-radius: 999px;
    pointer-events: none;
    backdrop-filter: blur(8px);
  }
</style>
