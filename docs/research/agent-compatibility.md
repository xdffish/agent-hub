# macOS Coding Agent 配置兼容性调查

核验日期：2026-11-10。范围：Claude Code、OpenAI Codex、Cursor、Google Antigravity 的本地 Skills、MCP、插件管理。全部结论依据联网读取的一手官方文档；未读取用户真实配置、凭据或 Keychain，未安装插件，未运行四端兼容性实验。使用了 OpenAI Docs 技能核验 Codex。文档是当前公开能力快照，不代表用户已安装版本具有这些能力。

## 结论与产品边界

1. **可以共享 Skill 源文件，但不能承诺四端共用一个原生发现目录。** Codex 与 Cursor 明确支持用户级 `~/.agents/skills/`；Antigravity 当前用户级目录是 `~/.gemini/config/skills/`；Claude Code 使用 `~/.claude/skills/`。项目级 `.agents/skills/` 已被 Codex、Cursor、Antigravity 采用，Claude Code 的官方发现目录仍为 `.claude/skills/`。这给“同一份源文件、按 Agent 映射”提供依据，但不等于配置格式、调用语义和权限完全兼容。[Codex Skills](https://learn.chatgpt.com/docs/build-skills)、[Cursor Skills](https://cursor.com/docs/skills)、[Antigravity Skills](https://antigravity.google/docs/skills)、[Claude Skills](https://code.claude.com/docs/en/skills)
2. **MCP 需要适配器，不能把同一个 JSON 原样写给四端。** Codex 原生用户配置使用 TOML，其余三端为各自 JSON；远程地址、transport、环境变量、OAuth 字段不同，Claude Code 还有容易混淆的 local/project/user 三种 scope。产品应保留未知字段，预览逐端差异和实际落盘位置。[Codex MCP](https://learn.chatgpt.com/docs/extend/mcp)、[Claude MCP](https://code.claude.com/docs/en/mcp)、[Cursor MCP](https://cursor.com/docs/mcp)、[Antigravity MCP](https://antigravity.google/docs/mcp)
3. **四端现在都有原生插件能力。** 不能把 Cursor 或 Antigravity 标为“不支持插件”，也不能把 VS Code 扩展等同于 Agent 插件。插件共享/转换应按 manifest 和组件检查，调用各端原生管理入口优于直接修改缓存和内部注册表。[Claude 插件](https://code.claude.com/docs/en/discover-plugins)、[OpenAI 插件](https://developers.openai.com/plugins/build/plugins)、[Cursor 插件](https://cursor.com/docs/plugins)、[Antigravity 插件](https://antigravity.google/docs/plugins)
4. **“密钥仅存 Agent Hub Keychain，所有 Agent 自动引用”尚不成立。** 已证实的是若干环境变量和本地 helper 接口；未发现四端共同支持的 `keychain://` 或任意 macOS Keychain 引用协议。应将密钥保存、交付到 Agent 进程、Agent 自己维护的 OAuth 凭据分成三个问题。不能把客户端内部使用安全存储推导成 Agent Hub 可以写入或共享其凭据。

## Skills：目录、共享与重复发现

| Agent / surface | 用户级目录（已证实） | 项目级目录（已证实） | 特别限制 |
|---|---|---|---|
| Claude Code | `~/.claude/skills/<name>/SKILL.md` | `.claude/skills/<name>/SKILL.md` | 支持 plugin 提供的 skills；未在本次官方资料证实直接扫描 `.agents/skills`。这不是“不支持”的证明。 |
| Codex 本地 CLI / IDE | `~/.agents/skills/`；另有 `/etc/codex/skills/` 管理级与内置系统 skills | 从 CWD 向上至 repo root 的 `.agents/skills/` | 官方当前主表以 `.agents` 为准；历史 `~/.codex/skills` 不能据此假定已移除，兼容行为需检测版本。 |
| Cursor 本地 | `~/.agents/skills/`、`~/.cursor/skills/` | `.agents/skills/`、`.cursor/skills/` | 还兼容 `.claude/skills`、`.codex/skills` 及相应 home 路径；嵌套目录有 scope 语义。远程/Cloud 不自动继承所有本地用户 skills。 |
| Antigravity App / CLI / IDE | `~/.gemini/config/skills/` | `.agents/skills/`，向上扫描规则见各 surface 文档 | IDE 兼容旧 `~/.gemini/antigravity/skills/`；CLI 另读 `~/.gemini/antigravity-cli/skills/`、`~/.gemini/skills/`，App 不读这两项；兼容旧 `.agent/skills`。 |

表内来源：[Claude Skills — Choose where skills load](https://code.claude.com/docs/en/skills#choose-where-skills-load)、[Codex — Where Codex loads local skills](https://learn.chatgpt.com/docs/build-skills#where-codex-loads-local-skills)、[Cursor — Skill directories](https://cursor.com/docs/skills#skill-directories)、[Antigravity — Skills by surface](https://antigravity.google/docs/skills#skills-by-surface)。Antigravity 另有 [Customization discovery](https://antigravity.google/docs/customization-discovery)；不要将其 App、CLI、独立 IDE、SDK 的能力混写。

**符号链接证据：** Claude Code 明确允许 enterprise/personal/project 中的 skill 子目录指向外部目录，并按同一目标去重；其插件 skills 有不同规则。Codex 明确支持 symlinked skill folders 并跟随目标。Cursor 和 Antigravity 的普通 skills symlink 行为在本次读取资料中未被证实，应列为版本化验证项，不应从文件系统常识推导承诺。[Claude Skills](https://code.claude.com/docs/en/skills#choose-where-skills-load)、[Codex Skills](https://learn.chatgpt.com/docs/build-skills#where-codex-loads-local-skills)

**共享方案的合理推断：** 将基础 skill 放进 `~/.agents/skills/<name>`，Codex 与 Cursor 可直接消费，Claude 在其原生目录放一个指向该 skill 的 symlink；Antigravity 可先提供复制/原生目录安装，symlink 作为待实测选项。这只是产品方案，不是已经运行验证的实现。Cursor 已会兼容读取 Claude/Codex 目录，不能再向它的多个目录重复安装同一 skill 后承诺自动去重。Codex 同名 skill 不合并，选择器可能同时显示，故不能按 `name` 唯一性默默删除。

**内容可移植性：** 四端支持 `SKILL.md` 不等于所有 frontmatter、动态 shell 注入、subagent、hooks 和工具名称均可移植。管理器应区分“共享源文件”与“已验证语义兼容”，保留原文件并提示 Agent 专用扩展。[Claude 扩展与标准](https://code.claude.com/docs/en/skills)、[Codex optional metadata](https://learn.chatgpt.com/docs/build-skills#optional-metadata)

## MCP：配置格式和作用域

| Agent | 全局 / 用户配置 | 项目配置 | schema / transport 关键差异 |
|---|---|---|---|
| Claude Code | `~/.claude.json` 顶层 `mcpServers` 为 user；该文件 `projects[绝对路径].mcpServers` 为 local | repo 根 `.mcp.json`：`mcpServers` | stdio `command/args/env`；远程需要 `type: "http"`（也接受 `streamable-http`），`url/headers`。省略远程 `type` 会被当作 stdio 并报错。支持 SSE（已弃用）等其他 transport。 |
| Codex | 默认 `~/.codex/config.toml`（实际位置受 `CODEX_HOME` 影响） | `.codex/config.toml`；项目配置受信任约束 | `[mcp_servers.<name>]`；stdio `command/args/env/env_vars/cwd`；HTTP `url/http_headers/env_http_headers/bearer_token_env_var`。插件 MCP 是另一套 manifest schema。 |
| Cursor | `~/.cursor/mcp.json` | `.cursor/mcp.json` | `mcpServers`；stdio `command/args/env`，可用 `envFile`；远程 `url/headers`，支持 SSE/Streamable HTTP；`auth` 为其 OAuth 配置。 |
| Antigravity App / CLI / IDE | 当前 `~/.gemini/config/mcp_config.json` | `.agents/mcp_config.json` | `mcpServers`；stdio `command/args/env/cwd`；远程 `serverUrl`（也接受 `url`）和 `headers`。当前文档要求 Streamable HTTP，明确不支持旧 HTTP+SSE/`httpUrl`。SDK transport 支持范围另算。 |

来源：[Claude installation scopes / precedence](https://code.claude.com/docs/en/mcp#mcp-installation-scopes)、[Codex MCP](https://learn.chatgpt.com/docs/extend/mcp)、[Codex 配置参考](https://learn.chatgpt.com/docs/config-file/config-reference)、[Cursor MCP](https://cursor.com/docs/mcp)、[Antigravity MCP](https://antigravity.google/docs/mcp)。注意 `.claude/settings.local.json` 是一般 local settings，不是 Claude MCP local scope 的实际存储位置。

已确认 Claude 同名服务器 local > project > user，整项覆盖不合并；插件和其他来源另有去重规则。其他端的同名合并优先级不要沿用这一规则，需分别核验。配置“存在”与“被当前 session 加载”必须分开显示；组织策略与 workspace trust 也可能使磁盘配置无效。[Claude scope hierarchy](https://code.claude.com/docs/en/mcp#scope-hierarchy-and-precedence)

## 凭据与 macOS Keychain 的可行性

| Agent | 已证实的外部值交付方式 | 未证实 / 不能承诺 |
|---|---|---|
| Claude Code MCP | `${VAR}`、`${VAR:-default}` 可用于 command、args、env、url、headers；MCP 可走原生 OAuth | 没有证据表明 MCP 配置能直接引用 Agent Hub 自有 Keychain item。部分 Claude/cloud provider 凭据名在远程 URL/header 插值会被读为空，不能复用这些名称。 |
| Codex MCP | stdio `env_vars` 转发；HTTP `bearer_token_env_var`、`env_http_headers`；本地 HTTP 支持 `http_headers_helper` 输出 JSON header | helper 仅适用于本地环境发起的 HTTP MCP，不适用于 stdio 或远程执行环境。未证实通用任意 Keychain URI。 |
| Cursor MCP | `${env:NAME}`；stdio 有 `envFile`；OAuth 由客户端原生处理 | `envFile` 不适用于远程 HTTP/SSE；未证实任意 Keychain item 引用或 Agent Hub 代写 OAuth token 的接口。 |
| Antigravity MCP | 官方证实 stdio `env`、远程 `headers`、OAuth，以及 `authProviderType: "google_credentials"` 使用 ADC | 本次未证实 `${env:NAME}`/`${VAR}` 插值或通用 Keychain helper。不能照搬 Cursor/Claude 写法。官方说明 MCP OAuth token 存于 `~/.gemini/antigravity/mcp_oauth_tokens.json`，不应据此读取、复制或共享它。 |

来源：[Claude 环境变量与凭据限制](https://code.claude.com/docs/en/mcp#environment-variable-expansion-in-mcpjson)、[Codex HTTP / STDIO 字段](https://learn.chatgpt.com/docs/extend/mcp)、[Cursor interpolation 与 authentication](https://cursor.com/docs/mcp#config-interpolation)、[Antigravity schema 与 OAuth](https://antigravity.google/docs/mcp)。

**已具备原生接口的候选方案：** Codex `http_headers_helper` 可以由 Agent Hub 提供本地 helper 从其自有 Keychain item 取值，再输出 header JSON。这是基于官方命令接口的可实施推断，尚未编写或验证；需要验证权限提示、启动环境、错误处理、更新/撤销语义，不能写成已完成能力。

**跨端方案仍需评审：** 若 Agent Hub 从 Keychain 读取后仅给它启动的子进程注入环境变量，已有环境变量接口的客户端可能可用；但独立从 Finder/Dock 启动的现有应用不会因此自动得到 Agent Hub 的子进程环境。不能承诺写入一份引用即可让所有已运行 Agent 获得密钥。stdio 可考虑 wrapper 按次取值再启动原始 MCP server；远程 HTTP 可考虑本地 broker/代理，但这会增加进程、协议和安全边界，不属于已证实的客户端原生 Keychain 兼容性。

原生 OAuth 应让每个客户端自行登录。客户端的 token audience、刷新方式和存储实现可能不同；“同一 MCP endpoint”不等于“能共享同一份 OAuth token”。产品第一阶段可只管理不含秘密的配置与 secret reference 元信息，并将凭据交付标为需配置/未验证。

## 插件：原生管理优先，禁止假定缓存可互换

| Agent | 已证实的原生入口 / 包结构 | 管理限制 |
|---|---|---|
| Claude Code | `/plugin`；shell `claude plugin install/list/enable/disable/uninstall`；`.claude-plugin/plugin.json`；可含 skills、agents、hooks、MCP | user/project/local 启用状态分别落在 `~/.claude/settings.json`、`.claude/settings.json`、`.claude/settings.local.json` 的 `enabledPlugins`。项目启用记录不等于同事机器已经下载。开发可用 `--plugin-dir`。 |
| Codex | `/plugins` 浏览管理；OpenAI 便携 Agent Plugin 根 `plugin.json`，MCP 根 `mcp.json`；支持 `.codex-plugin/plugin.json` 兼容 fallback | 不能将任意 Claude 插件无损导入。OpenAI 迁移文档明确要求处理 Claude 专用 userConfig、hooks 等；公共目录、手工 desktop 安装的支持范围不同。此处不承诺稳定通用 shell 安装命令或可直接编辑的缓存布局。 |
| Cursor | Customize 安装并选 project/user scope；Agent Plugin 根 `plugin.json` 或 Cursor `.cursor-plugin/plugin.json`；开发目录 `~/.cursor/plugins/local` | 本地导入受企业策略控制；该开发目录的 symlink 只允许目标仍位于目录内部，指向目录外插件仓库会被跳过。不能以外链共享仓库方案覆盖此限制。 |
| Antigravity | Customizations Marketplace；CLI `/plugin`、`agy plugin install/list --json/enable/disable/uninstall`；支持 GitHub URL、本地路径，另有 `agy plugin import` | 手工插件目录为项目 `.agents/plugins/` 和用户 `~/.gemini/config/plugins/`；App 安装与 CLI 有同步，但本次未对 IDE 插件完整语义作运行验证。不能把“import”解释为所有组件完全等价。 |

来源：[Claude 安装与 scope](https://code.claude.com/docs/en/discover-plugins)、[Claude manifest](https://code.claude.com/docs/en/plugins-reference)、[OpenAI package](https://developers.openai.com/plugins/build/plugins)、[Claude 插件迁移到 OpenAI 的限制](https://developers.openai.com/plugins/guides/submit-claude-plugin)、[Codex CLI 命令](https://learn.chatgpt.com/docs/developer-commands)、[Cursor 安装与本地测试](https://cursor.com/docs/plugins)、[Antigravity 插件](https://antigravity.google/docs/plugins)。

Agent Hub 可以统一展示逻辑安装意图与逐端状态；不应直接删除或修改其他客户端原生插件缓存来模拟卸载，也不应将不同 manifest 的插件目录强行 symlink 到四端后标记安装成功。

## 变更如何生效

| 对象 | 有官方证据的行为 | 产品应展示 |
|---|---|---|
| Codex skills 文件 | 自动检测新增/修改，未出现时重启；`skills.config` 等配置变更要求重启 | “文件已更新”和“需重启”分别显示。[Skills](https://learn.chatgpt.com/docs/build-skills) |
| Codex MCP / IDE | IDE 保存服务器后选择 Restart extension | 外部文件写入后不能一概宣称即时加载。[MCP](https://learn.chatgpt.com/docs/extend/mcp) |
| Claude plugins | 安装结果可为立即 active，或提示 `/reload-plugins`；非交互环境的插件 MCP 连接变化有下次 session 限制 | 遵循原生命令回执；不得强行自动加 `--force`。[安装](https://code.claude.com/docs/en/discover-plugins)、[MCP](https://code.claude.com/docs/en/mcp) |
| Claude 独立 skills / 手改 MCP | 本次没有收敛到足够精确的统一热加载保证 | 标为“已写入，生效待确认”，提供新 session 检查建议。 |
| Cursor skills | 官方说明启动时发现 | 不承诺所有外部写入都实时生效。[Skills](https://cursor.com/docs/skills) |
| Cursor 自定义 MCP / 本地 plugins | 自定义 MCP 更新本地文件后重启 Cursor；本地 plugins 重启或 Developer: Reload Window | 明确需要 reload/restart。[MCP](https://cursor.com/docs/mcp)、[Plugins](https://cursor.com/docs/plugins) |
| Antigravity CLI skills / MCP | 新 skill 后 `/skills reload` 或新 conversation；`/mcp` manager 可手动 reload 配置 | App/IDE 不套用 CLI slash command 保证。[Skills](https://antigravity.google/docs/skills)、[MCP](https://antigravity.google/docs/mcp) |

## 第一阶段的兼容性验收清单

这些是后续实施验证项，不表示本次已做：

- 按实际已安装版本与 surface 建立能力表，保留文档日期、来源、配置路径及 provenance；未知能力显示未知。
- 使用无密钥临时 HOME/隔离测试目录，验证 discovery、同名冲突、symlink 去重、删除和重载；不拿用户真实配置实验。
- 先覆盖四端 stdio 与 Streamable HTTP 的最小配置，再逐项支持 timeout、工具禁用、OAuth、插件组件；不静默丢弃未知字段。
- 单独验证 Finder 启动、CLI 启动、远程 executor 的环境继承与 helper 可达性；缺少凭据交付渠道时阻止“已配置可用”的成功提示。
- 发现已有配置时只做可审阅的合并；插件安装走原生入口或明确的本地安装规则；保留用户更改并避免跨来源自动去重删除。

未证实表示本轮一手资料不足，不表示产品不支持。后续若以源码补强，应记录具体版本、文件及 commit permalink，不能用 issue、第三方教程或名称相近工具（如 Gemini CLI）的行为代替 Antigravity 本身的证据。
