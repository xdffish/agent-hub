# Agent Hub 开发守则与协作规范 (AGENTS.md)

本文件是所有 AI 编码代理（Antigravity, Claude Code, Cursor, Codex 等）在本项目中必须无条件遵守的最高级别行为规范。

---

## 1. 核心架构与技术栈约束

- **桌面运行时**：Tauri v2（**严格禁止**使用任何 Tauri v1 弃用 API 或配置结构）。
- **后端架构**：Rust 2021 + Tokio 异步运行时，遵循安全沙箱与最小权限原则。
- **前端架构**：React 19 + TypeScript (Strict) + Vite 6 + Tailwind / Vanilla CSS。
- **IPC 通信**：基于 `tauri-specta` 生成强类型前后端绑定（`src/bindings.ts`），禁止使用无类型的字符串传参。

---

## 2. 核心行为与产品铁律 (Product Laws)

所有 Agent 编写代码时必须严格遵守 `docs/product-spec.md` 中的设计决策：
1. **破坏性隔离与只读优先**：对外部 Agent 配置文件的写入操作，必须遵循“先生成差异预览（Diff）并自动快照备份，经确认后再应用”，严禁静默覆盖。
2. **密钥绝对隔离**：凭据仅存入 macOS 钥匙串，界面默认遮蔽，严禁导出明文。
3. **多端状态区分**：不得混淆“已配置 / 已安装 / 已加载 / 运行可用”四种状态。

---

## 3. Skills 路由规范（开发时必查）

本项目已在 `.agents/skills/` 内置了完整的 Tauri v2 技能包。当涉及以下领域开发时，**必须在编写代码前主动检索并遵照对应 Skill 的规范**：
- 涉及权限控制（Capabilities）-> 查阅 `.agents/skills/tauri-security`
- 涉及前后端通信与命令 -> 查阅 `.agents/skills/tauri-ipc`
- 涉及窗口与菜单托盘 -> 查阅 `.agents/skills/tauri-window` & `tauri-app-system-tray`
- 涉及配置变更与持久化 -> 查阅 `.agents/skills/tauri-config` & `tauri-app-store`

---

## 4. 关键命令字典 (Cheat Sheet)

- **类型绑定导出**：`npm run bindings`（修改 Rust command 后必先执行此命令生成前端类型）
- **本地启动联调**：`npm run tauri dev`
- **代码格式化**：`npm run format`
- **版本发布与打包**：`npm run release`（严禁手动修改版本号，必须使用该交互式脚本统一同步并打 Git Tag）

---

## 5. 版本控制规范 (Git & Releases)

- 日常功能/缺陷提交严禁修改 `version` 字段。
- 发布时遵循语义化版本规范（SemVer），通过 `npm run release` 自动完成三处版本号同步，并打上形如 `vX.Y.Z` 的 Git Tag。
