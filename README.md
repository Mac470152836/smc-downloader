# SMC Downloader

Tauri 2 + React 桌面下载工具，支持断点续传，配合 SMC Packaging Runner 使用。

## 功能

- 粘贴下载链接 + download token，开始下载
- 断点续传（HTTP Range 请求，`.part` 文件续写）
- 暂停 / 继续 / 取消
- 下载进度条（百分比 + 速度 + 已下载 / 总大小）
- 下载完成后 `.part` → 正式文件名

## 前提条件

- [Rust](https://rustup.rs/) (stable)
- Node.js 18+ ([nvm](https://github.com/nvm-sh/nvm) 推荐)
- Tauri 2 CLI: `npm install -D @tauri-apps/cli`

## 开发

```bash
cd smc-downloader
npm install
npm run tauri dev
```

## 构建

```bash
npm run tauri build
```

产物在 `src-tauri/target/release/bundle/`：
- macOS: `.dmg` / `.app`
- Windows: `.msi` / `.exe`

## 使用

1. 在 SMC Packaging Runner 对话中调用 `compress_and_get_url`，获得 `download_url` 和 `download_token`
2. 打开 SMC Downloader，粘贴 URL 和 token
3. 选择保存目录（默认 ~/Downloads）
4. 点击「开始下载」
5. 可暂停 / 继续 / 取消
6. 下载完成后文件出现在保存目录

## 断点续传原理

```
首次下载:
  HEAD 请求获取 Content-Length
  → 创建 .meta.json (记录 url / token / filename / total_size)
  → 创建 .part 文件，流式写入

暂停 / 断线:
  保留 .part + .meta.json

恢复下载:
  读取 .meta.json，检查 .part 文件大小
  → GET Range: bytes=<已下载>-
  → 206 Partial Content，追加写入 .part
  → 完成后 rename .part → 正式文件名，删除 .meta.json
```

## 技术栈

- Tauri 2 (Rust 后端)
- React 18 (TypeScript 前端)
- reqwest (HTTP Range 下载)
- Vite (前端构建)
