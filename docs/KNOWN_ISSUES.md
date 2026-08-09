# OPPO Live Viewer — 已知问题与踩坑记录

> 开发过程中遇到并解决的问题，含根因与教训，供后续新工程（尤其 Electron HDR 版）复用。

## 1. WebView2 无法输出真 HDR（未解决，需换壳）

- **现象**：Ultra HDR 图片在 App 里只显示 SDR 基础图；同一张图拖进 Chrome 浏览器高光明显更亮（真 HDR）。
- **诊断**：F12 面板显示 `dynamic-range: high = 是`、`color-gamut p3 = 是`——WebView2 **能识别 HDR 屏**，但显示层面 HDR 不生效。
- **根因**：嵌入式 webview 的合成层不声明 HDR 内容支持（Chromium 的 `drawsHDRContent`）。Chromium 对 gain map 图片"仅在合成层支持 HDR 时按 HDR 解码"，否则只解码基础 SDR 图。这是 WebView2 的平台能力边界（Chromium 源码有 "Webview does not have proper HDR support" 的明确提交；WebKit 同样限制）。
- **已试无效**：① 直接用 `viewer://` URL 当 `<img>` 源；② blob URL；③ canvas(display-p3) 渲染路径（把 `<img>` 画进 P3 宽色域 canvas）。均无 HDR。
- **结论/路径**：真 HDR 需自带完整 Chromium 的壳（**Electron**）。前端代码可复用，仅桌面壳与后端协议层需改写。
- **教训**：`dynamic-range: high` 媒体查询 ≠ 合成层能输出 HDR；嵌入式 webview 的 HDR 输出普遍受限，做 HDR 应用应优先考虑 Electron 这类自带 Chromium 的方案。

## 2. Live 视频"没播放"——其实是 Svelte 字符串模板 bug 导致元素零尺寸

- **现象**：视频数据在解码播放（诊断 currentTime 在走、readyState=4），但画面完全不可见；点重播/循环无反应。
- **根因一（不可见）**：`style={vl ? "width:{vl.boxW}px..." : ""}` —— `{vl.boxW}` 写在 JS 字符串里**不会被求值**，产生字面量 CSS `width:{vl.boxW}px`（无效）→ 视频盒/视频元素零尺寸。
  - **正确写法**：`` `width:${vl.boxW}px` ``（模板字符串 `$ {}`）。
- **根因二（重播失效）**：视频播完后 `videoEnded=true` 把 `<video>` 从 DOM 卸载 → `videoEl` 变 null → `replay()` 的 `if (!videoEl) return` 直接返回。
  - **解决**：播完只 `visibility:hidden`，保持元素挂载。
- **教训**：Svelte 里 JS 字符串中的 `{}` 不会插值，必须用模板字符串；`bind:this` 引用的元素卸载后为 null。

## 3. HEVC 解码能力判断（canPlayType codec 字符串）

- **现象**：`video.canPlayType('video/mp4; codecs="hvc1"')` 返回空串，误判为不支持 HEVC。
- **根因**：裸 `hvc1` 不是合法 RFC6381 codec 串；须用完整形式如 `hvc1.1.6.L93.B0`（返回 `probably`）。
- **教训**：判断 HEVC 支持要用完整 codec 串；且实际能否播放以 `video.onerror` / metadata 加载为准，canPlayType 仅供参考。

## 4. 图片 404 竞态（viewer:// 直接加载 vs 缓存预热）

- **现象**：间歇性"图片加载失败"；重新加载/切走再切回就好了。
- **根因**：`<img src="viewer://...">` 一渲染就发请求，但 Rust 字节缓存还没被 `load_photo` 预热（异步）→ 协议处理器 404。
- **解决**：改为**先 `load_photo` 预热 + fetch 生成 blob objectURL，再渲染 `<img>`**（无竞态）。
- **教训**：自定义协议 + 异步预热时，`<img>`/`<video>` 直接引用协议 URL 会竞态；用"先取字节→blob"模式。

## 5. 跳远图时视频偶发不播（LRU 淘汰竞态）

