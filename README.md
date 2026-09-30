# SMC Downloader

Tauri 2 + React 桌面下载工具，支持断点续传，配合 SMC Packaging Runner 使用。

## 功能

- 粘贴下载链接 + download token，开始下载
- 断点续传（HTTP Range 请求，`.part` 文件续写）
- 暂停 / 继续 / 取消
- 下载进度条（百分比 + 速度 + 已下载 / 总大小）
- 下载完成后 `.part` → 正式文件名

## 前提条件

### 通用

- [Rust](https://rustup.rs/) (stable)
- Node.js 18+（[nvm](https://github.com/nvm-sh/nvm) 推荐）
- Tauri 2 CLI（随 npm 依赖安装）

### macOS 额外要求

- Xcode Command Line Tools: `xcode-select --install`

### Windows 额外要求

- [Microsoft Visual C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)（含 C++ 桌面开发工作负载）
- [WebView2](https://developer.microsoft.com/microsoft-edge/webview2/)（Windows 11 预装，Windows 10 需手动安装）

## 开发

```bash
cd smc-downloader
npm install
npm run tauri dev
```

## 打包

详细的 macOS 和 Windows 打包步骤见 [BUILD.md](./BUILD.md)。

快速构建：

```bash
npm install
npm run tauri build          # 当前平台打包
```

- macOS 产物：`src-tauri/target/release/bundle/dmg/*.dmg`
- Windows 产物：`src-tauri/target/release/bundle/msi/*.msi`、`nsis/*.exe`
- Windows 也可通过 GitHub Actions 自动构建：`git tag v0.1.0 && git push origin v0.1.0`

> 完整的环境安装、签名公证、通用二进制、常见问题等见 [BUILD.md](./BUILD.md)。

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
  GET 请求获取 Content-Length
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
