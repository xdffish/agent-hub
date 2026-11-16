# 应用更新与发布

Agent Hub 使用 Tauri 2 更新器读取 GitHub Release 的 `latest.json`。启动后检查版本；用户确认下载后验证安装包签名，再由用户选择安装并重启。检查失败不会阻止资源管理。

## 发布流程

1. 提交功能改动，保持工作区干净。
2. 运行 `npm run release`，选择新版本和 GitHub 发布。
3. CI 通过三平台检查后，构建安装包及签名。
4. 发布任务汇总附件，校验平台与签名齐全，生成更新清单；附件上传完成后公开 Release。

macOS 分别构建 Apple Silicon（aarch64，包含 M5）与 Intel（x86_64）安装包及签名更新包；更新器按运行架构选择对应下载地址。Windows 使用 NSIS；Linux 应用内更新使用 AppImage。DEB/RPM 用户通过安装包或包管理器升级。

第一次带更新器的版本仍需手动安装：v1.0.6 及更早版本不能接收应用内更新。更新器签名独立于 Windows 代码签名和 macOS 公证。

## 签名密钥

本机私钥存于 `~/.config/agent-hub/updater.key`，公钥存于同目录的 `updater.key.pub`，并写入 `src-tauri/tauri.conf.json`。请将私钥备份至安全位置；不能将私钥提交到 Git。更换公钥会使已安装旧公钥的客户端无法验证新签名。

GitHub 仓库需要 Actions Secret `TAURI_SIGNING_PRIVATE_KEY`。私钥设置了密码时，另配置 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`。当前密钥没有密码，通过本机文件权限和 GitHub Secrets 保护。

本地打包需在终端提供签名密钥路径：

```bash
export TAURI_SIGNING_PRIVATE_KEY="$HOME/.config/agent-hub/updater.key"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
npm run tauri build
```

开发构建及 CI 的 `--no-bundle` 检查不需要私钥。

## 发布恢复

如果构建成功但发布失败，可在 GitHub Actions 手动运行 Desktop packages，填写同一版本 tag，并在 `artifacts_run_id` 中填写该版本构建运行 ID。只能复用同一 tag、同一公钥生成的完整签名附件。流程会重新生成更新清单并上传，失败时保持 Release 草稿。

签名、缺失附件或重复平台检查失败时应修复产物再发布，不能绕过签名检查。

## 更新源

更新源为 `https://github.com/xdffish/agent-hub/releases/latest/download/latest.json`。网络需能访问 GitHub 及其下载服务。更新说明使用 Release 正文；维护者可以在发布前编辑草稿正文。若网络或安装权限受限，用户可从 Releases 手动下载安装。
