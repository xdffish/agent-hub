# Agent Hub

面向同时使用多个 Coding Agent 的个人开发者，在 macOS 集中盘点和管理 Skills、MCP 配置及插件。

Tauri 2 + React + TypeScript + Rust 构建的本机桌面工具。第一版已实现扫描、共享 Skill 库、变更预览、备份撤销、基础 MCP 配置与连接检查，以及插件清单识别。

## 运行

需要 Node.js、Rust 和 macOS Xcode Command Line Tools。

```bash
npm ci
npm run tauri dev
```

构建本机应用：

```bash
npm run tauri build
```

开发构建可使用 `npm run tauri build -- --debug`，产物位于 `src-tauri/target/debug/bundle/macos/Agent Hub.app`。本地构建未配置 Apple Developer 签名、公证和分发更新。

## 当前能力

- 只读盘点 Claude Code、Codex、Cursor、Antigravity 的已知全局位置及主动添加的项目。
- 从本地目录或 HTTPS Git 仓库导入 Skill；保留来源、版本，支持固定版本、检查更新与按目标分配。
- 所有资源文件写入先预览，再应用；重新核对目标快照、保存私有备份，支持撤销与失败恢复预览。
- 项目、来源和备份文件通过原生选择器授权；应用变更、Git 拉取和 MCP 检查还需原生确认，前端不能单凭绝对路径取得权限。
- 无凭据 MCP 基础配置按各 Agent 格式写入；保留同名项中的原生凭据与扩展字段。按需执行 stdio / HTTP 初始化检查。
- 插件目录发现与官方原生管理文档入口；发现 manifest 不表示插件已安装或启用。
- 导出、导入 Skill 共享库和分配参考，导入后核对本机路径再分配；不导出 MCP 配置或 OAuth 凭据。

应用数据位于 `~/Library/Application Support/dev.agenthub.desktop`，目录权限为 0700，备份文件为 0600。配置备份可能含原有秘密；Skill 内容也需用户自行检查，应用只能拦截常见凭据文件名。

## 开发检查

```bash
npm run build
npm run bindings
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
```

Rust 检查使用临时目录验证快照、覆盖冲突、撤销、凭据隐藏和配置字段保留，不修改真实 Agent 配置。

- [产品说明与验收标准](docs/product-spec.md)
- [四种 Agent 的官方资料兼容性调查](docs/research/agent-compatibility.md)
- [实现范围、验证结果与限制](docs/implementation.md)
- [命令权限、路径范围与 IPC 契约](docs/security-scope.md)
- [本地 Skill 规范补齐清单](docs/hardening-plan.md)

Agent 能力随版本变化；路径发现与写入成功不能替代原生加载和连接确认。详见兼容性调查与实现记录。
