# SMC Downloader 打包指南

本文档详细说明如何在 macOS 和 Windows 上从零搭建环境并打包 SMC Downloader 安装包。

---

## 一、通用环境准备

### 1.1 Rust 工具链

#### macOS

```bash
# 方式一：官方安装脚本（推荐）
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 方式二：手动下载（网络不好时）
# 访问 https://forge.rust-lang.org/infra/other-installation-methods.html
# 下载 aarch64-apple-darwin（M 芯片）或 x86_64-apple-darwin（Intel）的 rust-*.tar.gz
# 解压后执行：
#   ./install.sh
```

安装完成后验证：

```bash
source "$HOME/.cargo/env"
rustc --version    # 应显示 1.98.1 或更高
cargo --version
```

#### Windows

1. 访问 https://rustup.rs/，下载 `rustup-init.exe`
2. 双击运行，选择默认安装（选项 1）
3. 安装完成后重启终端，验证：

```powershell
rustc --version
cargo --version
```

> Windows 还需要安装 [Microsoft Visual C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)，勾选「使用 C++ 的桌面开发」工作负载。

### 1.2 Node.js

需要 Node.js 18+（推荐 20 LTS）。

#### macOS

```bash
# 方式一：nvm（网络通畅时推荐）
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.1/install.sh | bash
source ~/.zshrc
nvm install 20
nvm use 20

# 方式二：手动下载（nvm 安装失败时）
# 访问 https://nodejs.org/en/download
# 下载 macOS arm64（M 芯片）或 x64（Intel）的 .pkg
# 双击安装即可
```

#### Windows

访问 https://nodejs.org/en/download，下载 Windows Installer（.msi），选择 LTS 版本安装即可。

验证：

```bash
node --version    # v20.x.x
npm --version     # 10.x.x
```

### 1.3 Git 镜像问题排查

如果 `git clone` / `curl` 访问 GitHub 失败，检查是否配置了失效的 git 镜像：

```bash
# 查看全局配置
git config --global --list

# 如果存在 mirror.ghproxy.com 相关配置，删除它
# 检查 ~/.gitconfig 中是否有 [url ""] insteadOf = https://github.com 这样的条目
# 如有，手动编辑 ~/.gitconfig 删除该段
```

---

## 二、macOS 打包

### 2.1 前提条件

```bash
# Xcode Command Line Tools
xcode-select --install
```

### 2.2 开发调试

```bash
cd smc-downloader
npm install
npm run tauri dev
```

开发模式下会启动 Vite 开发服务器（http://localhost:1420），Rust 后端连接前端热重载。

### 2.3 正式打包

```bash
cd smc-downloader
npm install
npm run tauri build
```

构建完成后，产物路径：

```
src-tauri/target/release/bundle/
├── dmg/
│   └── SMC Downloader_0.1.0_aarch64.dmg      # Apple Silicon 芯片
└── macos/
    └── SMC Downloader.app/
```

> Intel 芯片的 Mac 上产物文件名为 `SMC Downloader_0.1.0_x64.dmg`。

### 2.4 通用二进制（同时支持 Intel + Apple Silicon）

```bash
rustup target add x86_64-apple-darwin
rustup target add aarch64-apple-darwin
npm run tauri build -- --target universal-apple-darwin
```

产物：

```
src-tauri/target/universal-apple-darwin/release/bundle/
├── dmg/
│   └── SMC Downloader_0.1.0_universal.dmg
└── macos/
    └── SMC Downloader.app/
```

### 2.5 macOS 签名与公证（可选）

如果需要分发给其他用户且不想触发 Gatekeeper 警告，需要 Apple Developer 证书：

```bash
# 设置签名身份
export APPLE_SIGNING_IDENTITY="Developer ID Application: Your Name (XXXXXXXXXX)"

# 构建并签名
npm run tauri build -- --bundles dmg

# 公证
xcrun notarytool submit "SMC Downloader_0.1.0_aarch64.dmg" \
  --apple-id "your@email.com" \
  --team-id "XXXXXXXXXX" \
  --password "app-specific-password" \
  --wait

# 装订公证票据
xcrun stapler staple "SMC Downloader_0.1.0_aarch64.dmg"
```

> 如果没有 Apple Developer 账号，用户首次打开时需右键 -> 打开，或在「系统设置 -> 隐私与安全性」中点击「仍要打开」。

---

## 三、Windows 打包

### 方式一：GitHub Actions 自动构建（推荐，无需 Windows 机器）

项目已配置 `.github/workflows/build-windows.yml`，推送到 GitHub 后打 tag 即可自动构建。

#### 3.1 推送代码到 GitHub

如果 SSH 不通（`Permission denied (publickey)`），使用 HTTPS + Personal Access Token：

```bash
# 1. 在 GitHub 创建仓库：https://github.com/new
#    仓库名：smc-downloader，设为 Private 或 Public

# 2. 创建 Personal Access Token
#    访问 https://github.com/settings/tokens
#    点击 "Generate new token (classic)"
#    勾选 "repo" 权限
#    复制 token（形如 ghp_xxxxxxxxxxxx）

# 3. 设置远程地址（用 HTTPS）
git remote add origin https://github.com/<你的用户名>/smc-downloader.git

# 4. 推送（输入用户名和 token 作为密码）
git push -u origin main
# Username for 'https://github.com': <你的用户名>
# Password for 'https://...': ghp_xxxxxxxxxxxx
```

#### 3.2 打 tag 触发构建

