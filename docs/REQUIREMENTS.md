# OPPO Live Viewer — 原始需求文档

> 版本：2026-08-09 · 需求来源：用户对旧版（PyQt6）查看器的体验反馈

## 背景

旧版 OPPO Live Photo 查看器（`~/.openclaw/workspace/oppo-live-viewer`，PyQt6 + Pillow）存在三个让用户放弃使用的硬伤，需要重写。

## 核心需求（三个痛点）

### 1. 界面美观
- 旧版 UI 采用硬编码 stylesheet + Fusion 风格，过于简陋。
- **要求**：现代化、美观的界面，用户倾向于使用 Tauri 等 Web 前端框架。
- **性能约束**：UI 美观的同时性能不能下降太多，尤其是鼠标滚轮快速翻看照片时必须流畅。

### 2. 支持 Android Ultra HDR 标准
- 用户的测试图片为 Android Ultra HDR（`.live.jpeg`，3456×4608，含 `hdrgm` XMP gain map、MPF、ICC profile）。
- Chrome 浏览器已支持该标准，在 HDR 屏幕上正确显示 HDR 效果。
- **要求**：查看器支持 Ultra HDR 图片。

### 3. 图像渲染质量（第一优先级）
- 用户明确：**渲染质量是三个需求中最重要的一项**。
- 主流图片浏览器（不支持 Live Photo）的渲染质量、缩放算法都明显优于旧版。
- 旧版"几乎不能看"的质量问题导致用户完全没有使用欲望。
- **要求**：色彩管理正确、缩放高质量、100% 放大时像素级锐利。

## 用户确认的决策

| 决策点 | 结论 |
|--------|------|
| 目标平台 | **Windows + macOS 为主**（用户有 HDR 屏幕），Linux 仅用于开发 |
| 框架 | **Tauri v2** |
| 滚轮行为 | **浏览上一张/下一张**（快速翻看）；缩放用双击 / Ctrl+滚轮 / 触控板捏合 |
| HDR 渲染逻辑 | **SDR 屏幕只显示基础 SDR 图，不应用 gain map**（与 Chrome 行为一致）；gain map 仅在 HDR 屏幕参与渲染 |
| HDR 优先级 | 高质量 SDR 渲染优先，HDR 作为锦上添花 |
| macOS 架构 | 用户仅有 **Apple Silicon (ARM)** Mac，无需构建 x86_64 macOS 安装包 |

## 验收标准

1. 界面现代化、暗色主题，滚轮快速翻看流畅无卡顿。
2. `.live.jpeg` 在 SDR 屏幕渲染色彩正确（对照系统看图器），Ultra HDR 徽标正确显示。
3. 100% 缩放像素级锐利，缩放/平移流畅。
4. Live Photo 视频自动播放，缺失 HEVC 解码时优雅降级为静态图。
5. 打包产物：Windows `.msi`/`.exe`、macOS `.dmg`（aarch64）、Linux `.deb`/`.AppImage`。

## 测试素材

- `20260227-144315.live.jpeg`（Ultra HDR + 内嵌 HEVC MP4，3456×4608）
- 普通 `.jpg` / `.png` 图片若干

## 使用方式

- 打开文件夹 → 网格浏览 → 点击进入单图查看。
- 拖拽文件夹或图片文件直接打开。
- 状态栏显示索引、文件名、尺寸、文件大小、拍摄时间、HDR / Live Photo 标记。
