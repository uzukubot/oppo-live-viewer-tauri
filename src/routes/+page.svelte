<script lang="ts">
  import { onMount } from "svelte";
  import { app, next, prev } from "$lib/state.svelte";
  import TopBar from "$lib/components/TopBar.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import Viewer from "$lib/components/Viewer.svelte";
  import StatusBar from "$lib/components/StatusBar.svelte";
  import Welcome from "$lib/components/Welcome.svelte";
  import { openFolder, openPath } from "$lib/actions";
  import { getCurrentWebview } from "@tauri-apps/api/webview";

  function onKey(e: KeyboardEvent) {
    const tag = (e.target as HTMLElement)?.tagName;
    if (tag === "INPUT" || tag === "TEXTAREA") return;
    if (!app.folder || app.photos.length === 0) return;
    switch (e.key) {
      case "ArrowDown":
      case "ArrowRight":
      case " ":
        e.preventDefault();
        next();
        break;
      case "ArrowUp":
      case "ArrowLeft":
        e.preventDefault();
        prev();
        break;
      case "Home":
        e.preventDefault();
        app.index = 0;
        break;
      case "End":
        e.preventDefault();
        app.index = app.photos.length - 1;
        break;
      case "Escape":
        app.sidebarVisible = !app.sidebarVisible;
        break;
    }
  }

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

    window.addEventListener("keydown", onKey);

    // 恢复上次打开的文件夹
    const last = localStorage.getItem("lastFolder");
    if (last && !app.folder) {
      openFolder(last);
    }
    return () => {
      unlisten?.();
      window.removeEventListener("keydown", onKey);
    };
  });
</script>

<div class="app">
  <TopBar />

  {#if !app.folder}
    <Welcome />
  {:else}
    <div class="pane">
      {#if app.sidebarVisible}
        <Sidebar />
      {/if}
      <div class="main">
        <Viewer />
        <StatusBar />
      </div>
    </div>
  {/if}

  {#if app.error && app.folder}
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

  .pane {
    display: flex;
    flex: 1;
    min-height: 0;
  }

  .main {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    min-height: 0;
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