```bash
git tag v0.1.0
git push origin v0.1.0
```

推送 tag 后，GitHub Actions 自动触发 Windows 构建（约 10-15 分钟）。

#### 3.3 下载产物

构建完成后有两种下载方式：

**方式 A：Artifacts（Actions 页面）**

1. 打开仓库 -> Actions 页签
2. 点击最新的 "Build Windows Installer" 运行记录
3. 在页面底部 "Artifacts" 区域下载 `smc-downloader-windows`
4. 解压后得到 `.msi` 和 `.exe` 安装包

**方式 B：Releases（自动创建）**

1. 打开仓库 -> Releases 页签
2. `v0.1.0` Release 会自动创建
3. 直接下载附件中的安装包

### 方式二：Windows 本地构建

#### 3.4 环境准备

1. 安装 [Microsoft Visual C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)，勾选「使用 C++ 的桌面开发」
2. 安装 [WebView2](https://developer.microsoft.com/microsoft-edge/webview2/)（Windows 11 预装，Windows 10 需手动安装）
3. 安装 Rust：访问 https://rustup.rs/ 下载 `rustup-init.exe`，双击默认安装
4. 安装 Node.js 20+：访问 https://nodejs.org/ 下载 LTS 版本 .msi 安装

#### 3.5 构建

```powershell
cd smc-downloader
npm install
npm run tauri build
```

产物路径：

```
src-tauri\target\release\bundle\
├── msi\
│   └── SMC Downloader_0.1.0_x64_en-US.msi
└── nsis\
    └── SMC Downloader_0.1.0_x64-setup.exe
```

- `.msi` — 适合企业部署（组策略批量推送）
- `.exe`（NSIS）— 适合用户直接双击安装

### 3.6 Windows 代码签名（可选）

未签名的安装包在 Windows SmartScreen 中会弹出警告。如果有代码签名证书：

```bash
# 在 tauri.conf.json 中配置签名
# 或使用 signtool 手动签名
signtool sign /a /t http://timestamp.digicert.com "SMC Downloader_0.1.0_x64-setup.exe"
```

> 没有证书时，用户点击「更多信息」->「仍要运行」即可安装。

---

## 四、构建配置说明

### 4.1 tauri.conf.json 关键字段

```jsonc
{
  "bundle": {
    "active": true,
    "targets": "all",        // 生成当前平台所有可用格式
    "icon": [                 // 各平台图标
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",     // macOS
      "icons/icon.ico"        // Windows
    ]
  }
}
```

如需指定格式：

| 需求 | targets 值 |
|------|-----------|
| macOS 只出 DMG | `["dmg"]` |
| Windows 只出 MSI | `["msi"]` |
| Windows 只出 NSIS exe | `["nsis"]` |
| 全部格式 | `"all"`（默认） |

### 4.2 版本号管理

修改 `tauri.conf.json` 中的 `version` 字段和 `package.json` 中的 `version` 字段，两者保持一致。

```bash
# 更新版本号示例
# tauri.conf.json:  "version": "0.2.0"
# package.json:     "version": "0.2.0"
```

### 4.3 Cargo.toml release 优化

```toml
[profile.release]
opt-level = 3        # 最高优化
lto = true           # 链接时优化，减小体积
codegen-units = 1    # 单编译单元，更好的优化
panic = "abort"      # panic 时直接终止，减小二进制体积
```

---

## 五、常见问题

### Q: npm install 后提示 node 版本不兼容？

确保 Node.js 版本 >= 18。Tauri 2 的 CLI 要求 Node 18+。使用 `node --version` 检查。

### Q: tauri dev 报 cargo metadata 找不到？

Rust 没有正确加入 PATH。执行 `source "$HOME/.cargo/env"` 或重启终端。

### Q: macOS 构建报 xcode-select 错误？

```bash
xcode-select --install
```

### Q: Windows 构建报 MSVC linker 错误？

安装 Visual C++ Build Tools，勾选「使用 C++ 的桌面开发」工作负载，重启后重试。

### Q: GitHub Actions 构建失败？

检查 `.github/workflows/build-windows.yml` 是否正确引用了 `tauri-action`。查看 Actions 运行日志定位具体错误。

### Q: git push 报 Permission denied (publickey)？

SSH key 未配置。改用 HTTPS 方式：

```bash
git remote set-url origin https://github.com/<用户名>/smc-downloader.git
git push -u origin main
# 输入 GitHub 用户名
# 密码处粘贴 Personal Access Token（不是登录密码）
```

### Q: 下载链接 403 / 500？

1. 确认 Runner 服务正在运行：`systemctl status packaging-runner`
2. 确认 token 未过期（48h 自动过期）
3. 重新调用 `compress_and_get_url` 获取新 token
4. 检查 nginx `auth_request` 配置指向正确的 Runner 地址

---

## 六、快速参考

| 操作 | macOS | Windows |
|------|-------|---------|
| 安装 Rust | `curl https://sh.rustup.rs \| sh` | 下载 rustup-init.exe |
| 安装 Node | `nvm install 20` 或官网 .pkg | 官网 .msi 安装 |
| 开发调试 | `npm run tauri dev` | `npm run tauri dev` |
| 正式打包 | `npm run tauri build` | `npm run tauri build` 或 GitHub Actions |
| 产物格式 | `.dmg` + `.app` | `.msi` + `.exe` |
| 产物路径 | `src-tauri/target/release/bundle/` | 同左 |
