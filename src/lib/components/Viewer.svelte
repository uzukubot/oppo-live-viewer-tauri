<script lang="ts">
  import { onMount } from "svelte";
  import { app, next, prev } from "$lib/state.svelte";
  import { displayDims } from "$lib/types";
  import type { PhotoMeta } from "$lib/types";
  import { photoCache } from "$lib/viewer/photoCache";

  const photo = $derived(app.photos[app.index] ?? null);

  let stage: HTMLDivElement;
  let cssW = $state(0);
  let cssH = $state(0);
  let dpr = $state(1);
  let zoom = $state(1);
  let pan = $state({ x: 0, y: 0 });
  let dragging = $state(false);
  let lastPoint = $state({ x: 0, y: 0 });
  let imgSrc = $state<string | null>(null);
  let imgLoaded = $state(false);
  let imgError = $state(false);

  // 视频
  let videoUrl = $state<string | null>(null);
  let videoError = $state(false);
  let videoDims = $state<{ w: number; h: number } | null>(null);
  let videoEl = $state<HTMLVideoElement | null>(null);
  let videoEnded = $state(false);
  let playMode = $state<"once" | "loop">("once");
  let muted = $state(true);

  function clamp(v: number, lo: number, hi: number) {
    return Math.min(hi, Math.max(lo, v));
  }

  // 适应窗口的布局（未应用 EXIF 旋转后的展示尺寸）
  const fit = $derived.by(() => {
    if (!photo) return null;
    const { w: iw, h: ih } = displayDims(photo);
    if (iw <= 0 || ih <= 0) return null;
    const scale = Math.min(cssW / iw, cssH / ih);
    return { iw, ih, scale };
  });

  // <img> 的尺寸与位置：Chromium 对 <img> 原生解码 Ultra HDR（HDR 屏显示 HDR）
  const imgStyle = $derived.by(() => {
    if (!fit) return null;
    const s = fit.scale * zoom;
    const w = fit.iw * s;
    const h = fit.ih * s;
    return `width:${w}px;height:${h}px;left:${(cssW - w) / 2 + pan.x}px;top:${(cssH - h) / 2 + pan.y}px;`;
  });

  // 切换照片（按 id）时重置视图与视频状态；load_photo 回写元数据（同 id）不重置
  let lastPhotoId: number | null = null;
  $effect(() => {
    const p = photo;
    void p;
    if (p?.id !== lastPhotoId) {
      lastPhotoId = p?.id ?? null;
      zoom = 1;
      pan = { x: 0, y: 0 };
      imgSrc = null;
      imgLoaded = false;
      imgError = false;
      videoUrl = null;
      videoError = false;
      videoDims = null;
      videoEnded = false;
    }
    if (p) {
      const pid = p.id;
      photoCache.ensureJpegUrl(p).then((u) => {
        if (photo?.id === pid) imgSrc = u;
      });
    }
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
    return () => ro.disconnect();
  });

  function zoomAt(factor: number, cx: number, cy: number) {
    if (!photo || !fit) return;
    const oldScale = fit.scale * zoom;
    const newZoom = clamp(zoom * factor, 1, 64);
    const newScale = fit.scale * newZoom;
    // 保持光标下的图像点不动
    const ox = (cssW - fit.iw * oldScale) / 2 + pan.x;
    const oy = (cssH - fit.ih * oldScale) / 2 + pan.y;
    const relX = (cx - ox) / oldScale;
    const relY = (cy - oy) / oldScale;
    const nx = (cssW - fit.iw * newScale) / 2;
    const ny = (cssH - fit.ih * newScale) / 2;
    zoom = newZoom;
    pan = {
      x: cx - nx - relX * newScale,
      y: cy - ny - relY * newScale,
    };
    clampPan(newScale);
  }

  function clampPan(scale?: number) {
    if (!photo || !fit) return;
    const s = scale ?? fit.scale * zoom;
    const dw = fit.iw * s;
    const dh = fit.ih * s;
    const maxX = Math.max(0, (dw - cssW) / 2);
    const maxY = Math.max(0, (dh - cssH) / 2);
    pan = {
      x: clamp(pan.x, -maxX - cssW * 0.15, maxX + cssW * 0.15),
      y: clamp(pan.y, -maxY - cssH * 0.15, maxY + cssH * 0.15),
    };
  }

  function toggleZoom() {
    if (!photo || !fit) return;
    if (zoom <= 1.001) {
      zoom = clamp(1 / (dpr * fit.scale), 1, 64);
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

  // ---- 视频控制 ----
  function onVideoMetadata() {
    if (!videoEl) return;
    videoDims = { w: videoEl.videoWidth, h: videoEl.videoHeight };
    videoEl.play().catch(() => {});
  }
  function onVideoError() {
    videoError = true;
  }
  /** 播一次：结束后回到封面图；循环模式：重新播放。 */
  function onVideoEnded() {
    if (playMode === "loop") {
      if (videoEl) {
        videoEl.currentTime = 0;
        videoEl.play().catch(() => {});
      }
    } else {
      videoEnded = true;
    }
  }
  function replay() {
    if (!videoEl) return;
    videoEl.currentTime = 0;
    videoEnded = false;
    videoEl.play().catch(() => {});
  }
  function toggleLoop() {
    playMode = playMode === "loop" ? "once" : "loop";
    if (playMode === "loop" && videoEnded) replay();
  }
  function toggleMute() {
    muted = !muted;
    if (!muted && videoEl) videoEl.play().catch(() => {});
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
  {#if photo}
    {#if imgSrc}
      <img
        class="view-img"
        src={imgSrc}
        style={imgStyle}
        draggable="false"
        alt=""
        onload={() => (imgLoaded = true)}
        onerror={() => (imgError = true)}
      />
    {/if}
    {#if !imgLoaded && !imgError}
      <div class="loading">加载中…</div>
    {/if}
    {#if imgError}
      <div class="msg err">图片加载失败</div>
    {/if}
  {:else}
    <div class="msg">加载图片…</div>
  {/if}

  {#if videoUrl && !videoError && !videoEnded}
    <div
      class="video-box"
      class:hidden={!vl}
      style={vl ? "width:{vl.boxW}px;height:{vl.boxH}px;" : ""}
    >
      <video
        bind:this={videoEl}
        src={videoUrl}
        autoplay
        muted={muted}
        playsinline
        onloadedmetadata={onVideoMetadata}
        onerror={onVideoError}
        onended={onVideoEnded}
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

  {#if photo?.is_live}
    <div class="video-controls">
      <button class="ctl" onclick={replay} title="从头播放一次">重播</button>
      <button
        class="ctl {playMode === 'loop' ? 'on' : ''}"
        onclick={toggleLoop}
        title="循环播放"
      >
        循环
      </button>
      <button
        class="ctl {muted ? '' : 'on'}"
        onclick={toggleMute}
        title="静音开关"
      >
        {muted ? "静音" : "有声"}
      </button>
    </div>
  {/if}

  {#if at100}
    <div class="zoom-hint">100%</div>
  {/if}
</div>

<style>
  .stage {
    position: relative;
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    background:
      radial-gradient(circle at 50% 40%, #1a1a1a 0%, #0c0c0c 100%);
    user-select: none;
    touch-action: none;
  }

  .view-img {
    position: absolute;
    display: block;
    user-select: none;
    -webkit-user-drag: none;
  }

  .loading,
  .msg {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    color: #8a8f98;
    font-size: 13px;
  }
  .msg.err {
    color: #f08787;
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

  .video-controls {
    position: absolute;
    bottom: 14px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    gap: 8px;
    padding: 6px 10px;
    border-radius: 10px;
    background: rgba(20, 22, 26, 0.82);
    border: 1px solid #2a2d33;
    backdrop-filter: blur(8px);
  }
  .ctl {
    border: 1px solid #34373d;
    background: #23252b;
    color: #c9cdd4;
    border-radius: 7px;
    padding: 5px 12px;
    font-size: 12px;
    cursor: pointer;
  }
  .ctl:hover {
    background: #2c2f36;
  }
  .ctl.on {
    background: #3d6ef7;
    border-color: #3d6ef7;
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
