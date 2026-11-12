import type { Resource } from "./types";

export const skillPolicies: Record<
  string,
  { title: string; rule: string; gaps: string }
> = {
  claude: {
    title: "Claude Code · 本地会话",
    rule: "同目录名：企业级 > 用户级 > 项目级。嵌套项目 Skill 可带命名空间并存；指向同一真实目录的入口去重。插件使用独立命名空间。",
    gaps: "当前未核验企业目录、额外 --add-dir、命令别名、可见性设置或会话加载结果。不能按 frontmatter 名称直接判定覆盖。",
  },
  codex: {
    title: "Codex · 本地 CLI / IDE",
    rule: "当前工作目录至仓库根的 .agents/skills、用户 ~/.agents/skills、管理级及内置来源。同名不合并，选择器可同时列出；不存在通用的项目覆盖用户规则。",
    gaps: ".codex/skills 是历史候选，不等同当前文档推荐位置；版本支持、禁用设置、管理级与内置资源、祖先目录尚未完整核验。",
  },
  cursor: {
    title: "Cursor · 本地",
    rule: "用户级和项目级支持 .cursor/skills、.agents/skills；兼容 .claude/skills、.codex/skills。嵌套项目目录受文件范围约束。",
    gaps: "官方 Skills 页未明确同名时各目录间的优先级和去重顺序；不按文档列举顺序推断赢家。Cloud / SSH 不等同本机。",
  },
  antigravity: {
    title: "Antigravity · 需区分 App / CLI / IDE",
    rule: "App：仓库个人目录 > 工作区 .agents/skills > 用户 ~/.gemini/config/skills > 内置。CLI 的 ~/.gemini/antigravity-cli/skills、~/.gemini/skills 还高于这些层。IDE 支持旧 ~/.gemini/antigravity/skills，但该层与新目录的冲突顺序未确认。",
    gaps: ".agent/skills 是项目兼容入口。当前未解析 discovery manifest、排除规则、仓库个人目录或确认实际使用的 App / CLI / IDE，不能据此宣称最终生效。",
  },
};

export function skillOrigin(r: Resource, agent: string): string {
  const scope = r.scope === "global" ? "用户级" : "项目级";
  const p = r.path.replaceAll("\\", "/");
  if (p.includes("/.agents/skills/")) return `${scope} · 共享发现目录`;
  if (p.includes("/.claude/skills/"))
    return `${scope} · ${agent === "claude" ? "原生目录" : "Claude 兼容目录"}`;
  if (p.includes("/.cursor/skills/")) return `${scope} · 原生目录`;
  if (p.includes("/.codex/skills/"))
    return `${scope} · ${agent === "cursor" ? "Codex 兼容目录" : "历史候选目录，版本待核验"}`;
  if (p.includes("/.gemini/antigravity/skills/"))
    return "用户级 · IDE 旧兼容目录";
  if (p.includes("/.gemini/config/skills/"))
    return "用户级 · App / CLI / IDE 原生目录";
  if (p.includes("/.agent/skills/")) return "项目级 · 旧兼容目录";
  return `${scope} · 来源或版本支持待核验`;
}

export function candidateAssessment(
  agent: string,
  rows: Resource[],
  context: string,
): string {
  if (agent === "codex") return "同名可并存；逐路径确认启用状态，不推断覆盖";
  if (agent === "cursor") return "目录优先级未明确；同名候选需在 Cursor 核验";
  if (agent === "antigravity")
    return "先确认 App / CLI / IDE 及 manifest；再按对应规则分析";
  if (context === "all") return "未选定项目上下文，暂不比较跨项目优先级";
  const native = rows.filter((r) => r.path.includes("/.claude/skills/"));
  const names = new Set(native.map((r) => r.path.split("/").pop()));
  if (names.size > 1)
    return "frontmatter 同名但目录名不同；不能按此分组判定覆盖";
  if (
    native.some((r) => r.scope === "global") &&
    native.some((r) => r.scope !== "global")
  )
    return "已扫描范围内：同目录名用户级优先于项目级；企业级等来源未核验";
  return "按企业级 > 用户级 > 项目级分析；当前候选与其他来源仍需核验";
}
