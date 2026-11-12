import { useCallback, useEffect, useRef, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { openUrl } from "@tauri-apps/plugin-opener";

export function AppUpdater({
  expanded,
  busy,
  runInstall,
}: {
  expanded: boolean;
  busy: boolean;
  runInstall: (action: () => Promise<void>) => Promise<void>;
}) {
  const update = useRef<Update | null>(null);
  const [version, setVersion] = useState("");
  const [notes, setNotes] = useState("");
  const [dismissed, setDismissed] = useState(false);
  const [phase, setPhase] = useState<
    | "idle"
    | "checking"
    | "available"
    | "downloading"
    | "ready"
    | "installing"
    | "restart"
    | "error"
  >("idle");
  const [message, setMessage] = useState("");
  const [progress, setProgress] = useState<number | null>(null);
  const checking = useRef(false);
  const mounted = useRef(true);
  const checkUpdate = useCallback(async () => {
    if (!isTauri() || checking.current) return;
    checking.current = true;
    setDismissed(false);
    setPhase("checking");
    setMessage("");
    try {
      const next = await check({ timeout: 15000 });
      if (!mounted.current) {
        await next?.close();
        return;
      }
      await update.current?.close();
      update.current = next;
      setVersion(next?.version ?? "");
      setNotes(next?.body ?? "");
      setPhase(next ? "available" : "idle");
      setMessage(
        next
          ? "下载后将校验安装包签名，安装前由你确认。"
          : "当前已是最新版本。",
      );
    } catch {
      if (mounted.current) {
        setPhase("error");
        setMessage(
          "暂时无法检查更新，请检查网络后重试，或前往 GitHub Releases 下载。",
        );
      }
    } finally {
      checking.current = false;
    }
  }, []);
  useEffect(() => {
    mounted.current = true;
    const timer = window.setTimeout(() => void checkUpdate(), 3000);
    return () => {
      mounted.current = false;
      window.clearTimeout(timer);
      void update.current?.close();
    };
  }, [checkUpdate]);

  async function download() {
    const candidate = update.current;
    if (!candidate) return;
    setPhase("downloading");
    setMessage("正在下载安装包…");
    let received = 0;
    let total = 0;
    try {
      await candidate.download((event) => {
        if (event.event === "Started") {
          total = event.data.contentLength ?? 0;
          setProgress(null);
        }
        if (event.event === "Progress") {
          received += event.data.chunkLength;
          setProgress(
            total ? Math.min(100, Math.round((received / total) * 100)) : null,
          );
        }
      });
      setPhase("ready");
      setMessage("安装包已下载并校验签名。请保存工作后安装并重启。");
    } catch {
      setPhase("available");
      setMessage("下载或签名校验失败，请重试。当前版本仍可继续使用。");
    }
  }
  async function install() {
    if (!update.current || busy) return;
    setPhase("installing");
    setMessage("正在安装更新，应用将重新启动…");
    await runInstall(async () => {
      let installed = phase === "restart";
      try {
        if (!installed) {
          await update.current!.install();
          installed = true;
        }
        await relaunch();
      } catch {
        setPhase(installed ? "restart" : "ready");
        setMessage(
          installed
            ? "更新已安装，请点击重启应用。"
            : "安装未完成，请重试；也可前往 GitHub Releases 手动安装。",
        );
      }
    });
  }
  if (!expanded && (!version || dismissed)) return null;
  const locked =
    phase === "checking" || phase === "downloading" || phase === "installing";
  return (
    <section className="settings-section app-updater" aria-label="应用更新">
      <h2>{version ? `发现新版本 ${version}` : "应用更新"}</h2>
      <p role="status" aria-live="polite">
        {!isTauri()
          ? "请在桌面应用中检查更新。"
          : message || "启动后自动检查更新，安装前由你确认。"}
      </p>
      {notes && (
        <details>
          <summary>查看更新说明</summary>
          <p className="update-notes">{notes}</p>
        </details>
      )}
      {phase === "downloading" && (
        <progress
          aria-label="更新下载进度"
          max={100}
          value={progress ?? undefined}
        />
      )}
      <div className="button-row">
        <button
          className="button"
          disabled={
            !isTauri() ||
            busy ||
            locked ||
            phase === "ready" ||
            phase === "restart"
          }
          onClick={() => void checkUpdate()}
        >
          {phase === "checking" ? "正在检查…" : "检查更新"}
        </button>
        {phase === "available" && (
          <button
            className="button primary"
            disabled={busy}
            onClick={() => void download()}
          >
            下载更新
          </button>
        )}
        {(phase === "ready" || phase === "restart") && (
          <button
            className="button primary"
            disabled={busy}
            onClick={() => void install()}
          >
            {phase === "restart" ? "重启应用" : "安装并重启"}
          </button>
        )}
        {!expanded && !locked && (
          <button className="button" onClick={() => setDismissed(true)}>
            稍后更新
          </button>
        )}
        <button
          className="button"
          onClick={() =>
            void openUrl("https://github.com/xdffish/agent-hub/releases").catch(
              () =>
                setMessage(
                  "无法打开浏览器，请访问 github.com/xdffish/agent-hub/releases。",
                ),
            )
          }
        >
          查看发布记录
        </button>
      </div>
    </section>
  );
}
