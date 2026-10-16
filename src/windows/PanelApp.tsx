import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { api, dataDir } from "../api";
import ClipRow from "../components/ClipRow";
import SettingsSheet from "../components/SettingsSheet";
import type { Clip, ClipStats, KindFilter } from "../types";

const FILTERS: { key: KindFilter; label: string }[] = [
  { key: "all", label: "全部" },
  { key: "text", label: "文本" },
  { key: "image", label: "图片" },
];

function formatBytes(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  if (bytes >= 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${bytes} B`;
}

/** 历史面板：搜索、筛选、键盘导航、点击即复制 */
export default function PanelApp() {
  const [clips, setClips] = useState<Clip[]>([]);
  const [stats, setStats] = useState<ClipStats | null>(null);
  const [query, setQuery] = useState("");
  const [debounced, setDebounced] = useState("");
  const [kind, setKind] = useState<KindFilter>("all");
  const [active, setActive] = useState(0);
  const [dir, setDir] = useState("");
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  /** 每分鐘刷新一次，让「N 分钟前」跟着走 */
  const [now, setNow] = useState(() => Date.now());
  /** 正在等第二次点击确认删除的条目 id */
  const [pendingDelete, setPendingDelete] = useState<number | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 60_000);
    return () => window.clearInterval(timer);
  }, []);

  // 确认删除的状态 3 秒后自动撤销，别一直挂在那
  useEffect(() => {
    if (pendingDelete === null) return;
    const timer = window.setTimeout(() => setPendingDelete(null), 3000);
    return () => window.clearTimeout(timer);
  }, [pendingDelete]);

  /** 删除要点两次：第一次只是把按钮变成「确认删除」 */
  const handleDelete = (clip: Clip) => {
    if (pendingDelete !== clip.id) {
      setPendingDelete(clip.id);
      return;
    }
    setPendingDelete(null);
    void api.deleteClip(clip.id);
  };

  const refresh = useCallback(async () => {
    try {
      const [list, nextStats] = await Promise.all([
        api.listClips(debounced, kind),
        api.clipStats(),
      ]);
      setClips(list);
      setStats(nextStats);
      setActive((index) => Math.min(index, Math.max(0, list.length - 1)));
      setError(null);
    } catch (err) {
      setError(String(err));
    }
  }, [debounced, kind]);

  useEffect(() => {
    void dataDir().then(setDir);
  }, []);

  // 搜索防抖：避免每敲一个字都查一遍库
  useEffect(() => {
    const timer = window.setTimeout(() => setDebounced(query), 150);
    return () => window.clearTimeout(timer);
  }, [query]);

  // 挂载时拉一次数据；之后由 clips:changed / pins:changed 事件驱动刷新
  useEffect(() => {
    // oxlint-disable-next-line react/set-state-in-effect -- 面板是外部数据源，挂载即取
    void refresh();
  }, [refresh]);

  // 后端有新条目 / 贴图变化 → 刷新
  useEffect(() => {
    const clipsChanged = listen("clips:changed", () => void refresh());
    const pinsChanged = listen("pins:changed", () => void refresh());
    return () => {
      void clipsChanged.then((fn) => fn());
      void pinsChanged.then((fn) => fn());
    };
  }, [refresh]);

  // 面板每次被唤起（获得焦点）都刷新并聚焦搜索框
  useEffect(() => {
    const unlisten = getCurrentWindow().onFocusChanged(({ payload }) => {
      if (payload) {
        void refresh();
        searchRef.current?.focus();
      }
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [refresh]);

  // 拖动后回写位置（和悬浮球同款节流），下次唤起还记得停在哪
  useEffect(() => {
    const win = getCurrentWindow();
    let timer: number | undefined;

    const unlisten = win.onMoved(({ payload }) => {
      window.clearTimeout(timer);
      timer = window.setTimeout(() => {
        void api.savePanelPosition(Math.round(payload.x), Math.round(payload.y));
      }, 400);
    });

    return () => {
      window.clearTimeout(timer);
      void unlisten.then((fn) => fn());
    };
  }, []);

  // 键盘导航
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        if (settingsOpen) setSettingsOpen(false);
        else void api.hidePanel();
        return;
      }
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setActive((i) => Math.min(i + 1, clips.length - 1));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setActive((i) => Math.max(i - 1, 0));
      } else if (e.key === "Enter") {
        // Enter = 看完整内容（和单击一致）；Ctrl+Enter = 复制
        const clip = clips[active];
        if (!clip) return;
        if (e.ctrlKey) void api.copyClip(clip.id);
        else void api.openViewer(clip.id);
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [clips, active, settingsOpen]);

  // 选中项滚动进视野
  useEffect(() => {
    document
      .querySelector(".clip-row.is-active")
      ?.scrollIntoView({ block: "nearest" });
  }, [active, clips]);

  // 截图按钮：失败时窗口多半已经被藏起来了，把面板叫回来并把原因摆出来，
  // 否则用户看到的就是「点一下应用没了」。
  const runCapture = async (action: () => Promise<void>) => {
    try {
      setError(null);
      await action();
    } catch (err) {
      setError(`截图失败：${String(err)}`);
      void api.showPanel();
    }
  };

  return (
    <div className="panel">
      {/* deep：标题文字上也能拖；按钮是 clickable，会自己挡掉拖拽。
          双击不去最大化（面板是固定尺寸的，且双击容易被当成拖拽） */}
      <header
        className="panel-header"
        data-tauri-drag-region="deep"
        onDoubleClick={(e) => e.stopPropagation()}
      >
        <span className="panel-title">PPClip</span>
        <div className="panel-actions">
          <button
            type="button"
            className="icon-button"
            title="区域截图"
            onClick={() => void runCapture(api.startRegionCapture)}
          >
            截
          </button>
          <button
            type="button"
            className="icon-button"
            title="全屏截图"
            onClick={() => void runCapture(api.captureFullscreen)}
          >
            全
          </button>
          <button
            type="button"
            className="icon-button"
            title="设置"
            onClick={() => setSettingsOpen((open) => !open)}
          >
            ⚙
          </button>
          <button
            type="button"
            className="icon-button"
            title="隐藏（Esc）"
            onClick={() => void api.hidePanel()}
          >
            ✕
          </button>
        </div>
      </header>

      <div className="panel-search">
        <input
          ref={searchRef}
          className="search-input"
          placeholder="搜索剪贴板历史…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <div className="filters">
          {FILTERS.map((filter) => (
            <button
              key={filter.key}
              type="button"
              className={`filter${kind === filter.key ? " is-active" : ""}`}
              onClick={() => setKind(filter.key)}
            >
              {filter.label}
            </button>
          ))}
        </div>
      </div>

      <div className="clip-list">
        {clips.length === 0 ? (
          <p className="placeholder">
            {debounced ? "没有匹配的记录" : "还没有记录：复制一段文字或一张图试试"}
          </p>
        ) : (
          clips.map((clip, index) => (
            <ClipRow
              key={clip.id}
              clip={clip}
              dataDir={dir}
              active={index === active}
              now={now}
              confirming={pendingDelete === clip.id}
              onClick={(item) => void api.openViewer(item.id)}
              onCopy={(item) => void api.copyClip(item.id)}
              onDelete={handleDelete}
              onContextMenu={(item) => void api.showClipMenu(item.id)}
            />
          ))
        )}
      </div>

      <footer className="panel-footer">
        <span>
          {stats
            ? `${stats.total} 条 · 文本 ${stats.textCount} · 图片 ${stats.imageCount}`
            : ""}
        </span>
        <span>{stats ? `图片 ${formatBytes(stats.imageBytes)}` : ""}</span>
      </footer>

      {error && <div className="panel-error">{error}</div>}
      {settingsOpen && (
        <SettingsSheet
          onClose={() => setSettingsOpen(false)}
          onChanged={() => void refresh()}
        />
      )}
    </div>
  );
}
