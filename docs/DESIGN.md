# OPPO Live Viewer — 技术设计文档

> 版本：2026-08-09（v2，对齐实际实现） · 需求见 [REQUIREMENTS.md](./REQUIREMENTS.md) · 已知问题见 [KNOWN_ISSUES.md](./KNOWN_ISSUES.md)

## 1. 总览

Tauri v2 + Svelte5 重写的跨平台 OPPO Live Photo / Ultra HDR 查看器。
**渲染质量第一优先**；所有像素级工作交给浏览器引擎（Chromium / WebKit / WebKitGTK）。

## 2. 架构：薄 Rust 后端 + 厚 Web 前端

```
┌────────────────────────── Tauri WebView ──────────────────────────┐
│  Svelte5 前端（双栏布局）                                           │
│  · Sidebar：搜索框 + 文件名列表(虚拟滚动) / 缩略图网格，可隐藏/调宽   │
│  · Viewer：<img> 静态图（原生解码）+ <video> Live + 缩放/平移       │
│  · 流式扫描监听（scan-batch 事件边收边追加）                         │
│  · photoCache：blob/mp4Url/缩略图 LRU + 预取 + 404 重试             │
│  · Diagnostics (F12)：HDR/HEVC/视频状态/commit 版本                  │
└───────────────┬────────────────────────────────────────────────────┘
                │ fetch viewer://load/{id}/{jpeg|mp4}
                ▼
┌────────────────────────── Rust 后端 ──────────────────────────────┐
│  parser.rs  · .live.jpeg 切分（ftypmp42 → JPEG/gain map/MP4）       │
│             · 轻量扫描(128KB) / 全量解析(full_meta)、EXIF、          │
│               Ultra HDR 检测、视频旋转角(tkhd)                       │
│  store.rs   · id→路径注册表 + 字节缓存(LRU 24) + 流式扫描状态         │
│  commands   · start_scan（立即返回总数+后台流式推送）                 │
│             · open_path（目录或单文件，算索引）                       │
│             · load_photo（全量解析+缓存，返回精确元数据）             │
│  协议       · register_uri_scheme_protocol("viewer") 提供字节        │
└────────────────────────────────────────────────────────────────────┘
```

**关键原则**：
- 像素工作不在 Rust 做——浏览器引擎的解码/ICC 色彩/缩放质量远优于自研。
- 文件路径只存在于 Rust 侧；URL 只含不透明 id，不泄露路径。
- 扫描文件夹只做**轻量解析**（首 128KB 检测 XMP/EXIF），打开单图才全量读。

## 3. 流式懒加载（大文件夹关键设计）

**痛点**：旧方案全量扫描后一次性返回，数千张图会卡死（同步命令阻塞主线程 + 每张读 8MB）。

**实现**：
1. `start_scan(folder)`：立即列出路径（纯 stat）+ 返回总数，**不阻塞**。
2. 后台线程逐张 `scan_photo`（只读首 128KB），每 25 个发 `scan-batch` 事件。
3. 前端监听事件**边收边追加** `app.photos`，列表秒出第一批，底部显示"已加载 n/total"。
4. 代际（generation）机制：切换文件夹时旧扫描线程自动停止，避免串数据。
5. 列表/网格均为**虚拟滚动**：只渲染视口内可见行。

**关键文件**：`lib.rs`（start_scan/scan_stream）、`store.rs`（ScanState）、`actions.ts`（scan-batch 监听）。

## 4. 静态图渲染（质量核心）

- 查看器用 **`<img>`** 显示静态图：Chromium 原生解码（含 ICC 色彩、EXIF 方向、mipmap 缩放），
  100% 缩放像素级锐利。
- 图片通过 `viewer://` fetch → **blob URL** 加载（先 load_photo 预热 Rust 缓存再取，避免 404 竞态）。
- fit/缩放/平移：给 `<img>` 设置 CSS 宽高/定位（无 canvas，浏览器原生渲染）。

## 5. 自定义协议 `viewer://`

- URL：`/load/{id}/{jpeg|mp4}`，查 store 缓存返回字节。
- 平台差异：Windows 用 `http://viewer.localhost`，macOS/Linux 用 `viewer://localhost`
  （前端由 UA 判断 origin）。