- **现象**：从列表跳到较远的 Live 图，有时视频不播；再选中一次就好了。
- **根因**：`prefetchNearby` 并发加载 ±1/±2 邻居，把 Rust 字节缓存（LRU 上限小）挤满，**淘汰了当前图的字节** → 取 mp4 时 404。
- **解决**：① 扩大缓存上限（8→24）；② `loadAndFetch` 取 jpeg/mp4 若 404 自动重试（重新 load_photo 预热再取）；③ `ensureMp4Url` 并发去重。
- **验证**：mock 模拟 LRU 淘汰（cap=4）+ 快速连跳 30 次，30/30 正常。
- **教训**：异步预取 + 有限缓存必有淘汰竞态，用"取不到自动重试（自愈）"兜底。

## 6. 大文件夹打开卡死（全量扫描 + 主线程阻塞）

- **现象**：打开数千张图的文件夹整个 hang 住。
- **根因**：`scan_folder` 同步命令在主线程跑，且每张图读 ≤8MB 搜 ftyp → 几千张几十 GB I/O + UI 冻结。
- **解决**：① 扫描只读首 **128KB**（XMP/EXIF 检测，is_live 靠文件名/MotionPhoto）；② 命令改 **async + spawn_blocking**；③ 改**流式**：`start_scan` 立即返回总数，后台逐批推 `scan-batch`，前端边收边追加。
- **教训**：文件扫描类命令绝不能同步阻塞主线程；大目录用"秒出 + 流式追加 + 进度"。

## 7. 自定义协议 origin 的平台差异

- Windows（WebView2）：`http://viewer.localhost/load/...`；macOS/Linux：`viewer://localhost/load/...`。
- 前端由 `navigator.userAgent` 判断 origin；CSP 需放行两种形式。
- **教训**：自定义协议 URL 的 origin 各平台不同，别硬编码。

## 8. GitHub Actions 里 PowerShell 不认 bash 子串

- **现象**：Windows 的 Artifact 压缩包名缺 commit 后缀（`installer-windows-latest-`），mac/ubuntu 正常。
- **根因**：`echo "short=${GITHUB_SHA::7}"` 是 bash 子串语法，Windows runner 默认 PowerShell 取不到值。
- **解决**：该步骤显式 `shell: bash`（Windows runner 自带 Git Bash）。
- **教训**：跨平台 workflow 的 shell 写法要显式指定 shell。

## 9. Linux 无头/VMware 环境限制（测试方法）

- 真实 GUI 无法在 VMware 无 3D 环境可靠显示/抓屏（WebKitGTK 窗口不映射到 X 合成输出）。
- **验证方法**：构建前端 + 无头 Chrome + 注入 mock `window.__TAURI_INTERNALS__`（重写 `viewer://` fetch、
  模拟流式扫描事件、模拟 404/淘汰），用 CDP 驱动做端到端断言（渲染像素、缩放、搜索、视频元素、console 零错误）。
- Rust 侧用单元测试（真实 `.live.jpeg` 解析）验证解析逻辑。
- **教训**：headless 环境验证前端逻辑可行；平台特有能力（HDR、HEVC 解码、真实窗口）仍须用户在真机确认。

## 10. HEVC 视频依赖平台解码器

- WebView2 播放 HEVC 依赖 Windows 的 HEVC 视频扩展（大多数 Win11 自带）；解码失败时 App 显示
  "Live 视频无法播放（可能缺少 HEVC 解码器）"提示，不静默失败。
- macOS WKWebView 原生支持 HEVC。
- **教训**：H.265 在嵌入式 webview 的解码可用性因平台/编解码器而异，需优雅降级 + 明确提示。

## 11. 视频 autoplay 被浏览器策略拦截

- **现象/风险**：自动播放可能被拦（尤其 muted 属性设置时机不对）。
- **解决**：`play()` 前显式 `videoEl.muted = muted`；被拦时**不隐藏视频**，显示"▶ 点击播放"提示（用户点击即播放）。
- **教训**：不要因 autoplay 被拦就隐藏视频元素，保留可点击恢复的入口。
