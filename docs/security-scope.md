# 权限与 IPC 边界

核对日期：2026-11-10。平台：macOS。本文件依据本地 Tauri 技能、当前官方文档和代码验证，不代表 OS 沙箱或第三方审计认证。

## Capability → 命令 → 资源矩阵

所有 capability 均明确限制为本地 `main` 窗口、macOS，无远程 URL 授权。自定义命令由同一份 `src-tauri/src/registry.rs` 清单注册并生成 `allow-*` / `deny-*` 权限，不依赖默认开放的应用命令行为。

| Capability | 允许命令 | 后端资源范围与确认 |
| --- | --- | --- |
| default | scan、history、plugin_guide | 已知全局目录、已添加项目、应用私有历史；文档仅四个固定官方 HTTPS 链接 |
| paths | choose_path、add_project、remove_project | 原生选择器产生项目/来源/导入/导出四种用途授权；项目选择只准加入所选目录；取消无新增授权 |
| plans | import_skill、deploy_skill、remove_resource、delete_skill、pin_skill、check_update、preview_mcp、preview_undo、export_library、import_library | 目标由 Agent、已添加项目与资源 ID 导出；来源读取需选择授权或已发现 Skill；交换包不能指定实际共享库 folder；导出只能创建原生选择的新文件 |
| apply | apply_plan | 只接收后端预览 ID；原生确认后重新核查快照、符号链接和目标范围；私有 settings、library 或指定 Agent 配置位置 |
| mcp-check | check_mcp | 只接收已发现资源 ID 和预期配置指纹；应用内确认后由后端重新读取并核对具体配置，变化则拒绝执行；不提供任意 command 或 request IPC |
| default / plans | toggle_states / preview_toggle | 只接受已发现资源、Agent、已添加项目上下文及期望状态/原生选择器/路径；后端重新解析并校验。新增可写范围为用户与已添加项目的 `.claude/settings.json`，以及已添加项目的 `.claude/settings.local.json`；写入仍经计划预览、原生确认、备份与撤销，详见 [原生启停](native-toggles.md) |

没有向前端授予 fs、shell、http、dialog 插件的通用操作权限。原生 dialog 通过 Rust 调用，文件、网络与进程保持在自定义后端服务中，其范围必须由后端校验；安装插件不等于它的 Scope 会约束 `std::fs` / `reqwest`。

Capability 分组用于明确调用面，不是同一个 main 窗口内部各页面之间的隔离。main 可调用这些已授权命令，敏感动作进一步依赖原生用户确认与后端范围检查。

## 路径策略

- 手填绝对路径不能新增项目、导入任意备份或导出到任意位置。原生选择授权在当前会话内按用途保存，最多 100 项；项目加入后作为持久目录范围。
- 来源可读取选择的目录及子目录，解析为真实路径后检查边界；子目录不能通过 `..` 或 symlink 逃逸。已经扫描发现的 Skill 可直接纳入共享库。
- 更新本地 Skill 来源时重新原生确认读取路径。导入交换包中的 source 只是来源描述，不会自动取得读取权限。
- 共享库实际路径必须是应用私有目录的 `library/<64 位十六进制 ID>`。不使用任意外部 folder；写入与恢复计划在执行前再次校验。
- Skill 写入只限已知 Skill 根目录中的有效 Skill 目录，拒绝修改 `.system` 内置目录；MCP 写入只限各适配器的确切配置文件。项目根被替换为符号链接后跳过扫描。
- 导出使用文件系统 `create_new`，即使选择后目标被其他程序创建，也不会覆盖该文件。导出不包含 MCP / OAuth 凭据；用户仍需检查 Skill 正文和脚本里手工写入的秘密。

## 进程与网络策略

