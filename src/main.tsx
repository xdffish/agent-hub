import React, { useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { isTauri } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import {
  Activity,
  ArrowDownToLine,
  ArrowLeftRight,
  ArrowUpRight,
  BookOpen,
  Check,
  ChevronDown,
  ChevronRight,
  CircleAlert,
  Clock3,
  Code2,
  Download,
  FileCode2,
  Folder,
  FolderPlus,
  GitBranch,
  Layers3,
  ListChecks,
  LayoutDashboard,
  LoaderCircle,
  LockKeyhole,
  Package,
  Pin,
  Plug,
  Plus,
  RefreshCw,
  Search,
  Settings2,
  ShieldCheck,
  Sparkles,
  Trash2,
  Undo2,
  Upload,
  X,
} from "lucide-react";
import type {
  Agent,
  History,
  Plan,
  Resource,
  Scan,
  Skill,
  Target,
} from "./types";
import { commands, type PathPurpose, type NativeControl } from "./bindings";
import "./styles.css";
import "@fontsource-variable/dm-sans";
import "@fontsource/ibm-plex-mono/400.css";
import claudeIcon from "./assets/agents/claude.png";
import codexIcon from "./assets/agents/codex.png";
import cursorIcon from "./assets/agents/cursor.png";
import antigravityIcon from "./assets/agents/antigravity.png";
import { skillOrigin } from "./skill-discovery";

const names: Record<string, string> = {
  claude: "Claude Code",
  codex: "Codex",
  cursor: "Cursor",
  antigravity: "Antigravity",
};
const agentIcons: Record<string, string> = {
  claude: claudeIcon,
  codex: codexIcon,
  cursor: cursorIcon,
  antigravity: antigravityIcon,
};
const kinds: Record<string, string> = {
  skill: "Skills",
  mcp: "MCP 服务",
  plugin: "插件",
};
const compatibilityRows = [
  {
    agent: "Claude Code",
    kind: "Skills",
    user: "~/.claude/skills/",
    project: "<repo>/.claude/skills/",
    precedence: "企业 > 用户 > 项目；同一真实目录去重，插件技能独立命名空间",
    manage:
      "Agent Hub 管理源副本；分配到 .claude/skills。插件用 Claude 原生 /plugin。",
  },
  {
    agent: "Claude Code",
    kind: "MCP",
    user: "~/.claude.json（user）；按项目记录 local scope",
    project: "<repo>/.mcp.json（project）",
    precedence: "local > project > user；同名整项覆盖",
    manage: "Agent Hub 预览适配后写入；密钥和 OAuth 由 Claude 原生流程管理。",
  },
  {
    agent: "Claude Code",
    kind: "插件",
    user: "用户级 enabledPlugins / marketplace 安装",
    project: ".claude/settings.json；项目插件声明",
    precedence: "安装、启用与配置作用域分开；不直接改缓存",
    manage:
      "通过 Claude 原生 /plugin 管理；Agent Hub 展示 manifest 与官方入口。",
  },
  {
    agent: "Codex",
    kind: "Skills",
    user: "~/.agents/skills/；管理级 /etc/codex/skills/",
    project: "CWD 至 repo root 每层 .agents/skills/",
    precedence: "同名不合并，可同时出现在选择器；另有系统技能",
    manage: "Agent Hub 分配至 .agents/skills；状态配置仍在 Codex 原生设置。",
  },
  {
    agent: "Codex",
    kind: "MCP",
    user: "$CODEX_HOME/config.toml（默认 ~/.codex/config.toml）",
    project: "受信任项目的 .codex/config.toml",
    precedence: "项目配置受信任状态约束；不要把 TOML 当 JSON",
    manage:
      "Agent Hub 按 TOML 适配器预览写入；密钥/OAuth 使用 Codex 原生方式。",
  },
  {
    agent: "Codex",
    kind: "插件",
    user: "Codex 原生插件目录 / 插件商店",
    project: "依插件分发及项目能力而异",
    precedence: "插件 manifest 可含 skills、MCP 等；不能等同独立 Skill",
    manage: "Codex 原生 /plugins 管理；Agent Hub 不直接改内部缓存。",
  },
  {
    agent: "Cursor",
    kind: "Skills",
    user: "~/.agents/skills/、~/.cursor/skills/；兼容 ~/.claude/skills、~/.codex/skills",
    project:
      ".agents/skills/、.cursor/skills/；兼容 .claude/skills、.codex/skills",
    precedence: "兼容目录已确认；各目录同名优先级未由官方明确",
    manage: "优先选一个规范目录；Agent Hub 标出所有候选，不猜最终赢家。",
  },
  {
    agent: "Cursor",
    kind: "MCP",
    user: "~/.cursor/mcp.json",
    project: "<repo>/.cursor/mcp.json",
    precedence: "配置 scope 依客户端规则；多文件合并/覆盖须在当前版本确认",
    manage:
      "Agent Hub 按 Cursor JSON schema 预览配置；OAuth 在 Cursor 原生完成。",
  },
  {
    agent: "Cursor",
    kind: "插件",
    user: "Customize / 插件市场，支持 user scope",
    project: "Customize / 插件市场，支持 project scope",
    precedence: "插件声明和安装状态由 Cursor 管理；本地目录不等于已启用",
    manage:
      "使用 Cursor 原生 Customize 安装/启用；Agent Hub 提供清单和文档入口。",
  },
  {
    agent: "Antigravity App",
    kind: "Skills",
    user: "~/.gemini/config/skills/",
    project: "工作区及祖先目录 .agents/skills/",
    precedence:
      "个人定制目录 > 工作区 > 用户 > 内置；manifest 可 include/exclude",
    manage: "Agent Hub 展示候选；检查 .agents/skills.json 后再推断生效。",
  },
  {
    agent: "Antigravity App",
    kind: "MCP",
    user: "~/.gemini/config/mcp_config.json",
    project: "<repo>/.agents/mcp_config.json",
    precedence: "优先级/合并细节依官方配置；未据路径推断赢家",
    manage:
      "Agent Hub 按 Antigravity JSON schema 预览；OAuth 由原生客户端处理。",
  },
  {
    agent: "Antigravity App",
    kind: "插件",
    user: "~/.gemini/config/plugins/；Marketplace",
    project: "<repo>/.agents/plugins/；Marketplace",
    precedence: "manifest 存在不等于 App 已安装或启用",
    manage: "走 Antigravity Marketplace / 原生管理入口；不改插件缓存。",
  },
];
const time = (s: string) =>
  new Date(Number(s) * 1000).toLocaleString("zh-CN", { hour12: false });
const shortPath = (p: string) => p.replace(/^\/Users\/[^/]+/, "~");
type View =
  | "overview"
  | "resources"
  | "library"
  | "compatibility"
  | "history"
  | "settings";
type Modal =
  | "import"
  | "deploy"
  | "mcp"
  | "project"
  | "export"
  | "restore"
  | "check"
  | null;
type McpCheck = {
  state: "checking" | "success" | "error";
  message: string;
  checkedAt: string;
  fingerprint: string;
};
function checkFor(resource: Resource, checks: Record<string, McpCheck>) {
  const result = checks[resource.id];
  return result?.fingerprint === resource.fingerprint ? result : undefined;
}
function checkDescription(
  resource: Resource,
  checks: Record<string, McpCheck>,
) {
  const result = checkFor(resource, checks);
  return result
    ? `${resource.description.split(" · ")[0]} · ${result.state === "checking" ? "检查中…" : result.state === "success" ? "初始化检查通过" : "检查未通过"}`
    : resource.description;
}
type Selection =
  { type: "resource"; item: Resource } | { type: "skill"; item: Skill };

function AgentMark({ id, small = false }: { id: string; small?: boolean }) {
  return (
    <span
      className={`agent-mark ${id} ${small ? "small" : ""}`}
      aria-hidden="true"
    >
      <img src={agentIcons[id]} alt="" />
    </span>
  );
}
function useDialogFocus(key: string, onClose: () => void) {
  const close = useRef(onClose);
  close.current = onClose;
  useEffect(() => {
    if (!key) return;
    const previous = document.activeElement as HTMLElement | null;
    const dialog = document.querySelector<HTMLElement>(".modal");
    if (!dialog) return;
    const focusables = () =>
      Array.from(
        dialog.querySelectorAll<HTMLElement>(
          'button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex="0"]',
        ),
      );
    (
      dialog.querySelector<HTMLElement>("[autofocus]") ?? focusables()[0]
    )?.focus();
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        close.current();
      }
      if (e.key === "Tab") {
        const items = focusables();
        const first = items[0];
        const last = items[items.length - 1];
        if (
          e.shiftKey &&
          (document.activeElement === first ||
            !dialog.contains(document.activeElement))
        ) {
          e.preventDefault();
          last?.focus();
        } else if (
          !e.shiftKey &&
          (document.activeElement === last ||
            !dialog.contains(document.activeElement))
        ) {
          e.preventDefault();
          first?.focus();
        }
      }
    };
    document.addEventListener("keydown", handler);
    return () => {
      document.removeEventListener("keydown", handler);
      previous?.focus();
    };
  }, [key]);
}
function App() {
  const [appVersion, setAppVersion] = useState("0.1.0");
  const [view, setView] = useState<View>("overview");
  const [kind, setKind] = useState("all");
  const [agent, setAgent] = useState("all");
  const [scope, setScope] = useState("all");
  const [query, setQuery] = useState("");
  const [data, setData] = useState<Scan | null>(null);
  const [nativeControls, setNativeControls] = useState<NativeControl[]>([]);
  const [toggleProject, setToggleProject] = useState("");
  const [history, setHistory] = useState<History[]>([]);
  const [selection, setSelection] = useState<Selection | null>(null);
  const [modal, setModal] = useState<Modal>(null);
  const [plan, setPlan] = useState<Plan | null>(null);
  const [mcpChecks, setMcpChecks] = useState<Record<string, McpCheck>>({});
  const [busy, setBusy] = useState("");
  const [notice, setNotice] = useState<{ error: boolean; text: string } | null>(
    null,
  );
  const busyRef = useRef(false);
  const checkResultRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (isTauri()) {
      getVersion()
        .then((v) => setAppVersion(v))
        .catch(() => {});
    }
  }, []);
  useEffect(() => {
    if (selection?.type === "resource" && checkFor(selection.item, mcpChecks)) {
      checkResultRef.current?.scrollIntoView({
        block: "nearest",
        behavior: "auto",
      });
    }
  }, [mcpChecks, selection?.item.id]);
  useEffect(() => {
    if (!selection || modal || plan) return;
    const previous = document.activeElement as HTMLElement | null;
    const drawer = document.querySelector<HTMLElement>(".detail-panel");
    drawer?.focus();
    const keydown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        setSelection(null);
      }
      if (event.key === "Tab" && drawer) {
        const items = Array.from(
          drawer.querySelectorAll<HTMLElement>(
            'button:not(:disabled), [href], [tabindex="0"]',
          ),
        );
        const first = items[0],
          last = items[items.length - 1];
        if (
          event.shiftKey &&
          (document.activeElement === first ||
            document.activeElement === drawer)
        ) {
          event.preventDefault();
          last?.focus();
        } else if (
          !event.shiftKey &&
          (document.activeElement === last || document.activeElement === drawer)
        ) {
          event.preventDefault();
          first?.focus();
        }
      }
    };
    document.addEventListener("keydown", keydown);
    return () => {
      document.removeEventListener("keydown", keydown);
      previous?.focus();
    };
  }, [selection?.item.id, modal, plan?.id]);

  useDialogFocus(modal ?? plan?.id ?? "", () => {
    if (!busyRef.current) {
      setModal(null);
      setPlan(null);
    }
  });

  async function load(acceptChanges = false) {
    const snapshot = await commands.scan(acceptChanges);
    setData(snapshot);
    setHistory(await commands.history());
    const context = snapshot.projects.includes(toggleProject)
      ? toggleProject
      : "";
    setToggleProject(context);
    setNativeControls(await commands.toggleStates(context || null));
    setSelection((old) => {
      if (!old) return null;
      if (old.type === "resource") {
        const next = snapshot.resources.find((r) => r.id === old.item.id);
        return next ? { type: "resource", item: next } : null;
      }
      const next = snapshot.library.find((r) => r.id === old.item.id);
      return next ? { type: "skill", item: next } : null;
    });
  }
  async function job(label: string, action: () => Promise<void>) {
    if (busyRef.current) return;
    busyRef.current = true;
    setBusy(label);
    setNotice(null);
    try {
      await action();
    } catch (e) {
      setNotice({ error: true, text: String(e) });
    } finally {
      busyRef.current = false;
      setBusy("");
    }
  }
  useEffect(() => {
    if (isTauri()) void job("扫描本机资源", () => load());
  }, []);
  const resources = data?.resources ?? [];
  const changed = resources.filter((r) => r.changed).length;
  const filtered = resources.filter(
    (r) =>
      (kind === "all" || r.kind === kind) &&
      (agent === "all" || r.agents.includes(agent)) &&
      (scope === "all" ||
        r.scope === scope ||
        (kind === "skill" && r.scope === "global")) &&
      `${r.name} ${r.description} ${r.path}`
        .toLowerCase()
        .includes(query.toLowerCase()),
  );
  const library = (data?.library ?? []).filter((s) =>
    `${s.name} ${s.description} ${s.source}`
      .toLowerCase()
      .includes(query.toLowerCase()),
  );
  const installedSkill =
    selection?.type === "resource" && selection.item.library_id
      ? data?.library.find((s) => s.id === selection.item.library_id)
      : undefined;
  const currentSkill =
    selection?.type === "skill" ? selection.item : installedSkill;
  const detailLocations = selection
    ? resources.filter(
        (r) =>
          r.name === selection.item.name &&
          r.kind ===
            (selection.type === "skill" ? "skill" : selection.item.kind),
      )
    : [];
  const toggleResource = (control: NativeControl) => {
    if (control.enabled === null) return;
    void job("生成启停预览", () =>
      preview(
        commands.previewToggle(
          control.resource_id,
          control.agent,
          control.context ?? null,
          control.enabled!,
          control.selector,
          control.path,
          !control.enabled,
        ),
      ),
    );
  };
  const showResources = (nextKind = "all", nextAgent = "all") => {
    setView("resources");
    setKind(nextKind === "all" ? "skill" : nextKind);
    setAgent(nextAgent);
    setScope("all");
    setSelection(null);
    setQuery("");
  };
  async function preview(request: Promise<Plan>) {
    setPlan(await request);
    setModal(null);
  }
  const setMessage = (text: string) => setNotice({ error: false, text });
  const resourceCount = (a: string) =>
    resources.filter((r) => r.agents.includes(a)).length;

  if (!isTauri())
    return (
      <div className="browser-message">
        <div className="brand-icon">
          <Layers3 />
        </div>
        <h1>Agent Hub</h1>
        <p>本机资源管理需要在桌面应用中运行。</p>
        <code>npm run tauri dev</code>
        <p className="muted">
          浏览器模式不会访问本机配置，也不会显示模拟扫描结果。
        </p>
      </div>
    );

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-icon">
            <Layers3 size={23} />
          </div>
          <div>
            <strong>Agent Hub</strong>
            <span>LOCAL CAPABILITY LIBRARY</span>
          </div>
        </div>
        <div className="workspace">
          <span className="workspace-icon">
            <Code2 size={18} />
          </span>
          <div>
            <b>我的工作空间</b>
            <small>本机 · macOS</small>
          </div>
          <LockKeyhole size={13} className="muted" />
        </div>
        <nav aria-label="主导航">
          <button
            className={view === "overview" ? "nav-item active" : "nav-item"}
            onClick={() => {
              setView("overview");
              setSelection(null);
            }}
          >
            <LayoutDashboard size={18} />
            本机概览
          </button>
          <div className="nav-label">资源管理</div>
          {(["skill", "mcp", "plugin"] as const).map((k) => (
            <button
              key={k}
              className={
                view === "resources" && kind === k && agent === "all"
                  ? "nav-item active"
                  : "nav-item"
              }
              onClick={() => showResources(k)}
            >
              {k === "skill" ? (
                <Sparkles size={18} />
              ) : k === "mcp" ? (
                <Plug size={18} />
              ) : (
                <Package size={18} />
              )}
              {kinds[k]}
              <span className="nav-count">
                {k === "skill"
                  ? new Set([
                      ...resources
                        .filter((r) => r.kind === k)
                        .map((r) => r.name),
                      ...(data?.library ?? []).map((s) => s.name),
                    ]).size
                  : new Set(
                      resources.filter((r) => r.kind === k).map((r) => r.name),
                    ).size}
              </span>
            </button>
          ))}
          <button
            className={`nav-item ${view === "compatibility" ? "active" : ""}`}
            onClick={() => {
              setView("compatibility");
              setSelection(null);
            }}
          >
            <ListChecks size={18} />
            Agent 兼容矩阵
          </button>
          <div className="nav-label label-row">
            已接入的 Agent <span>04</span>
          </div>
          {Object.entries(names).map(([id, name]) => (
            <button
              key={id}
              className={
                view === "resources" && agent === id
                  ? "nav-item active agent-nav"
                  : "nav-item agent-nav"
              }
              onClick={() => showResources("all", id)}
            >
              <AgentMark id={id} small />
              {name}
              <i
                className={`status-dot ${data?.agents.find((a) => a.id === id)?.detected ? "online" : ""}`}
              />
            </button>
          ))}
          <div className="nav-label label-row">
            项目{" "}
            <button
              className="icon-button"
              title="添加项目"
              onClick={() => setModal("project")}
            >
              <Plus size={15} />
            </button>
          </div>
          {(data?.projects ?? []).map((p) => (
            <button
              key={p}
              className="nav-item project-nav"
              title={p}
              onClick={() => {
                showResources();
                setScope(p);
              }}
            >
              <Folder size={16} />
              <span className="ellipsis">{p.split("/").pop()}</span>
            </button>
          ))}
          {!data?.projects.length && (
            <button className="add-project" onClick={() => setModal("project")}>
              <Plus size={14} />
              添加项目目录
            </button>
          )}
        </nav>
        <div className="sidebar-bottom">
          <button
            className={`nav-item ${view === "history" ? "active" : ""}`}
            onClick={() => {
              setView("history");
              setSelection(null);
            }}
          >
            <Clock3 size={17} />
            变更历史
          </button>
          <button
            className={`nav-item ${view === "settings" ? "active" : ""}`}
            onClick={() => {
              setView("settings");
              setSelection(null);
            }}
          >
            <Settings2 size={17} />
            设置与备份
          </button>
          <div className="local-foot">
            <ShieldCheck size={14} />
            数据保存在本机<span>v{appVersion}</span>
          </div>
        </div>
      </aside>
      <div className="main-shell">
        <header className="topbar">
          <div className="breadcrumb">
            工作空间
            <ChevronRight size={13} />
            <b>
              {view === "overview"
                ? "本机概览"
                : view === "library"
                  ? "由 Agent Hub 管理"
                  : view === "history"
                    ? "变更历史"
                    : view === "compatibility"
                      ? "Agent 兼容矩阵"
                      : view === "settings"
                        ? "设置与备份"
                        : agent !== "all"
                          ? names[agent]
                          : (kinds[kind] ?? "全部资源")}
            </b>
          </div>
          <div className="topbar-right">
            <span className="local-badge">
              <span className="status-dot online" />
              本地模式
            </span>
            <span className="divider" />
            <button
              disabled={!!busy}
              className="button subtle compact"
              onClick={() => void job("重新扫描", () => load())}
            >
              <RefreshCw size={14} className={busy ? "spin" : ""} />
              重新扫描
            </button>
          </div>
        </header>
        <div className="main-scroll">
          {notice && (
            <div
              role="status"
              className={`notice ${notice.error ? "error" : ""} ${modal || plan ? "floating-notice" : ""}`}
            >
              <CircleAlert size={17} />
              <span>{notice.text}</span>
              <button
                className="icon-button"
                title="关闭消息"
                onClick={() => setNotice(null)}
              >
                <X size={16} />
              </button>
            </div>
          )}
          {busy && (
            <div className="busy-line" role="status">
              <LoaderCircle size={14} className="spin" />
              {busy}…
            </div>
          )}
          {view === "overview" && (
            <>
              <div className="page-heading">
                <div>
                  <div className="eyebrow">YOUR AGENTS, IN SYNC</div>
                  <h1>
                    本机资源，一处管理<span className="heading-dot">.</span>
                  </h1>
                  <p>看清每个 Agent 的能力、来源和配置差异。</p>
                </div>
                <button
                  className="button primary"
                  disabled={!!busy}
                  onClick={() => setModal("import")}
                >
                  <Plus size={17} />
                  导入 Skill
                </button>
              </div>
              <div className="stats">
                <div>
                  <span>
                    发现的资源位置
                    <Layers3 size={16} />
                  </span>
                  <strong>
                    {resources.length}
                    <small>处</small>
                  </strong>
                  <p>按实际文件位置盘点</p>
                </div>
                <div>
                  <span>
                    由 Agent Hub 管理
                    <BookOpen size={16} />
                  </span>
                  <strong>
                    {data?.library.length ?? 0}
                    <small>项</small>
                  </strong>
                  <p>已纳入管理的资源</p>
                </div>
                <div>
                  <span>
                    发现 Agent
                    <Activity size={16} />
                  </span>
                  <strong>
                    {data?.agents.filter((a) => a.detected).length ?? 0}
                    <small>/ 4</small>
                  </strong>
                  <p>基于应用目录与 PATH</p>
                </div>
                <div>
                  <span>
                    外部变更
                    <ArrowLeftRight size={16} />
                  </span>
                  <strong className={changed ? "orange" : ""}>
                    {changed}
                    <small>项</small>
                  </strong>
                  <p>{changed ? "与上次接纳内容不同" : "未发现新的内容差异"}</p>
                </div>
              </div>
              <div className="section-heading">
                <h2>
                  你的 Agents <span className="badge neutral">macOS</span>
                </h2>
                <button className="text-button" onClick={() => showResources()}>
                  查看全部 Skills
                  <ArrowUpRight size={14} />
                </button>
              </div>
              <div className="agent-grid">
                {(
                  data?.agents ??
                  Object.keys(names).map((id) => ({
                    id,
                    name: names[id],
                    detected: false,
                    evidence: "等待扫描",
                    guide: "",
                  }))
                ).map((a: Agent) => (
                  <button
                    className="agent-card"
                    key={a.id}
                    onClick={() => showResources("all", a.id)}
                  >
                    <div className="agent-card-top">
                      <AgentMark id={a.id} />
                      <span
                        className={`badge ${a.detected ? "green" : "neutral"}`}
                      >
                        {a.detected ? "已发现" : "未发现程序"}
                      </span>
                    </div>
                    <h3>{a.name}</h3>
                    <p className="ellipsis" title={a.evidence}>
                      {a.evidence}
                    </p>
                    <div className="agent-card-foot">
                      <b>
                        {resourceCount(a.id)} <span>个资源位置</span>
                      </b>
                      <ChevronRight size={15} />
                    </div>
                  </button>
                ))}
              </div>
              {changed > 0 && (
                <div className="attention-strip">
                  <CircleAlert size={20} />
                  <div>
                    <b>发现 {changed} 项外部修改</b>
                    <p>
                      请检查当前内容；接纳只更新盘点基准，不会覆盖 Agent 配置。
                    </p>
                  </div>
                  <button
                    className="button"
                    disabled={!!busy}
                    onClick={() =>
                      void job("接纳当前盘点", async () => {
                        await load(true);
                        setMessage("已将当前内容设为盘点基准。");
                      })
                    }
                  >
                    接纳当前内容
                  </button>
                </div>
              )}
              <div className="section-heading">
                <h2>
                  Skills 一览
                  <span className="muted small-text">发现文件 ≠ 已加载</span>
                </h2>
                <button className="text-button" onClick={() => setModal("mcp")}>
                  <Plus size={14} />
                  添加 MCP
                </button>
              </div>
              <SkillInventory
                controls={nativeControls}
                agentFilter="all"
                onToggle={toggleResource}
                busy={!!busy}
                rows={resources.filter((r) => r.kind === "skill")}
                library={data?.library ?? []}
                limit={5}
                onResource={(r) => setSelection({ type: "resource", item: r })}
                onSkill={(s) => setSelection({ type: "skill", item: s })}
              />
              <div className="bottom-note">
                <ShieldCheck size={15} />
                <span>
                  扫描只读；所有配置变更先预览，再应用。插件列表仅包含已识别的本地
                  manifest。
                </span>
              </div>
            </>
          )}
          {(view === "resources" || view === "library") && (
            <>
              <div className="page-heading">
                <div>
                  <div className="eyebrow">
                    {view === "library"
                      ? "ONE SOURCE, MULTIPLE AGENTS"
                      : "LOCAL INVENTORY"}
                  </div>
                  <h1>
                    {view === "library"
                      ? "由 Agent Hub 管理"
                      : kind === "skill"
                        ? agent === "all"
                          ? "Skills"
                          : `${names[agent]} · Skills`
                        : agent !== "all"
                          ? names[agent]
                          : (kinds[kind] ?? "全部资源")}
                  </h1>
                  <p>
                    {view === "library"
                      ? "保留来源与版本，按需分配到你的 Agent。扫描到的原生目录不会自动加入此库。"
                      : kind === "skill"
                        ? "按名称归组，查看管理版本、文件位置与 Agent 发现情况。点击条目查看说明和扫描目录。"
                        : "盘点已知配置位置；运行与加载状态需在原生 Agent 中确认。"}
                  </p>
                </div>
                <button
                  className="button primary"
                  disabled={!!busy}
                  onClick={() =>
                    setModal(
                      view === "resources" && kind === "mcp" ? "mcp" : "import",
                    )
                  }
                >
                  <Plus size={16} />
                  {view === "resources" && kind === "mcp"
                    ? "添加 MCP"
                    : "导入 Skill"}
                </button>
              </div>
              {view === "library" && (
                <div className="bottom-note skill-roots-note">
                  <BookOpen size={15} />
                  <span>
                    <b>原生 Skill 扫描位置：</b>
                    <code>~/.agents/skills</code>（Codex、Cursor）；
                    <code>~/.claude/skills</code>（Claude Code）；
                    <code>~/.gemini/config/skills</code>（Antigravity）。
                    项目级还扫描 <code>&lt;项目&gt;/.agents/skills</code>、
                    <code>&lt;项目&gt;/.claude/skills</code>、
                    <code>&lt;项目&gt;/.cursor/skills</code>、
                    <code>&lt;项目&gt;/.codex/skills</code> 和
                    <code>&lt;项目&gt;/.agent/skills</code>。 这些是由 Agent Hub
                    管理的副本；分配时才写入这些原生目录。
                  </span>
                </div>
              )}
              <div className="toolbar">
                <label className="search">
                  <Search size={16} />
                  <input
                    aria-label="搜索资源"
                    placeholder="搜索名称、用途或来源…"
                    value={query}
                    onChange={(e) => setQuery(e.target.value)}
                  />
                  <kbd>搜索</kbd>
                </label>
                {view === "resources" && (
                  <>
                    <select
                      aria-label="Agent 筛选"
                      value={agent}
                      onChange={(e) => setAgent(e.target.value)}
                    >
                      <option value="all">全部 Agent</option>
                      {Object.entries(names).map(([id, n]) => (
                        <option key={id} value={id}>
                          {n}
                        </option>
                      ))}
                    </select>
                    <select
                      aria-label="作用域筛选"
                      value={scope}
                      onChange={(e) => setScope(e.target.value)}
                    >
                      <option value="all">
                        {kind === "skill"
                          ? "全部位置（不推断项目生效）"
                          : "全部作用域"}
                      </option>
                      <option value="global">
                        {kind === "skill" ? "仅用户级候选" : "全局"}
                      </option>
                      {data?.projects.map((p) => (
                        <option value={p} key={p}>
                          {p.split("/").pop()}
                          {kind === "skill" ? " + 用户级候选" : ""}
                        </option>
                      ))}
                    </select>
                  </>
                )}
                {view === "library" && (
                  <button className="button" onClick={() => setModal("export")}>
                    <Upload size={15} />
                    导出
                  </button>
                )}
              </div>
              {view === "resources" ? (
                <>
                  <div className="tabs">
                    {["skill", "mcp", "plugin"].map((k) => (
                      <button
                        key={k}
                        className={kind === k ? "selected" : ""}
                        onClick={() => setKind(k)}
                      >
                        {k === "all" ? "全部资源" : kinds[k]}
                      </button>
                    ))}
                    <span>{filtered.length} 个原生资源位置</span>
                  </div>
                  {kind === "skill" ? (
                    <SkillInventory
                      controls={nativeControls}
                      agentFilter={agent}
                      onToggle={toggleResource}
                      busy={!!busy}
                      rows={filtered}
                      library={
                        agent === "all" && scope === "all"
                          ? library
                          : library.filter((s) =>
                              filtered.some((r) => r.library_id === s.id),
                            )
                      }
                      onResource={(r) =>
                        setSelection({ type: "resource", item: r })
                      }
                      onSkill={(s) => setSelection({ type: "skill", item: s })}
                    />
                  ) : (
                    <ResourceGroups
                      controls={nativeControls}
                      agentFilter={agent}
                      onToggle={toggleResource}
                      busy={!!busy}
                      key={kind}
                      resourceKind={kind}
                      rows={filtered}
                      selection={selection}
                      onSelect={(r) =>
                        setSelection({ type: "resource", item: r })
                      }
                    />
                  )}
                </>
              ) : (
                <div className="library-list">
                  {library.map((s) => (
                    <button
                      key={s.id}
                      className={`library-row ${selection?.item.id === s.id ? "selected" : ""}`}
                      onClick={() => setSelection({ type: "skill", item: s })}
                    >
                      <div className="resource-icon skill">
                        <Sparkles size={19} />
                      </div>
                      <div className="resource-label">
                        <b>{s.name}</b>
                        <p>{s.description}</p>
                        <span>
                          <GitBranch size={12} />
                          {shortPath(s.source)}
                        </span>
                      </div>
                      <span className="mono version">
                        {s.revision.slice(0, 8)}
                      </span>
                      {s.pinned && <Pin size={14} />}
                      <ChevronRight size={16} />
                    </button>
                  ))}
                  {!library.length && (
                    <Empty
                      title="还没有由 Agent Hub 管理的 Skill"
                      text="从已有目录或 Git 仓库导入，预览后纳入管理。"
                      action={() => setModal("import")}
                    />
                  )}
                </div>
              )}
            </>
          )}
          {view === "compatibility" && (
            <>
              <div className="page-heading">
                <div>
                  <div className="eyebrow">
                    DOCUMENTED LOCATIONS & MANAGEMENT
                  </div>
                  <h1>Agent 兼容矩阵</h1>
                  <p>
                    按 Agent
                    区分资源目录、作用域、同名规则和推荐管理入口；扫描到文件只代表候选存在，不代表已加载。
                  </p>
                </div>
                <button
                  className="button"
                  onClick={() =>
                    window.open(
                      "https://github.com/qingjian-team/qingjian",
                      "_blank",
                    )
                  }
                >
                  查看项目
                </button>
              </div>
              <div className="compat-toolbar">
                <span>官方文档核验：2026-11-10</span>
                <span>不确定项标为待核验；App、CLI、IDE 规则分开记录</span>
              </div>
              <div className="compat-table-wrap">
                <table className="compat-table">
                  <thead>
                    <tr>
                      <th>Agent</th>
                      <th>资源</th>
                      <th>用户级范围</th>
                      <th>项目级范围</th>
                      <th>优先级 / 同名规则</th>
                      <th>如何管理</th>
                    </tr>
                  </thead>
                  <tbody>
                    {compatibilityRows.map((r) => (
                      <tr key={`${r.agent}-${r.kind}`}>
                        <th>{r.agent}</th>
                        <td>
                          <span
                            className={`compat-kind ${r.kind === "Skills" ? "skill" : r.kind === "MCP" ? "mcp" : "plugin"}`}
                          >
                            {r.kind}
                          </span>
                        </td>
                        <td>{r.user}</td>
                        <td>{r.project}</td>
                        <td>{r.precedence}</td>
                        <td>{r.manage}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
              <p className="inventory-hint">
                范围依据仓库内兼容性调查与官方文档整理。实际行为可能受版本、工作区信任、Claude
                add-dir、Codex 启用设置、Antigravity manifests
                和插件状态影响。完整来源见{" "}
                <code>docs/research/agent-compatibility.md</code> 与{" "}
                <code>docs/research/skill-discovery-priority.md</code>。
              </p>
            </>
          )}
          {view === "history" && (
            <>
              <div className="page-heading">
                <div>
                  <div className="eyebrow">EVERY CHANGE HAS A TRACE</div>
                  <h1>变更历史</h1>
                  <p>
                    记录已应用的变更计划；应用前备份，撤销前检查后续修改。项目列表与固定状态等直接设置不在此日志内。
                  </p>
                </div>
              </div>
              <div className="history-list">
                {history.map((h) => {
                  const missing = (h.required_projects ?? []).filter(
                    (p) => !data?.projects.includes(p),
                  );
                  return (
                    <div key={h.id} className="history-row">
                      <span className="history-icon">
                        {h.status === "applied" ? (
                          <Check size={18} />
                        ) : (
                          <CircleAlert size={18} />
                        )}
                      </span>
                      <div>
                        <b>{h.title}</b>
                        <p>
                          {time(h.time)} · {h.paths.length} 个目标 ·{" "}
                          {h.undone_by
                            ? h.status === "applied"
                              ? "已撤销"
                              : "已恢复"
                            : h.status === "applied"
                              ? "已应用"
                              : h.status === "applying"
                                ? "执行被中断，请检查备份"
                                : "失败，请检查备份"}
                        </p>
                        {h.paths.map((p) => (
                          <code key={p}>{shortPath(p)}</code>
                        ))}
                        {h.undo_of && (
                          <p>反向操作 · 原记录 {h.undo_of.slice(0, 10)}</p>
                        )}
                        {h.undone_by && (
                          <p>
                            处理记录 {h.undone_by.slice(0, 10)}
                            ；需要反向恢复时请选择该记录。
                          </p>
                        )}
                        {!h.undone_by && missing.length > 0 && (
                          <p className="warning-text">
                            需重新授权项目：{missing.map(shortPath).join("、")}
                          </p>
                        )}
                      </div>
                      <button
                        disabled={!!busy || !!h.undone_by}
                        className="button"
                        onClick={() =>
                          void job("生成撤销预览", async () => {
                            for (const project of missing) {
                              const chosen =
                                await commands.choosePath("project");
                              if (!chosen) return;
                              if (chosen !== project)
                                throw new Error(`请选择原项目目录：${project}`);
                              await commands.addProject(chosen);
                            }
                            if (missing.length) await load();
                            await preview(commands.previewUndo(h.id));
                          })
                        }
                      >
                        <Undo2 size={14} />
                        {h.undone_by
                          ? "已处理"
                          : missing.length
                            ? "重新授权并预览"
                            : h.undo_of
                              ? "预览反向操作"
                              : h.status === "applied"
                                ? "预览撤销"
                                : "预览恢复"}
                      </button>
                    </div>
                  );
                })}
                {!history.length && (
                  <Empty
                    title="尚无配置变更"
                    text="扫描不会产生写入历史。应用变更后，你可以在这里查看和撤销。"
                  />
                )}
              </div>
            </>
          )}
          {view === "settings" && (
            <>
              <div className="page-heading">
                <div>
                  <div className="eyebrow">LOCAL BY DESIGN</div>
                  <h1>设置与备份</h1>
                  <p>按需运行，没有后台同步或云账号。</p>
                </div>
              </div>
              <section className="settings-section">
                <h2>Agent Hub 管理数据备份</h2>
                <p>
                  导出 Skill 文件、来源、版本和分配参考。MCP 配置、OAuth
                  和凭据不随包导出。请检查 Skill 内容中是否手工写入了敏感信息。
                  此备份不包含项目列表、变更历史和私有操作备份；导入不会自动分配到
                  Agent。
                </p>
                <div className="button-row">
                  <button className="button" onClick={() => setModal("export")}>
                    <Upload size={16} />
                    导出管理数据
                  </button>
                  <button
                    className="button"
                    onClick={() => setModal("restore")}
                  >
                    <Download size={16} />
                    导入备份
                  </button>
                </div>
              </section>
              <section className="settings-section">
                <h2>项目目录</h2>
                <p>仅扫描你主动添加的项目；移除项目不会删除其中的文件。</p>
                {data?.projects.map((p) => (
                  <div className="project-setting" key={p}>
                    <Folder size={16} />
                    <code>{p}</code>
                    <button
                      className="icon-button danger"
                      disabled={!!busy}
                      title="停止扫描此项目"
                      onClick={() =>
                        void job("移除项目", async () => {
                          await commands.removeProject(p);
                          await load();
                        })
                      }
                    >
                      <X size={16} />
                    </button>
                  </div>
                ))}
                <button className="button" onClick={() => setModal("project")}>
                  <FolderPlus size={16} />
                  添加项目
                </button>
              </section>
              <section className="settings-section">
                <h2>本机存储</h2>
                <code className="path-box">{data?.data_dir ?? "等待扫描"}</code>
                <p>
                  配置备份可能包含原有密钥，目录权限仅允许当前用户访问。应用默认退出时不保留扫描进程。
                </p>
              </section>
              <section className="settings-section">
                <h2>扫描边界与兼容性</h2>
                {data?.warnings.map((w, i) => (
                  <p key={i} className="warning-text">
                    <CircleAlert size={14} />
                    {w}
                  </p>
                ))}
                <p>
                  工具版本和插件语义存在差异；原生管理入口保留各平台的加载和鉴权边界。
                </p>
                <div className="button-row">
                  {Object.entries(names).map(([id, n]) => (
                    <button
                      key={id}
                      className="button compact"
                      onClick={() =>
                        void job("打开官方文档", async () => {
                          await commands.pluginGuide(id);
                        })
                      }
                    >
                      {n}
                      <ArrowUpRight size={13} />
                    </button>
                  ))}
                </div>
              </section>
            </>
          )}
          <footer className="page-footer">
            <span>
              <span className="status-dot online" />
              {data ? `上次扫描 ${time(data.scanned_at)}` : "等待本机扫描"}
            </span>
            <span>Agent Hub · 本机能力管理</span>
          </footer>
        </div>
      </div>
      {selection && (
        <>
          <button
            className="drawer-backdrop"
            aria-label="关闭详情抽屉"
            onClick={() => setSelection(null)}
          />
          <aside
            className="detail-panel"
            role="dialog"
            aria-modal="true"
            aria-label={`${selection.item.name} 详情`}
            tabIndex={-1}
          >
            <div className="detail-top">
              <span>资源详情</span>
              <button
                className="icon-button"
                title="关闭详情"
                onClick={() => setSelection(null)}
              >
                <X size={18} />
              </button>
            </div>
            <header className="drawer-hero">
              <div
                className={`detail-symbol resource-icon ${selection.type === "skill" ? "skill" : selection.item.kind}`}
              >
                {selection.type === "skill" ||
                selection.item.kind === "skill" ? (
                  <Sparkles size={25} />
                ) : selection.item.kind === "mcp" ? (
                  <Plug size={25} />
                ) : (
                  <Package size={25} />
                )}
              </div>
              <h2>{selection.item.name}</h2>
              <span className="badge neutral">
                {selection.type === "skill"
                  ? "Agent Hub 管理版本"
                  : selection.item.kind === "skill"
                    ? "目录中存在 · 加载状态未知"
                    : "已发现 · 加载待确认"}
              </span>
            </header>
            <section className="drawer-section drawer-purpose">
              <h3>说明与使用场景</h3>
              <p>
                {selection.item.description === "查看 SKILL.md 了解用途与依赖"
                  ? "暂未读取到用途说明，可打开目录查看 SKILL.md。"
                  : selection.type === "resource" &&
                      selection.item.kind === "mcp"
                    ? checkDescription(selection.item, mcpChecks)
                    : selection.item.description}
              </p>
            </section>
            <section className="drawer-section drawer-files">
              <div className="drawer-section-heading">
                <h3>文件位置</h3>
                <span>
                  {new Set(detailLocations.map((r) => r.canonical)).size} 份文件
                  / {detailLocations.length} 个入口
                </span>
              </div>
              <ResourceLocations
                rows={detailLocations}
                busy={!!busy}
                checks={mcpChecks}
                onOpen={(r) =>
                  void job("打开文件或目录", async () => {
                    await commands.revealResource(r.id);
                  })
                }
              />
              {selection.type === "skill" && (
                <p className="drawer-source">来源：{selection.item.source}</p>
              )}
            </section>
            <div className="detail-actions drawer-section">
              <h3>管理</h3>
              {selection.type === "resource" && detailLocations.length > 1 && (
                <label className="management-target">
                  操作对象
                  <select
                    value={selection.item.id}
                    disabled={!!busy}
                    onChange={(e) => {
                      const r = detailLocations.find(
                        (r) => r.id === e.target.value,
                      );
                      if (r) setSelection({ type: "resource", item: r });
                    }}
                  >
                    {detailLocations.map((r) => (
                      <option value={r.id} key={r.id}>
                        {shortPath(r.path)}
                      </option>
                    ))}
                  </select>
                </label>
              )}
              {selection.type === "resource" && (
                <section className="native-control-section">
                  <h4>原生启停配置</h4>
                  {selection.item.kind === "mcp" &&
                    selection.item.agents.includes("claude") && (
                      <label className="management-target">
                        Claude MCP 生效项目
                        <select
                          disabled={!!busy}
                          value={toggleProject}
                          onChange={(e) => {
                            const project = e.target.value;
                            void job("读取原生启停状态", async () => {
                              const controls = await commands.toggleStates(
                                project || null,
                              );
                              setNativeControls(controls);
                              setToggleProject(project);
                            });
                          }}
                        >
                          <option value="">
                            {selection.item.scope === "global"
                              ? "请选择项目"
                              : "使用此配置所属项目"}
                          </option>
                          {data?.projects.map((p) => (
                            <option key={p} value={p}>
                              {shortPath(p)}
                            </option>
                          ))}
                        </select>
                      </label>
                    )}
                  {nativeControls
                    .filter((c) => c.resource_id === selection.item.id)
                    .map((c) => (
                      <div
                        className="native-control-detail"
                        key={`${c.resource_id}:${c.agent}`}
                      >
                        <NativeSwitch
                          control={c}
                          busy={!!busy}
                          onToggle={toggleResource}
                        />
                        <p>{c.reason}</p>
                        {c.path && <small>{shortPath(c.path)}</small>}
                      </div>
                    ))}
                  <p className="native-control-hint">
                    显示原生配置状态，不代表会话已加载。切换后先预览，应用成功才更新状态。
                  </p>
                </section>
              )}
              {currentSkill && (
                <p className="drawer-source">
                  管理版本 {currentSkill.revision.slice(0, 16)}
                  {currentSkill.pinned ? " · 已固定" : ""}
                </p>
              )}

              {currentSkill && (
                <>
                  <button
                    className="button primary"
                    disabled={!!busy}
                    onClick={() => setModal("deploy")}
                  >
                    <ArrowLeftRight size={15} />
                    分配到 Agent
                  </button>
                  <button
                    className="button"
                    disabled={!!busy || currentSkill.pinned}
                    onClick={() =>
                      void job("获取上游并比较", () =>
                        preview(commands.checkUpdate(currentSkill.id)),
                      )
                    }
                  >
                    <RefreshCw size={15} />
                    检查更新并预览
                  </button>
                  <button
                    className="button"
                    disabled={!!busy}
                    onClick={() =>
                      void job("更新版本策略", async () => {
                        await commands.pinSkill(
                          currentSkill.id,
                          !currentSkill.pinned,
                        );
                        await load();
                      })
                    }
                  >
                    <Pin size={15} />
                    {currentSkill.pinned ? "解除固定版本" : "固定当前版本"}
                  </button>
                </>
              )}
              {selection.type === "resource" &&
                selection.item.kind === "skill" &&
                !currentSkill && (
                  <button
                    className="button primary"
                    disabled={!!busy}
                    onClick={() =>
                      void job("生成导入预览", () =>
                        preview(
                          commands.importSkill(selection.item.canonical, ""),
                        ),
                      )
                    }
                  >
                    <Plus size={15} />
                    导入管理副本
                  </button>
                )}
              {selection.type === "resource" &&
                selection.item.kind === "mcp" && (
                  <>
                    <button
                      className="button primary"
                      disabled={!!busy}
                      onClick={() => setModal("check")}
                    >
                      {checkFor(selection.item, mcpChecks)?.state ===
                      "checking" ? (
                        <LoaderCircle size={15} className="spin" />
                      ) : (
                        <Activity size={15} />
                      )}
                      {checkFor(selection.item, mcpChecks)?.state === "checking"
                        ? "检查中…"
                        : checkFor(selection.item, mcpChecks)
                          ? "重新检查"
                          : "检查连接"}
                    </button>
                    {checkFor(selection.item, mcpChecks) &&
                      (() => {
                        const result = checkFor(selection.item, mcpChecks)!;
                        return (
                          <div
                            ref={checkResultRef}
                            className={`mcp-check-result ${result.state}`}
                            role="status"
                            aria-live="polite"
                          >
                            <b>
                              {result.state === "checking"
                                ? "正在检查所选配置"
                                : result.state === "success"
                                  ? "初始化检查通过"
                                  : "检查未通过"}
                            </b>
                            <p>{result.message}</p>
                            {result.checkedAt && (
                              <small>
                                {result.checkedAt} · 仅本次应用会话有效
                              </small>
                            )}
                            <small>
                              {selection.item.agents
                                .map((a) => names[a])
                                .join("、")}{" "}
                              · {shortPath(selection.item.path)}
                            </small>
                          </div>
                        );
                      })()}
                    <button
                      className="button"
                      disabled={!!busy}
                      onClick={() => setModal("mcp")}
                    >
                      <Settings2 size={15} />
                      编辑 / 分配配置
                    </button>
                  </>
                )}
              {selection.type === "resource" &&
              selection.item.kind === "plugin" ? (
                selection.item.agents.map((a) => (
                  <button
                    key={a}
                    className="button"
                    onClick={() =>
                      void job("打开原生管理文档", async () => {
                        await commands.pluginGuide(a);
                      })
                    }
                  >
                    <ArrowUpRight size={15} />
                    {names[a]} 管理入口
                  </button>
                ))
              ) : selection.type === "resource" &&
                selection.item.kind === "skill" &&
                (selection.item.is_link ||
                  selection.item.path !== selection.item.canonical) ? (
                <p className="native-control-hint" role="note">
                  此位置是链接或映射入口，应用内不提供删除。请通过系统文件管理器移除此链接；删除源目录会影响其他入口。
                </p>
              ) : (
                <button
                  className="button danger"
                  disabled={!!busy}
                  onClick={() =>
                    void job("生成移除预览", () =>
                      selection.type === "skill"
                        ? preview(commands.deleteSkill(selection.item.id))
                        : preview(commands.removeResource(selection.item.id)),
                    )
                  }
                >
                  <Trash2 size={15} />
                  {selection.type === "skill"
                    ? "删除管理版本"
                    : selection.item.kind === "mcp"
                      ? "删除此 MCP 配置"
                      : "删除此 Skill 目录"}
                </button>
              )}
            </div>
            {selection.type === "resource" && (
              <details className="drawer-advanced">
                <summary>高级信息</summary>
                <dl>
                  <dt>内容指纹</dt>
                  <dd className="mono">
                    {selection.item.fingerprint.slice(0, 16)}
                  </dd>
                  <dt>扫描说明</dt>
                  <dd>{selection.item.detail}</dd>
                </dl>
                {selection.item.changed && (
                  <p className="warning-text">与上次盘点内容不同</p>
                )}
              </details>
            )}
            <div className="detail-note">
              <ShieldCheck size={15} />
              <p>
                应用前可查看影响范围与差异。备份保留在本机，后续修改会阻止直接撤销。
              </p>
            </div>
          </aside>
        </>
      )}
      {modal && (
        <ActionModal
          modal={modal}
          data={data}
          selection={selection}
          skill={currentSkill}
          busy={!!busy}
          onClose={() => setModal(null)}
          onSubmit={(values) =>
            void job("处理请求", async () => {
              if (modal === "project") {
                await commands.addProject(values.path);
                setModal(null);
                await load();
              }
              if (modal === "import")
                await preview(
                  commands.importSkill(values.source, values.subdir),
                );
              if (modal === "deploy") {
                if (!currentSkill)
                  throw new Error("请先选择由 Agent Hub 管理的 Skill");
                await preview(
                  commands.deploySkill(currentSkill.id, values.targets),
                );
              }
              if (modal === "mcp")
                await preview(
                  commands.previewMcp(
                    values.name,
                    values.config,
                    values.targets,
                  ),
                );
              if (modal === "export") {
                const path = await commands.exportLibrary(values.path);
                setModal(null);
                setMessage(`已导出到 ${path}`);
              }
              if (modal === "restore")
                await preview(
                  commands.importLibrary(values.path, values.overwrite),
                );
              if (modal === "check" && selection?.type === "resource") {
                setModal(null);
                const resource = selection.item;
                const update = (result: McpCheck) =>
                  setMcpChecks((old) => ({ ...old, [resource.id]: result }));
                update({
                  state: "checking",
                  message:
                    "最多等待 12 秒；仅检查 MCP 初始化，不调用业务工具。",
                  checkedAt: "",
                  fingerprint: resource.fingerprint,
                });
                try {
                  const message = await commands.checkMcp(
                    resource.id,
                    resource.fingerprint,
                  );
                  update({
                    state: "success",
                    message,
                    checkedAt: new Date().toLocaleString("zh-CN"),
                    fingerprint: resource.fingerprint,
                  });
                } catch (error) {
                  update({
                    state: "error",
                    message: String(error),
                    checkedAt: new Date().toLocaleString("zh-CN"),
                    fingerprint: resource.fingerprint,
                  });
                }
              }
            })
          }
        />
      )}
      {plan && (
        <div className="modal-backdrop">
          <section
            className="modal plan-modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="plan-title"
          >
            <div className="modal-heading">
              <div>
                <div className="eyebrow">REVIEW BEFORE APPLY</div>
                <h2 id="plan-title">{plan.title}</h2>
              </div>
              <button
                className="icon-button"
                disabled={!!busy}
                title="关闭预览"
                onClick={() => setPlan(null)}
              >
                <X size={20} />
              </button>
            </div>
            <div className="modal-body">
              <div className="plan-summary">
                <ShieldCheck size={18} />
                <p>
                  {plan.changes.length}{" "}
                  个目标将发生变化。应用前再次检查文件，并保存私有备份。
                </p>
              </div>
              {plan.notes.map((n, i) => (
                <p className="plan-note" key={i}>
                  {n}
                </p>
              ))}
              {plan.changes.map((c) => (
                <div className="diff" key={c.path}>
                  <div>
                    <FileCode2 size={14} />
                    <code>{c.path}</code>
                  </div>
                  <pre>{c.diff}</pre>
                </div>
              ))}
            </div>
            <div className="modal-footer">
              <button
                className="button"
                disabled={!!busy}
                onClick={() => setPlan(null)}
              >
                取消
              </button>
              <button
                className="button primary"
                disabled={!!busy}
                onClick={() =>
                  void job("应用变更", async () => {
                    let result: History;
                    try {
                      result = await commands.applyPlan(plan.id);
                    } catch (e) {
                      setPlan(null);
                      throw e;
                    }
                    setPlan(null);
                    await load(true);
                    setMessage(
                      `已应用「${result.title}」。请在目标 Agent 中确认加载状态。`,
                    );
                  })
                }
              >
                {busy ? (
                  <LoaderCircle className="spin" size={16} />
                ) : (
                  <Check size={16} />
                )}
                确认应用
              </button>
            </div>
          </section>
        </div>
      )}
    </div>
  );
}

function ResourceLocations({
  rows,
  busy,
  onOpen,
  checks,
}: {
  checks: Record<string, McpCheck>;
  rows: Resource[];
  busy: boolean;
  onOpen: (r: Resource) => void;
}) {
  if (rows[0] && rows[0].kind !== "skill")
    return (
      <div className="location-groups">
        {rows.map((r) => (
          <section className="location-group" key={r.id}>
            <button
              className="location-primary"
              disabled={busy}
              onClick={() => onOpen(r)}
              title="用系统默认应用打开此文件"
            >
              <FileCode2 size={17} />
              <span>
                <b>{r.agents.map((a) => names[a]).join("、")}</b>
                <small>
                  {r.scope === "global"
                    ? "用户级"
                    : `项目：${shortPath(r.scope)}`}
                </small>
                <small>
                  {r.kind === "mcp"
                    ? checkDescription(r, checks)
                    : r.description}
                </small>
                <small>{shortPath(r.path)}</small>
              </span>
              <ArrowUpRight size={14} />
            </button>
          </section>
        ))}
      </div>
    );
  const groups = new Map<string, Resource[]>();
  for (const r of rows)
    groups.set(r.canonical, [...(groups.get(r.canonical) ?? []), r]);
  return (
    <div className="location-groups">
      {[...groups].map(([canonical, entries]) => {
        const root = entries.find((r) => r.path === canonical) ?? entries[0];
        const links = entries.filter((r) => r.path !== canonical);
        return (
          <section className="location-group" key={canonical}>
            <button
              className="location-primary"
              disabled={busy}
              onClick={() => onOpen(root)}
              title="在 Finder 中打开目录"
            >
              <Folder size={17} />
              <span>
                <b>
                  {shortPath(
                    root.kind === "skill"
                      ? canonical
                      : canonical.slice(0, canonical.lastIndexOf("/")),
                  )}
                </b>
                <small>
                  {root.kind === "skill"
                    ? "实际目录"
                    : `文件：${canonical.split("/").pop()}`}
                  {groups.size > 1 ? " · 独立副本" : ""}
                </small>
                {entries.some((r) => r.path === canonical) && (
                  <small>
                    {root.scope === "global" ? "用户级" : "项目级"} ·{" "}
                    {root.agents.map((a) => names[a]).join("、")}
                  </small>
                )}
              </span>
              <ArrowUpRight size={14} />
            </button>
            {links.length > 0 && (
              <div className="location-links">
                <span className="location-links-title">链接与映射入口</span>
                {links.map((r) => (
                  <button
                    disabled={busy}
                    key={r.id}
                    onClick={() => onOpen(r)}
                    title={`打开实际目录：${shortPath(canonical)}`}
                  >
                    <span aria-hidden="true">↳</span>
                    <span>
                      <b>{shortPath(r.path)}</b>
                      <small>
                        {r.is_link ? "符号链接" : "路径映射"} ·{" "}
                        {r.scope === "global" ? "用户级" : "项目级"} ·{" "}
                        {r.agents.map((a) => names[a]).join("、")}
                      </small>
                    </span>
                    <ArrowUpRight size={12} />
                  </button>
                ))}
              </div>
            )}
          </section>
        );
      })}
    </div>
  );
}

function SkillInventory({
  rows,
  controls,
  agentFilter,
  onToggle,
  busy,
  library,
  onResource,
  onSkill,
  limit,
}: {
  rows: Resource[];
  controls: NativeControl[];
  agentFilter: string;
  onToggle: (c: NativeControl) => void;
  busy: boolean;
  library: Skill[];
  onResource: (resource: Resource) => void;
  onSkill: (skill: Skill) => void;
  limit?: number;
}) {
  const [filter, setFilter] = useState("all");
  const groups = new Map<string, { locations: Resource[]; managed: Skill[] }>();
  for (const r of rows) {
    if (r.kind !== "skill") continue;
    if (!groups.has(r.name)) groups.set(r.name, { locations: [], managed: [] });
    groups.get(r.name)!.locations.push(r);
  }
  for (const s of library) {
    if (!groups.has(s.name)) groups.set(s.name, { locations: [], managed: [] });
    groups.get(s.name)!.managed.push(s);
  }
  const visible = [...groups.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .filter(([, g]) =>
      filter === "managed"
        ? g.managed.length > 0
        : filter === "different"
          ? new Set(g.locations.map((r) => r.fingerprint)).size > 1
          : true,
    );
  return (
    <div className="skill-inventory">
      <div className="skill-filters" aria-label="Skill 视图筛选">
        {[
          ["all", "全部"],
          ["managed", "由 Agent Hub 管理"],
          ["different", "目录内容有差异"],
        ].map(([value, label]) => (
          <button
            key={value}
            className={filter === value ? "selected" : ""}
            aria-pressed={filter === value}
            onClick={() => setFilter(value)}
          >
            {label}
          </button>
        ))}
        <span>{visible.length} 组 Skill</span>
      </div>
      {visible.slice(0, limit).map(([name, g]) => {
        const agents = [...new Set(g.locations.flatMap((r) => r.agents))];
        const realPaths = new Set(g.locations.map((r) => r.canonical));
        const contents = new Set(g.locations.map((r) => r.fingerprint));
        return (
          <ResourceGroupCard
            controls={nativeControlsFor(controls, g.locations, agentFilter)}
            onToggle={onToggle}
            busy={busy}
            key={name}
            name={name}
            kind="skill"
            agents={agents}
            subtitle={`${g.locations.length} 个发现位置 · ${g.managed.length} 个管理版本`}
            badge={
              contents.size > 1
                ? "目录内容不同"
                : realPaths.size > 1
                  ? "独立目录 · 内容相同"
                  : g.locations.length > 1
                    ? "多个入口 · 同一目录"
                    : g.locations.length
                      ? "单个目录"
                      : "仅由 Agent Hub 管理"
            }
            warning={contents.size > 1}
            onClick={() =>
              filter === "managed" && g.managed[0]
                ? onSkill(g.managed[0])
                : g.locations[0]
                  ? onResource(g.locations[0])
                  : onSkill(g.managed[0])
            }
          />
        );
      })}
      {limit && visible.length > limit && (
        <p className="inventory-hint">
          展示前 {limit} 组，在 Skills 页面查看全部。
        </p>
      )}
      {!visible.length && (
        <Empty
          title={
            filter === "managed"
              ? "还没有由 Agent Hub 管理的 Skill"
              : "没有符合条件的 Skill"
          }
          text={
            filter === "managed"
              ? "导入本地或 Git Skill，预览并应用后即可纳入管理。"
              : "调整筛选、重新扫描，或导入 Skill。"
          }
        />
      )}
    </div>
  );
}

function nativeControlsFor(
  controls: NativeControl[],
  resources: Resource[],
  agent: string,
) {
  const ids = new Set(resources.map((r) => r.id));
  const unique = new Map<string, NativeControl>();
  for (const c of controls.filter(
    (c) => ids.has(c.resource_id) && (agent === "all" || c.agent === agent),
  )) {
    const key =
      c.path && c.selector
        ? `${c.agent}:${c.path}:${c.selector}:${c.context}`
        : `${c.agent}:${c.resource_id}`;
    unique.set(key, c);
  }
  return [...unique.values()];
}
function NativeSwitch({
  control,
  busy,
  onToggle,
}: {
  control: NativeControl;
  busy: boolean;
  onToggle: (c: NativeControl) => void;
}) {
  return (
    <div className="native-switch-row">
      <span>{names[control.agent]}</span>
      {control.enabled === null ? (
        <span className="muted" title={control.reason}>
          {control.unavailable_label}
        </span>
      ) : (
        <>
          <span className="native-state">
            {control.enabled ? "配置已启用" : "配置已停用"}
          </span>
          <button
            type="button"
            role="switch"
            aria-checked={control.enabled}
            aria-label={`${control.enabled ? "停用" : "启用"} ${names[control.agent]} 的 ${control.selector} 配置`}
            disabled={busy}
            title={control.reason}
            className={`native-switch ${control.enabled ? "on" : ""}`}
            onClick={() => onToggle(control)}
          >
            <span />
          </button>
        </>
      )}
    </div>
  );
}

function ResourceGroupCard({
  controls,
  onToggle,
  busy,
  name,
  kind,
  subtitle,
  agents,
  badge,
  warning = false,
  selected = false,
  onClick,
}: {
  controls: NativeControl[];
  onToggle: (c: NativeControl) => void;
  busy: boolean;
  name: string;
  kind: string;
  subtitle: string;
  agents: string[];
  badge: string;
  warning?: boolean;
  selected?: boolean;
  onClick: () => void;
}) {
  const directControl =
    controls.length === 1 && controls[0].enabled !== null ? controls[0] : null;
  return (
    <div
      className={`skill-group resource-card-row ${directControl ? "has-native-switch" : ""} ${selected ? "selected" : ""}`}
    >
      <button
        className="resource-card-title"
        onClick={onClick}
        aria-label={`查看 ${name} 详情`}
      >
        <span className={`resource-icon ${kind}`}>
          {kind === "skill" ? (
            <Sparkles size={18} />
          ) : kind === "mcp" ? (
            <Plug size={18} />
          ) : (
            <Package size={18} />
          )}
        </span>
        <span className="skill-group-name">
          <b>{name}</b>
          <small>{subtitle}</small>
        </span>
      </button>
      {directControl && (
        <div className="group-native-control">
          <NativeSwitch
            control={directControl}
            busy={busy}
            onToggle={onToggle}
          />
        </div>
      )}
      <button
        className="resource-card-meta"
        onClick={onClick}
        aria-label={`查看 ${name} 的实例和详情`}
      >
        <span className="agent-stack">
          {agents.map((a) => (
            <span key={a} title={`${names[a]}：扫描到候选实例，加载状态未确认`}>
              <AgentMark id={a} small />
            </span>
          ))}
        </span>
        <span className={`badge ${warning ? "amber" : "neutral"}`}>
          {badge}
        </span>
        <ChevronRight size={16} />
      </button>
    </div>
  );
}

function ResourceGroups({
  rows,
  controls,
  agentFilter,
  onToggle,
  busy,
  selection,
  onSelect,
  resourceKind,
}: {
  rows: Resource[];
  controls: NativeControl[];
  agentFilter: string;
  onToggle: (c: NativeControl) => void;
  busy: boolean;
  selection: Selection | null;
  onSelect: (r: Resource) => void;
  resourceKind: string;
}) {
  const [filter, setFilter] = useState("all");
  const groups = new Map<string, Resource[]>();
  for (const r of rows) {
    const key = `${r.kind}:${r.name}`;
    groups.set(key, [...(groups.get(key) ?? []), r]);
  }
  const visible = [...groups.values()]
    .sort((a, b) => a[0].name.localeCompare(b[0].name))
    .filter(
      (g) => filter === "all" || new Set(g.map((r) => r.fingerprint)).size > 1,
    );
  const isMcp = resourceKind === "mcp";
  return (
    <div className="skill-inventory">
      <div className="skill-filters" aria-label="资源视图筛选">
        {[
          ["all", "全部"],
          ["different", isMcp ? "配置有差异" : "清单有差异"],
        ].map(([value, label]) => (
          <button
            key={value}
            className={filter === value ? "selected" : ""}
            aria-pressed={filter === value}
            onClick={() => setFilter(value)}
          >
            {label}
          </button>
        ))}
        <span>{visible.length} 组资源</span>
      </div>
      {visible.map((g) => {
        const first = g[0];
        const differs = new Set(g.map((r) => r.fingerprint)).size > 1;
        const agents = [...new Set(g.flatMap((r) => r.agents))];
        return (
          <ResourceGroupCard
            controls={nativeControlsFor(controls, g, agentFilter)}
            onToggle={onToggle}
            busy={busy}
            key={`${first.kind}:${first.name}`}
            name={first.name}
            kind={first.kind}
            agents={agents}
            subtitle={`${g.length} 个${first.kind === "mcp" ? "配置实例" : "清单位置"} · ${agents.length} 个 Agent`}
            badge={
              g.length === 1
                ? first.kind === "mcp"
                  ? "单个配置"
                  : "单个清单"
                : differs
                  ? first.kind === "mcp"
                    ? "配置不同"
                    : "清单不同"
                  : first.kind === "mcp"
                    ? "配置一致"
                    : "清单一致"
            }
            warning={differs}
            selected={
              selection?.type === "resource" &&
              g.some((r) => r.id === selection.item.id)
            }
            onClick={() => onSelect(first)}
          />
        );
      })}
      {!visible.length && (
        <Empty title="没有符合条件的资源" text="调整筛选或重新扫描。" />
      )}
    </div>
  );
}

function Empty({
  title,
  text,
  action,
}: {
  title: string;
  text: string;
  action?: () => void;
}) {
  return (
    <div className="empty-state">
      <span>
        <Layers3 size={27} />
      </span>
      <h3>{title}</h3>
      <p>{text}</p>
      {action && (
        <button className="button" onClick={action}>
          <Plus size={15} />
          导入 Skill
        </button>
      )}
    </div>
  );
}

interface FormValues {
  overwrite: boolean;
  path: string;
  source: string;
  subdir: string;
  name: string;
  config: string;
  targets: Target[];
}
function ActionModal({
  modal,
  data,
  selection,
  skill,
  busy,
  onClose,
  onSubmit,
}: {
  modal: Exclude<Modal, null>;
  data: Scan | null;
  selection: Selection | null;
  skill?: Skill;
  busy: boolean;
  onClose: () => void;
  onSubmit: (v: FormValues) => void;
}) {
  const [path, setPath] = useState("");
  const [overwrite, setOverwrite] = useState(false);
  const [source, setSource] = useState("");
  const [subdir, setSubdir] = useState("");
  const selectedResource =
    selection?.type === "resource" ? selection.item : null;
  const [name, setName] = useState(
    modal === "mcp" && selectedResource?.kind === "mcp"
      ? selectedResource.name
      : "",
  );
  const [config, setConfig] = useState(
    '{\n  "command": "npx",\n  "args": ["-y", "@modelcontextprotocol/server-filesystem", "/absolute/path"]\n}',
  );
  const [agents, setAgents] = useState<string[]>(
    modal === "mcp" && selectedResource?.kind === "mcp"
      ? selectedResource.agents
      : [],
  );
  const [project, setProject] = useState("");
  const [selecting, setSelecting] = useState(false);
  const [pathError, setPathError] = useState("");
  async function choose(purpose: PathPurpose) {
    setSelecting(true);
    setPathError("");
    try {
      const chosen = await commands.choosePath(purpose);
      if (chosen) {
        if (purpose === "source") {
          setSource(chosen);
          setSubdir("");
        } else setPath(chosen);
      }
    } catch (e) {
      setPathError(String(e));
    } finally {
      setSelecting(false);
    }
  }
  const titles: Record<string, string> = {
    import: "导入 Skill",
    deploy: `分配 ${skill?.name ?? "Skill"}`,
    mcp: "添加 / 编辑 MCP",
    project: "添加项目目录",
    export: "导出管理数据",
    restore: "导入 Agent Hub 管理数据备份",
    check: "检查 MCP 连接",
  };
  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    onSubmit({
      overwrite,
      path,
      source,
      subdir,
      name,
      config,
      targets: agents.map((agent) => ({ agent, project: project || null })),
    });
  };
  const targets = modal === "deploy" || modal === "mcp";
  return (
    <div className="modal-backdrop">
      <form
        className="modal"
        onSubmit={submit}
        role="dialog"
        aria-modal="true"
        aria-labelledby="action-title"
      >
        <div className="modal-heading">
          <div>
            <div className="eyebrow">AGENT HUB</div>
            <h2 id="action-title">{titles[modal]}</h2>
          </div>
          <button
            className="icon-button"
            type="button"
            disabled={busy}
            title="关闭对话框"
            onClick={onClose}
          >
            <X size={20} />
          </button>
        </div>
        <div className="modal-body">
          {pathError && (
            <p role="alert" className="warning-text">
              {pathError}
            </p>
          )}
          {modal === "import" && (
            <>
              <label className="field">
                HTTPS Git 仓库，或选择本地目录
                <input
                  autoFocus
                  required
                  value={source}
                  onChange={(e) => setSource(e.target.value)}
                  placeholder="https://github.com/org/repo"
                />
              </label>
              <button
                type="button"
                className="button picker-button"
                disabled={busy || selecting}
                onClick={() => void choose("source")}
              >
                <Folder size={15} />
                {selecting ? "等待目录选择…" : "选择本地 Skill 目录"}
              </button>
              <label className="field">
                仓库内 Skill 目录 <small>可留空</small>
                <input
                  value={subdir}
                  onChange={(e) => setSubdir(e.target.value)}
                  placeholder="skills/my-skill"
                />
              </label>
              <p className="help">
                所选目录根部必须有 SKILL.md。不执行附带脚本；Git 导入只支持
                HTTPS，使用已有 Git 凭据配置。
              </p>
            </>
          )}
          {["project", "export", "restore"].includes(modal) && (
            <>
              <label className="field">
                {modal === "project" ? "项目绝对路径" : "备份文件绝对路径"}
                <input
                  autoFocus
                  required
                  value={path}
                  readOnly
                  placeholder={
                    modal === "project"
                      ? "请通过下方按钮选择项目目录"
                      : "请通过下方按钮选择备份文件"
                  }
                />
              </label>
              <button
                type="button"
                className="button picker-button"
                disabled={busy || selecting}
                onClick={() =>
                  void choose(
                    modal === "project"
                      ? "project"
                      : modal === "export"
                        ? "export"
                        : "import",
                  )
                }
              >
                <Folder size={15} />
                {selecting
                  ? "等待系统选择器…"
                  : modal === "project"
                    ? "选择项目目录"
                    : modal === "export"
                      ? "选择导出位置"
                      : "选择备份文件"}
              </button>
              <p className="help">
                {modal === "project"
                  ? "只扫描你添加的目录。资源读取不会执行该项目的任何脚本。"
                  : modal === "export"
                    ? "导出不会覆盖现有文件。只包含 Skill 库与分配参考，不导出 MCP 配置和凭据；请先检查资源内容。"
                    : "合并到现有 Skill 管理数据，默认跳过同来源资源；其他本机资源保留。导入前展示新增、覆盖、跳过摘要及差异。分配记录仅作参考。"}
              </p>
              {modal === "restore" && (
                <label className="field">
                  同来源资源处理
                  <select
                    value={overwrite ? "overwrite" : "skip"}
                    onChange={(e) =>
                      setOverwrite(e.target.value === "overwrite")
                    }
                  >
                    <option value="skip">保留本机版本，跳过冲突（默认）</option>
                    <option value="overwrite">
                      用备份覆盖同来源副本、版本及固定状态
                    </option>
                  </select>
                </label>
              )}
            </>
          )}
          {modal === "mcp" && (
            <>
              <label className="field">
                服务名称
                <input
                  required
                  autoFocus
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="filesystem"
                />
              </label>
              <label className="field">
                通用配置 JSON
                <textarea
                  required
                  spellCheck={false}
                  rows={8}
                  value={config}
                  onChange={(e) => setConfig(e.target.value)}
                />
              </label>
              <p className="help">
                支持 command / args 或
                url；第一版不新增密钥配置。已有同名项中的原生凭据与扩展字段会保留。OAuth
                请在所属 Agent 中登录。
              </p>
            </>
          )}
          {targets && (
            <>
              <fieldset>
                <legend>目标 Agent</legend>
                <div className="target-grid">
                  {Object.entries(names).map(([id, n]) => (
                    <label
                      key={id}
                      className={
                        agents.includes(id) ? "target selected" : "target"
                      }
                    >
                      <input
                        type="checkbox"
                        checked={agents.includes(id)}
                        onChange={() =>
                          setAgents((a) =>
                            a.includes(id)
                              ? a.filter((x) => x !== id)
                              : [...a, id],
                          )
                        }
                      />
                      <AgentMark id={id} small />
                      <span>{n}</span>
                      {agents.includes(id) && <Check size={14} />}
                    </label>
                  ))}
                </div>
              </fieldset>
              <label className="field">
                作用域
                <select
                  value={project}
                  onChange={(e) => setProject(e.target.value)}
                >
                  <option value="">全局</option>
                  {data?.projects.map((p) => (
                    <option key={p} value={p}>
                      {p}
                    </option>
                  ))}
                </select>
              </label>
              {modal === "deploy" && (
                <p className="help">
                  共享的 .agents/skills 可能被多个 Agent
                  同时读取，预览会说明影响范围。导入的 Skill
                  未经自动兼容性认证。
                </p>
              )}
            </>
          )}
          {modal === "check" && (
            <>
              <div className="plan-summary">
                <Activity size={20} />
                <p>
                  将检查 <b>{selectedResource?.name}</b> 的所选配置。
                </p>
              </div>
              <dl className="check-target">
                <dt>所属 Agent</dt>
                <dd>
                  {selectedResource?.agents.map((a) => names[a]).join("、")}
                </dd>
                <dt>配置位置</dt>
                <dd>
                  {selectedResource?.path && shortPath(selectedResource.path)}
                </dd>
                <dt>作用域</dt>
                <dd>
                  {selectedResource?.scope === "global"
                    ? "用户级"
                    : selectedResource?.scope}
                </dd>
              </dl>
              <p className="help">
                stdio 会启动配置中的命令及其依赖；HTTP
                会向服务器发送初始化请求。超时 12
                秒，仅检查协议初始化，不调用业务工具。OAuth 和特殊凭据 helper
                请通过原生 Agent 检查。
              </p>
            </>
          )}
        </div>
        <div className="modal-footer">
          <button
            type="button"
            className="button"
            disabled={busy}
            onClick={onClose}
          >
            取消
          </button>
          <button
            className="button primary"
            type="submit"
            disabled={busy || selecting || (targets && !agents.length)}
          >
            {busy ? (
              <LoaderCircle size={15} className="spin" />
            ) : modal === "check" ? (
              <Activity size={15} />
            ) : (
              <ChevronRight size={15} />
            )}
            {modal === "export"
              ? "导出文件"
              : modal === "project"
                ? "添加并扫描"
                : modal === "check"
                  ? "开始检查"
                  : "查看变更预览"}
          </button>
        </div>
      </form>
    </div>
  );
}

createRoot(document.getElementById("root")!).render(<App />);
