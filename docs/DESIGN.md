# OPPO Live Viewer — 设计文档

> 版本：2026-08-09 · 对应实现见 [REQUIREMENTS.md](./REQUIREMENTS.md)

## 1. 目标

用 Tauri v2 + Svelte 5 重写跨平台 OPPO Live Photo / Ultra HDR 查看器，解决旧版三个痛点：界面丑、渲染质量差、无 HDR 支持。**渲染质量第一优先**。

## 2. 总体架构：薄 Rust 后端 + 厚 Web 前端

所有像素级工作（JPEG 解码、ICC 色彩管理、缩放、Ultra HDR 渲染）交给浏览器引擎
（Chromium / WebKit / WebKitGTK），质量由引擎保证，远优于旧版 Pillow 单次 BICUBIC。

```
┌──────────────────────────── Tauri WebView ────────────────────────────┐
│  Svelte 前端                                                           │
│  · FolderGrid 虚拟化网格（缩略图 + LIVE/HDR 徽标）                       │
│  · Viewer 单图查看（canvas + createImageBitmap 高质量渲染）              │
│  · Live Photo <video> 播放（blob URL）                                  │
│  · ZoomCanvas：fit 用预下采样位图 / 100% 用原图 1:1 像素                 │
│  · photoCache：blob/预览/原图/缩略图 LRU 缓存 + 相邻预取                  │
└───────────────┬────────────────────────────────────────────────────────┘
                │ fetch viewer://load/{id}/{jpeg|mp4}
                ▼
┌──────────────────────────── Rust 后端 ────────────────────────────────┐
│  parser.rs   · .live.jpeg 容器切分（搜索 ftypmp42 → JPEG/gain map/MP4）  │
│              · EXIF 方向/时间、Ultra HDR 检测（hdrgm/ISO 21496-1）、     │
│                视频旋转角（tkhd matrix）                                │
│  store.rs    · id→磁盘路径注册表 + 字节缓存（LRU，前端只拿不透明 id）     │
│  commands    · scan_folder / open_path（目录或单文件）/ load_photo       │
│  协议         · register_uri_scheme_protocol("viewer") 提供字节          │
└────────────────────────────────────────────────────────────────────────┘
```

**关键原则**：
- 像素工作不在 Rust 做，避免重复造轮子且保证全平台一致的解码/色彩质量。
- 文件路径只存在于 Rust 侧，URL 只含不透明 id，不泄露文件系统路径。
- 打开单图才全量读取（`load_photo` → 切分 → 缓存字节）；扫描文件夹只读
  文件头 + 首 ≤8MB（XMP/EXIF/ftyp 标记），避免大目录全量读盘。

## 3. Rust 模块

### parser.rs
- `PhotoMeta`：id/path/name/width/height/orientation/is_live/mp4_offset/
  video_rotation/size/date/ultra_hdr
- 切分：搜索 `b"ftypmp42"`（回退 `b"ftypisom"`），`mp4_offset = ftyp_pos - 4`；
  **JPEG 部分 = bytes[0..mp4_offset]**（含 gain map，交给浏览器）、
  **MP4 部分 = bytes[mp4_offset..EOF]**；JPEG 截到最后一个 `ffd9`（EOI）。
- Ultra HDR 检测：扫描缓冲区内 `hdrgm`（XMP）与 `GainMapVersion`（ISO 21496-1），
  提取 `GainMapMin/Max/Gamma`（仅用于徽标与信息展示，不参与 SDR 渲染）。
- 视频旋转：解析 `tkhd` matrix → 0/90/180/270。

### 自定义 URI scheme `viewer://`
- 注册于 Builder；URL 形如 `/load/{id}/{jpeg|mp4}`，查 store 缓存返回字节。
- 平台差异：Windows 用 `http://viewer.localhost`，macOS/Linux 用
  `viewer://localhost`（前端由 UA 判断）。
- CSP 需放行 `viewer:` / `http://viewer.localhost`（img/media/connect-src）。

## 4. 前端

### 渲染（质量核心）
- **fit**：`createImageBitmap(blob, { resizeWidth/Height, imageOrientation:'from-image' })`
  解码时高质量下采样（设备像素精度），避免浏览器 drawImage 缩放损失。
- **100%**：解码原图 `createImageBitmap(blob)`，`imageSmoothingEnabled=false`
  1:1 物理像素绘制。
- 浏览器引擎处理 ICC 色彩管理与 EXIF 方向。

### 交互
- 滚轮 = 上一张/下一张（主交互）；Ctrl/Shift+滚轮、双击、`+/-` = 缩放；
  放大后拖拽平移；`Esc` 返回网格；方向键/空格浏览；`Home/End` 首末张。

### Live Photo 播放
- MP4 通过 `viewer://` fetch → `URL.createObjectURL` → `<video autoplay loop muted>`。
- `video_error` → 隐藏视频，保留静态图（HEVC 缺失优雅降级）。
- 支持视频旋转角（tkhd matrix），`<video>` 用 CSS transform 旋转。
- 点击视频切换静音。

### 缓存与性能
- `photoCache`：按 id 缓存 blob / fit 预览位图 / 原图位图 / 缩略图，LRU + 内存预算。
- 查看器 `$effect` 中预取相邻 ±1、±2 张的 blob，实现快速翻看。

## 5. Ultra HDR 策略

| 平台 | WebView | HDR 渲染 |
|------|---------|---------|
| Windows | WebView2 (Chromium) | 原生支持 Ultra HDR ✅ |
| macOS | WKWebView (Safari) | ⚠️ WebKit gain map 支持未落地，预期先 SDR + 徽标 |
| Linux | WebKitGTK | 仅 SDR（开发用） |

- 把完整 "JPEG 部分"（主图 + gain map + XMP/MPF）交给浏览器，
  由引擎决定 SDR/HDR——**与 Chrome 行为一致**。
- Rust 仅检测 gain map 显示徽标；SDR 屏幕不应用 gain map（符合标准理念）。

## 6. GitHub Actions 打包

- `.github/workflows/build.yml`：`tauri-action@v0` 构建
  Windows / macOS (aarch64) / Ubuntu 三平台，上传安装包到 draft release
  与 Actions Artifacts。
- 触发：push main、`v*` tag、手动 dispatch。
- pre-commit（gitleaks 密钥检测 + 通用检查 + rustfmt）。

## 7. 里程碑与验证

M0 脚手架 → M1 Rust 解析/扫描 → M2 渲染（协议+ZoomCanvas）→ M3 视频 →
M4 快速翻看 → M5 网格 → M6 打磨/打包。

验证（Linux 无头环境 GUI 抓屏不可用）：
- Rust 单元测试：真实 `.live.jpeg` 检测 Ultra HDR / 切分 JPEG+MP4。
- 无头 Chrome + mock `window.__TAURI_INTERNALS__`（重写 `viewer://` fetch）
  端到端：渲染像素、缩放 100%、网格缩略图、视频降级、console 零错误。

## 8. 关键文件

```
src-tauri/src/parser.rs    容器解析/元数据/Ultra HDR 检测
src-tauri/src/store.rs     id 注册表 + LRU 字节缓存
src-tauri/src/lib.rs       Builder/协议/commands
src/lib/components/        TopBar/FolderGrid/ThumbCell/Viewer/ZoomCanvas/StatusBar/Welcome
src/lib/viewer/photoCache.ts  blob/位图缓存 + 预取
src/lib/protocol.ts        viewer:// 访问 + 高质量解码
src/lib/state.svelte.ts    全局状态（Svelte 5 runes）
```