- Git 只接受不含内嵌凭据、查询参数和 fragment 的 HTTPS 仓库地址；每次拉取原生确认具体来源。固定调用 Git，无 shell 拼接，禁用仓库 hooks 与 file transport，超时 45 秒。Git 仍可使用用户既有凭据助手。
- MCP 检查固定为已发现配置中的一次初始化。使用一次应用内确认，显示所属 Agent、配置路径、作用域及执行/联网说明；不把 header、参数或 token 发给前端。后端核对配置指纹后执行，取消确认不会调用检查。结果按资源 ID 和指纹存于本次应用会话；配置改变后旧结果失效。仅当用户信任配置时执行。
- stdio 进程清空默认继承环境，仅保留 PATH、HOME、临时目录、语言、用户与 XDG 等基线变量；配置显式声明的 env 单独传入。环境引用仍可读取配置所需变量，不等于秘密管理沙箱。
- 远程 HTTP 要求 HTTPS，仅 localhost 与回环 IP 可用明文 HTTP；禁用重定向。请求方法限协议初始化/通知 POST，以及尝试关闭会话的 DELETE。不提供通用 URL 请求接口。
- 12 秒超时，stdio 子进程组在完成、超时或取消 future 后关闭；敏感异步操作的生命周期计数在完成前阻止普通关窗与退出，避免进程退出跳过清理。检查不调用业务工具，特殊凭据 helper 与 OAuth 使用原生 Agent 入口。
- 启动一个 MCP 与直接运行该程序具有相同的本机权限。本应用没有把第三方 MCP 放进 OS 沙箱，不能保证被启动程序没有自身副作用。

## CSP 与窗口

- 加载打包的本地静态资源；script-src/self，无远程脚本；connect-src 仅 Tauri IPC。
- object-src、frame-src、base-uri、form-action 明确禁止；style-src 保留 React 使用所需的 inline style。
- 不启用远程 WebView 能力。外部管理文档由后端固定白名单交给系统浏览器。

## 类型与错误契约

- 使用固定版本 `specta` / `tauri-specta` 2.0.0-rc.26 和 `specta-typescript` 0.0.13；依据可用 registry 包的真实依赖匹配，避免混用候选版本。
- `#[specta::specta]` 从 Rust 命令参数、返回值和 DTO 生成 `src/bindings.ts`。`src/types.ts` 仅导出序列化阶段类型别名，无独立手写数据结构。
- 前端调用 `commands.*`，无动态 command string 或未约束 payload。成功返回 `Promise<T>`；业务 `Err(String)` 以 rejection 交给统一 job 处理，框架权限/参数错误同样可 reject，前端 catch 按 unknown 处理。
- `npm run bindings` 更新绑定；隔离检查将重新生成结果与提交文件逐字比较，Rust 契约变化而绑定未更新会失败。

## 验证与发布边界

- 已覆盖本地 main 允许、其他窗口与远程来源拒绝的真实 Tauri Mock IPC；权限集合与注册命令完全匹配。
- 已覆盖未授权/跨用途路径拒绝、计划越界写入拒绝、共享库 metadata 重定向拒绝、TLS 策略、符号链接、导出不覆盖，以及原有快照与凭据保护检查。
- 当前 17 项隔离检查与 macOS 构建通过，实际扫描可用；原生选择器及原生确认的取消交互仍待 GUI 验收，不能仅由 Mock IPC 或编译通过推断其交互正确。
- 文件快照不是跨程序的文件系统锁，仍存在检查与写入之间的并发窗口。后续跨平台、签名公证、远程服务和团队功能需要分别评审，不因当前检查通过自动视为安全。

依据：[Tauri Capabilities](https://v2.tauri.app/security/capabilities/)、[原生 Dialog](https://v2.tauri.app/plugin/dialog/)、[Tauri Specta](https://specta.dev/docs/tauri-specta)。本地技能：[tauri](../.agents/skills/tauri/SKILL.md)、[security](../.agents/skills/tauri-security/SKILL.md)、[IPC](../.agents/skills/tauri-ipc/SKILL.md)、[framework-security](../.agents/skills/tauri-framework-security/SKILL.md)、[dialog](../.agents/skills/tauri-app-dialog/SKILL.md)。
