# Skill 发现与同名处理

官方资料核验于 2026-11-10。本机安装版本与实际会话尚未验证；文档规则不等于加载结果。

| Agent / 形态 | 用户级 | 项目级 | 同名规则 |
|---|---|---|---|
| Claude Code 本地 | `~/.claude/skills` | `.claude/skills` | 目录或命令文件名冲突：企业 > 用户 > 项目；嵌套项目可带命名空间并存；同一真实目标去重 |
| Codex 本地 CLI / IDE | `~/.agents/skills` | CWD 向上至 repo root 的每层 `.agents/skills` | frontmatter 同名不合并，选择器可保留多个；不可直接判定项目覆盖用户 |
| Cursor 本地 | `~/.agents/skills`、`~/.cursor/skills` | `.agents/skills`、`.cursor/skills`，含嵌套项目范围 | 兼容 Claude/Codex 同作用域目录；官方本页未明确冲突优先级或去重顺序 |
| Antigravity App | `~/.gemini/config/skills` | 工作区及祖先至 repo root 的 `.agents/skills` | 仓库个人目录 > 工作区 > 用户 > 内置；manifest 可改变来源和排除项 |
| Antigravity CLI | 同 App，另有 `~/.gemini/antigravity-cli/skills`、`~/.gemini/skills` | 同 App | 两个 CLI 专属来源高于 App 的层级；二者互相顺序未证实 |
| Antigravity IDE | `~/.gemini/config/skills`，兼容 `~/.gemini/antigravity/skills` | `.agents/skills`，旧 `.agent/skills` 为兼容入口 | 未证实新旧用户目录间顺序，不沿用 App/CLI 所有规则 |

来源：[Claude locations / name conflicts](https://code.claude.com/docs/en/skills#resolve-skills-that-share-a-name)、[Codex local skills](https://learn.chatgpt.com/docs/build-skills#where-codex-loads-local-skills)、[Cursor directories](https://cursor.com/docs/skills#skill-directories)、[Antigravity surfaces](https://antigravity.google/docs/skills#skills-by-surface)、[Antigravity conflict rules](https://antigravity.google/docs/customization-discovery#which-customization-wins)。

## 产品表达

- 每个 Agent 的每个发现位置分别标注用户级/项目级、共享/原生/兼容/历史候选；同一路径对不同 Agent 可有不同类别。
- 名称归组用于浏览；Claude 的冲突身份还取决于目录名、调用名和命名空间，不能将分组直接当成覆盖关系。
- 选择项目时保留用户级候选用于比较；未选项目时不能把其他项目的候选合成一个生效集合。
- 将官方已确认规则、已扫描范围中的候选分析、实际会话加载结果分开呈现。
- 当前界面已显示规则、来源分类和上下文；并非完整加载解析器。祖先与嵌套项目目录、管理级、内置/启用插件、禁用设置、Antigravity manifests 和 CLI 专属目录扫描仍不完整，界面须提示这些边界。
- `CODEX_HOME` 的自定义 Skill 候选不能自动归为 Cursor 支持的路径；Cursor 的兼容目录仍为其文档指定的 `~/.codex/skills`。

## 后续完整解析所需输入

具体 Agent 版本与形态、工作区根与 CWD、仓库边界、原生设置与额外发现参数、manifest 与 exclude、插件启用清单。收集和解析后，才可在已支持范围标记“按规则优先/被覆盖/并存/被禁用”；会话实测另行标记。
