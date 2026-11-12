# 原生启停配置

已实现 Claude Code 普通独立 Skill、用户/项目 MCP、已有明确启用记录的插件，Codex 的 Skill / MCP，以及 Antigravity 的 MCP 配置开关。其他组合在详情中说明具体原因，未接入托管停用或通过移动文件模拟禁用。

## 使用

- 单个可操作实例在卡片中部同行显示配置状态和开关；Agent 归属由卡片图标及开关提示说明。
- 多实例或未确认原生状态时，卡片不额外增加操作入口，点击卡片进入抽屉查看说明和选择操作对象。
- 点击开关生成预览，核对原生配置文件和作用范围。应用前仍使用既有原生写入确认、前置快照检查与私有备份；取消不修改，应用后重新读取状态。
- Claude MCP 的停用是按项目、按服务名生效的，可能影响此项目其他来源的同名服务。用户级 MCP 必须先选择已添加项目；不会把它显示成全局停用。
- 状态表示配置允许或禁用，不代表当前会话已经加载或重载。Codex 配置变更后需要重启；其他端需在原生会话确认。

## 后端行为

| 组合 | 原生落盘位置 / 行为 |
|---|---|
| Claude Skill | 同作用域 `.claude/settings.json` 的 `skillOverrides[name] = "on" / "off"`；只支持目录名与名称一致的直接独立 Skill；若有已存在的项目 local 覆盖，读取并修改 `.claude/settings.local.json` 中该项。特殊来源与部分可见状态不自动处理 |
| Claude MCP | `~/.claude.json` 的 `projects[project].disabledMcpServers`；只调整用户配置服务器的 opt-out，不处理内置默认关闭服务器的 opt-in |
| Claude 插件 | 合并同作用域 settings 与项目 local 的已有 `enabledPlugins` 记录，同键 local 优先，修改获胜记录所在文件；只支持唯一匹配的布尔记录。仅有 manifest、记录不唯一时不提供开关；不承诺已安装或运行 |
| Codex Skill | 同作用域 config.toml 的 `skills.config`，读取按名称/路径匹配的最后一项；追加针对绝对 SKILL.md 路径的明确覆盖，保留原有条目和注释；链接/映射入口路径匹配未实测，只在实际目录条目提供开关 |
| Codex MCP | 扫描到的 config.toml 中 `mcp_servers[name].enabled`，缺省按 true 读取；仅编辑该字段，保留其他条目与 TOML 注释 |
| Antigravity MCP | 扫描到的 mcp_config.json 中 `mcpServers[name].disabled`，缺省按 false 读取；开关状态取反，停用不删除服务 |

“尚未接入”“需选择项目”“部分可见”“缺少启用记录”“启用记录不唯一”“配置读取异常”等为不同情况，详情同时展示具体原因。Cursor MCP 已定位到内部存储，但配置档/项目与功能开关会影响取值，当前不直接写其运行中的数据库。Antigravity 插件公开 CLI 已在本机只读查询成功，其完整管理协议尚未接入，不能以 manifest 推断状态。研究依据见 [深入调查](research/native-toggle-deep-research.md)。

`toggle_states` 仅接收已添加项目上下文；重新扫描资源，按原生文件缓存读取配置，不输出秘密值。`preview_toggle` 重新解析资源并核对 Agent、配置路径、选择器及原状态；前端无法指定任意写入目标。执行仍由后端计划和目标白名单控制，允许已知 `.claude/settings.json` 及已添加项目的 `.claude/settings.local.json` 路径。源 Skill 及链接、MCP 条目和凭据、插件文件均保留。开关读取的是具体配置入口，更高层配置、组织策略与运行中会话仍需另行确认。

本机版本、组织管理策略、额外 CLI 参数和会话实际加载仍需原生验收。此轮仅进行静态检查与构建，未改写用户真实 Agent 配置，也未宣称已完成各客户端运行验证。

来源：[Claude Skill 可见性](https://code.claude.com/docs/en/skills#override-skill-visibility-from-settings)、[Claude MCP 禁用](https://code.claude.com/docs/en/mcp#disable-a-server-without-removing-it)、[Claude enabledPlugins](https://code.claude.com/docs/en/settings-reference#enabledplugins)、[Codex Skill 配置](https://learn.chatgpt.com/docs/build-skills#enable-or-disable-local-codex-skills)。官方资料核验：2026-11-10。
