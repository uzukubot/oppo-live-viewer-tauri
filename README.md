# OPPO Live Viewer

跨平台 OPPO Live Photo / Android Ultra HDR 查看器（Tauri v2 + Svelte 5）。

- 🖼️ 高质量渲染：浏览器引擎解码 + ICC 色彩管理，100% 缩放像素级锐利
- 📸 Android Ultra HDR：HDR 屏幕自动显示 HDR，SDR 屏幕显示基础图（与 Chrome 一致）
- 🎬 Live Photo 动态播放（HEVC，缺失时优雅降级）
- ⚡ 滚轮快速翻看 + 相邻预取
- 🖱️ 拖拽打开文件夹/图片，记住上次文件夹

## 开发

```bash
npm install
npm run tauri dev
```

## 打包

```bash
npm run tauri build
```

GitHub Actions 会自动构建 Windows / macOS (aarch64) / Linux 三平台安装包，
产物见 Actions Artifacts 与 release。

## 文档

- [原始需求](docs/REQUIREMENTS.md)
- [设计文档](docs/DESIGN.md)

## 技术栈

- Tauri v2（Rust 后端：容器解析 / `viewer://` 协议 / 元数据）
- Svelte 5 + Vite（前端渲染：canvas + createImageBitmap）
