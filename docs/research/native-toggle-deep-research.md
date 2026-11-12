# 原生启停状态深入调查

核验日期：2026-11-10。范围：本机 macOS 上 Claude Code、Codex、Cursor、Antigravity 的 Skill / MCP / 插件启停。目标是找到可管理的原生状态，区分配置入口与会话实际加载，而不是以文件存在推断启用。

## 结论与本轮落地

此前的“状态未确认”既包含尚未实现的适配，也包含上下文缺失、部分可见性、缓存清单和路径匹配问题，不能作为单一产品状态。

| 情况 | 证据与处理 | 本轮结果 |
| --- | --- | --- |
| Codex MCP | 官方配置支持 `mcp_servers.<name>.enabled`；false 关闭而保留条目 | 接入状态读取与预览切换；缺省 true；保留其他字段及 TOML 注释 |
| Antigravity MCP | 官方统一 schema 支持 `mcpServers.<name>.disabled`，默认 false | 接入状态读取与预览切换；开关与 disabled 取反，不删除配置 |
| Claude 项目 Skill / 插件 local 覆盖 | 官方 settings 层级支持项目 local 覆盖项目 settings | 已有 local 记录优先；修改该记录所在文件。插件也识别仅存在于 local 的唯一启用记录 |
| Claude MCP 无项目上下文 | 原有实现针对项目 `disabledMcpServers`，不能把用户级服务器的某个项目状态称作全局开关 | 显示“需选择项目”，选定后读取该项目配置 |
| Claude Skill 部分可见状态 | `name-only` / `user-invocable-only` 不等于完全 on 或 off | 显示“部分可见”，保留原生含义，暂不提供会丢失该语义的二态开关 |
| 插件只有 manifest | 缓存、市场目录与已启用安装不是同一概念 | 显示“缺少启用记录”或“启用记录不唯一”，不默认 enabled |
| Cursor MCP | 本机实现包含工作区和 profile 两套禁用列表，运行时 gate 决定采用哪套 | 已定位原因，详情说明“需在 Cursor 管理”；尚未实现可靠的外部控制适配 |
| Antigravity 插件 | 官方公开 CLI 支持 list JSON / enable / disable，本机 list JSON 成功 | 已证明可查询，并非无法支持；执行计划、安装路径映射、状态回读与撤销尚待适配，当前不伪装成已接入开关 |
| Codex 链接 Skill | 还需核对本机版本对选择器、规范化路径、别名和链接的实际匹配 | 显示“需核验路径映射”，保留限制；本轮源码下载未成功，不声称已验证 |

### 官方依据

- [Codex MCP](https://learn.chatgpt.com/docs/extend/mcp#other-configuration-options)：可配置 enabled，不必移除服务条目。
- [Codex 配置参考](https://learn.chatgpt.com/docs/config-file/config-reference)：`mcp_servers.<id>.enabled` 为 boolean。
- [Antigravity MCP schema](https://antigravity.google/docs/mcp#mcp-configuration-properties)：disabled 是服务级连接开关；enabledTools / disabledTools 是工具级过滤，不能混用。
- [Claude settings](https://code.claude.com/docs/en/settings#settings-precedence)：用户、项目、项目 local 与 managed 是不同层级。
- [Claude Skill visibility](https://code.claude.com/docs/en/skills#override-skill-visibility-from-settings)：Skill 可见性不止二态。
- [Cursor MCP](https://cursor.com/docs/mcp)：官方产品提供 Customize 启停入口；这并未给第三方承诺稳定的数据库写入协议。
- [Antigravity plugins](https://antigravity.google/docs/plugins)：公开 `agy plugin list --json` 和 enable / disable 命令。

## 本机只读证据

### 版本

- Codex CLI：0.166.0（`codex --version`）。
- Claude Code CLI：2.1.308（`claude --version`）。
- Cursor：3.29.15（应用 Info.plist）。
- Antigravity App：2.30.0；IDE：2.6.0（各自应用 Info.plist）。不同界面不能混为同一版本。

### Cursor 内部状态

一手证据文件：`/Applications/Cursor.app/Contents/Resources/app/out/vs/workbench/workbench.desktop.main.js`。

SHA-256：`4d946eb3ceaac507fb22ad694e2c1b03b0d6a82cdabfa825b174aff9ac5f2857`。

静态代码中可核对的符号与分支：

- `disabledMcpServers`、`disabledGlobalMcpServers` 是分别注册的存储项。
- `useProfileScopedGlobalMcpDisabledState` 来自 `mcp_profile_scoped_global_disabled_state` 功能开关。
- `getDisabledServersListFor` 根据该 gate 与 `projectManaged` 选择禁用列表。
- `setServerEnabled` 修改所选列表，并执行项目白名单、替代配置、客户端清理等原生服务逻辑。
- `resolveEffectiveServers` 同时考虑配置解析和所选禁用列表；启用判断还涉及审批与策略。

因此，“只给 mcp.json 加 disabled 字段”或“直接改一个 SQLite 数组”都没有足够依据保证与本机 UI 行为一致。当前已定位存储和调用路径，剩余问题是实现稳定且可回读的客户端适配。没有修改 Cursor 数据库。

### 可立即覆盖的 MCP 配置入口

仅统计结构与布尔字段，不输出命令、环境变量、URL、鉴权数据：

- `~/.codex/config.toml`：8 个 MCP 条目，8 个可按原生字段解析状态。
- `~/.gemini/config/mcp_config.json`：2 个 MCP 条目，2 个可按原生字段解析状态。

这意味着新增适配可覆盖本机这 10 个配置入口；这是只读解析统计，不是 10 次启停运行验收。

### Antigravity 插件

执行公开只读命令 `agy plugin list --json`，得到 4 个安装记录，3 个 enabled=true，1 个 false。`gsheets` 为 false；其余为 true。输出含安装路径，可用于后续与扫描到的 manifest 核对，不能仅按显示名拼接。

本机 `~/.local/bin/agy` 是一个转发到 `agy-native/agy` 的包装脚本。不能假定所有用户机器都采用该路径；后续产品适配还需可执行文件发现、版本检测、超时、输出校验，以及现有变更计划对 CLI 操作的支持。本轮没有运行 enable / disable。

## 实施边界与验证

- 开关仍先生成差异预览，沿用确认、前置快照、备份与历史撤销。
- MCP 仅改原条目的布尔字段，不重新构造服务，不删配置或凭据。
- Claude 已有项目 local 覆盖直接作为目标；后端白名单仅扩展到已添加项目的 `.claude/settings.local.json`，拒绝任意路径与符号链接祖先。
- 读取异常有单独标签与原因；不统一冒充未启用或未安装。
- 配置入口状态不等于组织策略允许、会话已加载、进程已停止或连接可用。
- 完成类型绑定生成、前端构建、Rust 编译检查与格式检查；没有改写真实 Agent 配置，没有运行启停验收，也没有额外运行测试。

研究方法：官方文档、本机已安装产品的静态实现、原生公开只读命令与配置布尔字段。按 research Skill 尝试独立后台研究，但环境拒绝其模型，主代理继续完成调查。部分 GitHub 源码请求超时，未把下载失败当作“不支持”的证据。
