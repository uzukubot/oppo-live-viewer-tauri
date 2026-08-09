<script lang="ts">
  import { app } from "$lib/state.svelte";
  import ListView from "./ListView.svelte";
  import FolderGrid from "./FolderGrid.svelte";
</script>

<aside class="sidebar">
  <div class="side-header">
    <span class="count">
      {#if app.scanning}
        已加载 {app.photos.length} / {app.scanTotal}
      {:else}
        共 {app.scanTotal} 张
      {/if}
    </span>
    <div class="seg">
      <button
        class="seg-btn {app.viewMode === 'list' ? 'on' : ''}"
        onclick={() => (app.viewMode = "list")}
        title="文件名列表（省资源）"
      >
        列表
      </button>
      <button
        class="seg-btn {app.viewMode === 'grid' ? 'on' : ''}"
        onclick={() => (app.viewMode = "grid")}
        title="缩略图网格"
      >
        缩略图
      </button>
    </div>
    <button
      class="collapse"
      onclick={() => (app.sidebarVisible = false)}
      title="隐藏侧边栏"
    >
      «
    </button>
  </div>

  {#if app.viewMode === "grid"}
    <FolderGrid />
  {:else}
    <ListView />
  {/if}
</aside>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    width: 300px;
    min-width: 200px;
    max-width: 50%;
    background: #121212;
    border-right: 1px solid #26282d;
    flex: none;
  }

  .side-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 10px;
    border-bottom: 1px solid #26282d;
  }

  .count {
    flex: 1;
    min-width: 0;
    font-size: 12px;
    color: #8a8f98;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .seg {
    display: flex;
    background: #202227;
    border: 1px solid #34373d;
    border-radius: 8px;
    overflow: hidden;
    flex: none;
  }
  .seg-btn {
    border: none;
    background: transparent;
    color: #8a8f98;
    padding: 6px 10px;
    font-size: 12px;
    cursor: pointer;
    white-space: nowrap;
  }
  .seg-btn:hover {
    color: #d8dae0;
  }
  .seg-btn.on {
    background: #3d6ef7;
    color: #fff;
  }

  .collapse {
    border: 1px solid #34373d;
    background: #23252b;
    color: #c9cdd4;
    border-radius: 7px;
    padding: 5px 10px;
    font-size: 13px;
    cursor: pointer;
    flex: none;
  }
  .collapse:hover {
    background: #2c2f36;
  }
</style>
