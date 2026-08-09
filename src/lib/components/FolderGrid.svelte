<script lang="ts">
  import { onMount } from "svelte";
  import { app, show } from "$lib/state.svelte";
  import ThumbCell from "./ThumbCell.svelte";

  const CELL = 200;
  const GAP = 14;

  let gridEl: HTMLDivElement;
  let scrollTop = $state(0);
  let viewportH = $state(600);
  let width = $state(0);
  let sel = $state(0);

  const cols = $derived(Math.max(1, Math.floor((width + GAP) / (CELL + GAP))));
  const totalRows = $derived(Math.max(0, Math.ceil(app.photos.length / cols)));
  const totalH = $derived(totalRows * (CELL + GAP) + GAP);

  const winStart = $derived(
    Math.max(0, Math.floor(scrollTop / (CELL + GAP)) - 1),
  );
  const winEnd = $derived(
    Math.min(
      totalRows - 1,
      winStart + Math.ceil(viewportH / (CELL + GAP)) + 2,
    ),
  );

  const cells = $derived.by(() => {
    const list: { index: number; x: number; y: number }[] = [];
    for (let r = winStart; r <= winEnd; r++) {
      for (let c = 0; c < cols; c++) {
        const index = r * cols + c;
        if (index >= app.photos.length) break;
        list.push({
          index,
          x: c * (CELL + GAP) + GAP,
          y: r * (CELL + GAP) + GAP,
        });
      }
    }
    return list;
  });

  function onScroll() {
    if (!gridEl) return;
    scrollTop = gridEl.scrollTop;
    viewportH = gridEl.clientHeight;
  }

  function scrollToRow(index: number) {
    const row = Math.floor(index / cols);
    const y = row * (CELL + GAP);
    gridEl?.scrollTo({ top: y, behavior: "smooth" });
  }

  function onKey(e: KeyboardEvent) {
    const tag = (e.target as HTMLElement)?.tagName;
    if (tag === "INPUT" || tag === "TEXTAREA") return;
    if (app.photos.length === 0) return;
    switch (e.key) {
      case "ArrowRight":
        e.preventDefault();
        sel = Math.min(app.photos.length - 1, sel + 1);
        scrollToRow(sel);
        break;
      case "ArrowLeft":
        e.preventDefault();
        sel = Math.max(0, sel - 1);
        scrollToRow(sel);
        break;
      case "ArrowDown":
        e.preventDefault();
        sel = Math.min(app.photos.length - 1, sel + cols);
        scrollToRow(sel);
        break;
      case "ArrowUp":
        e.preventDefault();
        sel = Math.max(0, sel - cols);
        scrollToRow(sel);
        break;
      case "Enter":
        e.preventDefault();
        show(sel);
        break;
    }
  }

  onMount(() => {
    const ro = new ResizeObserver(() => {
      if (!gridEl) return;
      width = gridEl.clientWidth;
      viewportH = gridEl.clientHeight;
    });
    ro.observe(gridEl);
    window.addEventListener("keydown", onKey);
    return () => {
      ro.disconnect();
      window.removeEventListener("keydown", onKey);
    };
  });
</script>

<div class="grid" bind:this={gridEl} onscroll={onScroll}>
  <div class="inner" style="height:{totalH}px;">
    {#each cells as c (c.index)}
      <button
        class="cell"
        style="left:{c.x}px;top:{c.y}px;width:{CELL}px;height:{CELL}px;"
        onclick={() => {
          sel = c.index;
          show(c.index);
        }}
      >
        <ThumbCell meta={app.photos[c.index]} selected={sel === c.index} />
      </button>
    {/each}
  </div>
</div>

<style>
  .grid {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
    background: #121212;
  }
  .grid::-webkit-scrollbar {
    width: 10px;
  }
  .grid::-webkit-scrollbar-thumb {
    background: #333;
    border-radius: 5px;
  }

  .inner {
    position: relative;
  }

  .cell {
    position: absolute;
    padding: 0;
    border: none;
    background: none;
    cursor: pointer;
  }
</style>
