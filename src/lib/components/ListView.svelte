<script lang="ts">
  import { onMount } from "svelte";
  import { app, show } from "$lib/state.svelte";
  import { displayDims } from "$lib/types";

  const ROW = 44;
  let listEl: HTMLDivElement;
  let scrollTop = $state(0);
  let viewportH = $state(600);
  let sel = $state(0);

  const total = $derived(app.photos.length);
  const totalH = $derived(total * ROW);
  const winStart = $derived(Math.max(0, Math.floor(scrollTop / ROW) - 2));
  const winEnd = $derived(
    Math.min(total - 1, winStart + Math.ceil(viewportH / ROW) + 4),
  );
  const rows = $derived.by(() => {
    const out: number[] = [];
    for (let i = winStart; i <= winEnd; i++) out.push(i);
    return out;
  });

  function onScroll() {
    if (!listEl) return;
    scrollTop = listEl.scrollTop;
    viewportH = listEl.clientHeight;
  }

  function scrollToSel() {
    listEl?.scrollTo({ top: sel * ROW, behavior: "smooth" });
  }

  function onKey(e: KeyboardEvent) {
    const tag = (e.target as HTMLElement)?.tagName;
    if (tag === "INPUT" || tag === "TEXTAREA") return;
    if (app.photos.length === 0) return;
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        sel = Math.min(total - 1, sel + 1);
        scrollToSel();
        break;
      case "ArrowUp":
        e.preventDefault();
        sel = Math.max(0, sel - 1);
        scrollToSel();
        break;
      case "Enter":
        e.preventDefault();
        show(sel);
        break;
    }
  }

  onMount(() => {
    const ro = new ResizeObserver(() => {
      if (listEl) viewportH = listEl.clientHeight;
    });
    ro.observe(listEl);
    window.addEventListener("keydown", onKey);
    return () => {
      ro.disconnect();
      window.removeEventListener("keydown", onKey);
    };
  });
</script>

<div class="list" bind:this={listEl} onscroll={onScroll}>
  <div class="inner" style="height:{totalH}px;">
    {#each rows as i (i)}
      <button
        class="row {sel === i ? 'selected' : ''}"
        style="top:{i * ROW}px;height:{ROW}px;"
        onclick={() => {
          sel = i;
          show(i);
        }}
      >
        <span class="name" title={app.photos[i].name}>{app.photos[i].name}</span>
        <span class="meta">
          {#if app.photos[i].is_live}
            <span class="tag live">LIVE</span>
          {/if}
          {#if app.photos[i].ultra_hdr}
            <span class="tag hdr">HDR</span>
          {/if}
          <span class="dims">{displayDims(app.photos[i]).w}×{displayDims(app.photos[i]).h}</span>
        </span>
      </button>
    {/each}
  </div>

  {#if app.scanning}
    <div class="scan-footer">已加载 {app.photos.length} / {app.scanTotal}，正在扫描…</div>
  {/if}
</div>

<style>
  .list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
    background: #121212;
  }
  .list::-webkit-scrollbar {
    width: 10px;
  }
  .list::-webkit-scrollbar-thumb {
    background: #333;
    border-radius: 5px;
  }

  .inner {
    position: relative;
  }

  .scan-footer {
    position: sticky;
    bottom: 0;
    padding: 8px 18px;
    font-size: 12px;
    color: #8a8f98;
    background: linear-gradient(transparent, #121212 40%);
    text-align: center;
    pointer-events: none;
  }

  .row {
    position: absolute;
    left: 0;
    right: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 0 18px;
    border: none;
    border-bottom: 1px solid #1c1e22;
    background: transparent;
    color: #cfd3da;
    font-size: 13.5px;
    text-align: left;
    cursor: pointer;
  }
  .row:hover {
    background: #1b1d21;
  }
  .row.selected {
    background: rgba(61, 110, 247, 0.16);
  }

  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: ui-monospace, "SF Mono", Menlo, Consolas, monospace;
    font-size: 12.5px;
  }

  .meta {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: none;
  }

  .tag {
    font-size: 9.5px;
    font-weight: 700;
    letter-spacing: 0.5px;
    padding: 2px 7px;
    border-radius: 999px;
  }
  .tag.live {
    background: rgba(255, 59, 92, 0.2);
    color: #ff6b82;
  }
  .tag.hdr {
    background: rgba(24, 160, 251, 0.18);
    color: #4cb3ff;
  }

  .dims {
    color: #5a5e66;
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }
</style>
