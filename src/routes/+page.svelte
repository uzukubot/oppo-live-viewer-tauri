<script lang="ts">
  import { onMount } from "svelte";
  import { app } from "$lib/state.svelte";
  import TopBar from "$lib/components/TopBar.svelte";
  import FolderGrid from "$lib/components/FolderGrid.svelte";
  import Viewer from "$lib/components/Viewer.svelte";
  import StatusBar from "$lib/components/StatusBar.svelte";
  import Welcome from "$lib/components/Welcome.svelte";
  import { openFolder, openPath } from "$lib/actions";
  import { getCurrentWebview } from "@tauri-apps/api/webview";

  const showGrid = $derived(
    app.view === "grid" && app.photos.length > 0,
  );

  onMount(() => {
    let unlisten: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent((e) => {
        if (e.payload.type === "drop") {
          const p = e.payload.paths[0];
          if (p) openPath(p);
        }
      })
      .then((f) => (unlisten = f));

    // 恢复上次打开的文件夹
    const last = localStorage.getItem("lastFolder");
    if (last && !app.folder) {
      openFolder(last);
    }
    return () => unlisten?.();
  });
</script>

<div class="app">
  <TopBar />

  {#if app.loading}
    <div class="center">扫描中…</div>
  {:else if !app.folder}
    <Welcome />
  {:else if app.photos.length === 0}
    <div class="center err">{app.error || "没有支持的图片"}</div>
  {:else if showGrid}
    <FolderGrid />
  {:else}
    <Viewer />
    <StatusBar />
  {/if}

  {#if app.error && app.photos.length > 0}
    <div class="toast">{app.error}</div>
  {/if}
</div>

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: #121212;
    color: #e8e8e8;
  }

  .center {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #8a8f98;
    font-size: 14px;
  }
  .center.err {
    color: #f08787;
  }

  .toast {
    position: fixed;
    bottom: 52px;
    left: 50%;
    transform: translateX(-50%);
    background: rgba(240, 84, 84, 0.92);
    color: #fff;
    padding: 10px 18px;
    border-radius: 10px;
    font-size: 13px;
    box-shadow: 0 6px 24px rgba(0, 0, 0, 0.4);
    z-index: 100;
  }
</style>
