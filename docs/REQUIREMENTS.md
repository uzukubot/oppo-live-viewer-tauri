# OPPO Live Viewer — 产品需求文档

> 版本：2026-08-09（v2） · 对应技术文档见 [DESIGN.md](./DESIGN.md) · 已知问题见 [KNOWN_ISSUES.md](./KNOWN_ISSUES.md)

## 1. 背景

旧版 OPPO Live Photo 查看器（`~/.openclaw/workspace/oppo-live-viewer`，PyQt6 + Pillow）存在三个让用户放弃使用的硬伤，需要重写。

## 2. 核心需求（原始痛点，全部已解决）

| # | 痛点 | 现状 |
|---|------|------|
| 1 | 界面丑（硬编码 stylesheet + Fusion） | ✅ 已用 Tauri v2 + Svelte5 重写，现代暗色 UI |
| 2 | 渲染质量差（Pillow 单次 BICUBIC、无缓存） | ✅ 改用浏览器原生解码 + `<img>` 高质量渲染 |
| 3 | 无 HDR / 无缩放 / 无 Live 播放 | ⚠️ 见下方"HDR 状态"，其余已实现 |

**渲染质量是最高优先级**：100% 缩放像素级锐利、色彩管理正确、滚轮快速翻看不卡。

## 3. 用户确认的交互决策

| 决策点 | 结论 |
|--------|------|
| 目标平台 | Windows + macOS 为主（有 HDR 屏），Linux 仅开发 |
| 桌面框架 | Tauri v2 + Svelte5 |
| **布局** | 打开文件夹后**侧边栏文件列表 + 主区查看器同屏**；侧边栏可隐藏、可拖拽调宽 |
| 滚轮行为 | 浏览上一张/下一张；缩放用双击 / Ctrl+滚轮 / 触控板捏合 |
| **文件夹加载** | **流式懒加载**：秒出第一批文件名，边扫边追加，底部显示"已加载 n/total" |
| 列表默认视图 | 文件名列表（省资源）；缩略图网格可切换 |
| 文件名搜索 | 侧边栏支持按文件名子串过滤 |
| Live 视频 | 进入时**自动播一次后回封面**；控制条：重播 / 循环 / 静音 |
| HDR 渲染逻辑 | SDR 屏只显示基础 SDR 图，不应用 gain map（与 Chrome 一致） |
| macOS 架构 | 仅 Apple Silicon (aarch64) |

## 4. HDR 状态（重要结论，2026-08-09 确认）

**结论：Tauri 的 WebView2 无法输出真 HDR**。

- 诊断确认：WebView2 `dynamic-range: high` 返回 `是`、`color-gamut p3` 返回 `是`——**能识别 HDR 屏**，但显示层面 HDR 不生效（gain map 未应用，输出仍为 SDR）。
- 与 Chrome 浏览器（同 Chromium 151 引擎）对比：Chrome 高光明显更亮（真 HDR），App 里是 SDR 基础图。
- 根因：嵌入式 webview 的合成层（compositor）不声明 HDR 内容支持（`drawsHDRContent`），Chromium 据此只按 SDR 解码 gain map 图片。这是 WebView2 的平台能力边界，**非代码 bug**。
- 已试：直接 `viewer://` URL、blob URL、canvas(display-p3) 渲染路径——均无效。
- **HDR 真支持需要新工程**（自带 Chromium 的壳，如 Electron），前端代码可复用。

### 当前 HDR 相关能力
- ✅ 检测 Ultra HDR（`hdrgm` / ISO 21496-1），显示 HDR 徽标与状态栏标记
- ✅ SDR 屏幕正确显示基础图（与 Chrome 行为一致）
- ❌ HDR 屏幕上的真 HDR 输出（受 WebView2 限制，待新工程）

## 5. 功能清单（已实现）

- 打开文件夹 / 拖拽文件或文件夹 / 记住上次文件夹
- 侧边栏：文件名列表（虚拟滚动、懒加载）+ 缩略图网格，可切换、可隐藏、可调宽
- 文件名搜索（大小写不敏感过滤）
- 查看器：滚轮翻看、双击/Ctrl+滚轮缩放、拖拽平移、100% 像素级
- Live Photo：自动播一次→封面，重播/循环/静音控制，视频旋转角
- 状态栏：索引、文件名、尺寸、大小、日期、Ultra HDR / Live Photo 标记
- 诊断面板（F12）：平台、HDR 屏检测、HEVC 支持、视频状态、commit 版本
- 版本追踪：Artifact 压缩包名 + App 内 F12 均显示短 commit ID
- 打包：Windows (.msi/.exe + 绿色版 exe)、macOS (.dmg aarch64)、Linux (.deb/.AppImage)

## 6. 验收标准

1. 打开大文件夹（数千张）秒出列表，滚动流畅，无全屏"扫描中"阻塞。
2. 滚轮快速翻看无卡顿；100% 缩放像素级锐利。
3. Live Photo 视频自动播放一次后回封面，重播/循环/静音可用。
4. Ultra HDR 图片在 SDR 屏显示基础图 + 徽标；真 HDR 输出待新工程。
5. F12 诊断面板版本号与 Artifact 压缩包名一致（判断构建版本）。

## 7. 测试素材

- `20260227-144315.live.jpeg`（Ultra HDR + 内嵌 HEVC MP4，3456×4608）
- 用户 Windows 实机上的各类 OPPO `.live.jpeg`（Ultra HDR，GainMapMax 0.8~1.5）
