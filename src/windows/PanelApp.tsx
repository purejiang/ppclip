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
  const searchRef = useRef<HTMLInputElement>(null);

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
  // oxlint-disable-next-line react/set-state-in-effect
  useEffect(() => {
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
        const clip = clips[active];
        if (clip) void api.copyClip(clip.id);
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

  return (
    <div className="panel">
      <header className="panel-header" data-tauri-drag-region>
        <span className="panel-title">PPClip</span>
        <div className="panel-actions">
          <button
            type="button"
            className="icon-button"
            title="区域截图"
            onClick={() => void api.startRegionCapture()}
          >
            截
          </button>
          <button
            type="button"
            className="icon-button"
            title="全屏截图"
            onClick={() => void api.captureFullscreen()}
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
              onClick={(item) => void api.copyClip(item.id)}
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