- CSP 放行 `viewer:` / `http://viewer.localhost`。
- 缓存被 LRU 淘汰导致 404 时，前端 `loadAndFetch` 自动重试（重新 load_photo 预热）。

## 6. Live Photo 视频

- MP4 经 `viewer://` fetch → objectURL → `<video>`。
- **默认"播一次后回封面"**；控制条：重播 / 循环 / 静音。
- 播放策略：显式设 `muted` 再 `play()`；autoplay 被拦时显示"点击播放"（不隐藏视频）。
- 视频元素**播完保持挂载**（仅隐藏），保证重播/循环可 seek。
- 支持视频旋转角（tkhd matrix）CSS 变换。
- HEVC 缺失时显示明确提示（不静默失败）。

## 7. HDR 策略与限制（2026-08-09 确认）

| 平台 | WebView | 真 HDR 输出 |
|------|---------|-----------|
| Windows | WebView2 (Chromium) | ❌ **受限**（见下） |
| macOS | WKWebView (Safari) | ❌ 预计同样受限（WebKit gain map 未落地） |
| Linux | WebKitGTK | 仅 SDR（开发用） |

- 已把完整 JPEG 部分（含 gain map）交给 `<img>`，理论上浏览器引擎应自行决定 SDR/HDR。
- 但 **WebView2 合成层不声明 HDR 内容支持**（`drawsHDRContent`），Chromium 据此只按 SDR
  解码 gain map 图片。`dynamic-range: high` 返回 `是` 不代表合成层输出 HDR。
- 已尝试：直接 URL、blob URL、canvas(display-p3) 渲染——均无效。
- **结论**：真 HDR 需自带 Chromium 的壳（如 Electron），前端代码可复用。

## 8. 缓存与性能

- Rust 字节缓存：LRU 上限 24（`CachedFile{jpeg, mp4}`）。
- 前端 `photoCache`：blob / mp4Url / 缩略图 LRU，`prefetchNearby` 预取 ±1/±2。
- 取字节 404 时自动重试（重新 load_photo 预热），自愈。

## 9. 版本追踪

- GitHub Actions：Artifact 压缩包名带 7 位短 commit（`installer-{platform}-{sha}`）。
- App 内：`vite.config.js` `define` 注入 `GITHUB_SHA` 前 7 位为 `__APP_COMMIT__`，
  `src/lib/version.ts` 导出 `APP_COMMIT`，F12 诊断面板显示 + 复制文本带出。

## 10. 前端组件

```
src/lib/components/
  TopBar.svelte      顶栏（打开文件夹）
  Sidebar.svelte    侧边栏（搜索 + 列表/缩略图切换 + 折叠）
  ListView.svelte   文件名列表（虚拟滚动 + 搜索过滤 + 流式追加）
  FolderGrid.svelte 缩略图网格（虚拟化 + 搜索过滤）
  ThumbCell.svelte  网格缩略图单元
  Viewer.svelte     <img> 静态图 + <video> + 缩放/平移 + 视频控制条
  StatusBar.svelte  状态栏（索引/尺寸/HDR 标记）
  Diagnostics.svelte F12 诊断面板（HDR/HEVC/视频/commit）
  Welcome.svelte    空态欢迎页
src/lib/
  state.svelte.ts   全局状态（Svelte5 runes）
  api.ts            命令封装（start_scan/open_path/load_photo）
  protocol.ts       viewer:// 访问 + fetchBlob
  actions.ts        打开/流式扫描/事件监听
  viewer/photoCache.ts 字节缓存 + 预取 + 重试
  version.ts        APP_COMMIT
```

## 11. GitHub Actions / 工程化

- `.github/workflows/build.yml`：Windows / macOS(aarch64) / Ubuntu，`tauri-action`，
  产物传 draft release + Actions Artifacts（名带 commit）；green 版 Windows exe。
- 触发：push main / `v*` tag / 手动；docs 变更不触发。
- pre-commit：gitleaks 密钥检测 + 通用检查 + rustfmt。
- 验证：Rust 单元测试（真实文件解析）+ 无头 Chrome + mock `__TAURI_INTERNALS__`
  端到端（渲染/缩放/搜索/视频/流式，console 零错误）。
