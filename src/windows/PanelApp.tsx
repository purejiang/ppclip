import { useCallback, useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { api, dataDir } from "../api";
import ClipRow from "../components/ClipRow";
import SettingsPage from "../components/SettingsPage";
import {
  IconArrowBack,
  IconBroom,
  IconClipboardArrow,
  IconGear,
  IconMonitor,
  IconScan,
  IconSearch,
} from "../components/icons";
import type { Clip, ClipStats, KindFilter } from "../types";

/** 筛选：稿子里是 All / Text / Images / Files，我们只有文本与图片两类 */
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

/**
 * 历史面板（照 stitch 稿）：
 *
 * 48px 居中品牌头（左占位 + PPCLIP + 右上设置）→ 搜索 + 胶囊筛选（带闪电开关）
 * → 扁平玻璃列表 → 状态条（脉冲点 + n / 上限 + 截图 / 清空）→ 24px 页脚（Esc 关闭）。
 * 设置是同一扇窗里的第二屏（标题栏换成「← + 剪贴板设置」），所以 Esc 是返回而不是关窗。
 */
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
  /** 已经贴在桌面上的条目 id（图钉按钮据此点亮 / 变成取消贴图） */
  const [pinnedIds, setPinnedIds] = useState<number[]>([]);
  /** 历史条目上限，状态条显示 `n / 上限` */
  const [maxItems, setMaxItems] = useState(300);
  const [clearing, setClearing] = useState(false);
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

  /** 贴到桌面 / 取消贴图（稿子行里的图钉是 toggle 语义） */
  const handlePin = async (clip: Clip) => {
    try {
      setError(null);
      const pins = await api.listPins();
      const existing = pins.find((pin) => pin.clipId === clip.id);
      if (existing) await api.deletePin(existing.id);
      else await api.createPin(clip.id);
      setPinnedIds((await api.listPins()).map((pin) => pin.clipId));
    } catch (err) {
      setError(`贴图操作失败：${String(err)}`);
    }
  };

  const refresh = useCallback(async () => {
    try {
      const [list, nextStats, settings, pins] = await Promise.all([
        api.listClips(debounced, kind),
        api.clipStats(),
        api.getSettings(),
        api.listPins().catch(() => []),
      ]);
      setClips(list);
      setStats(nextStats);
      setMaxItems(settings.maxItems);
      setPinnedIds(pins.map((pin) => pin.clipId));
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
    const clipsChanged = api.onClipsChanged(() => void refresh());
    const pinsChanged = api.onPinsChanged(() => void refresh());
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

  // 拖动后回写位置（节流），下次唤起还记得停在哪
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
      if (settingsOpen) return;
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
  }, [active, clips, settingsOpen]);

  /** 单击一条：看完整内容 */
  const handleRowClick = (clip: Clip) => {
    void api.openViewer(clip.id);
  };

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

  const clearAll = async () => {
    setClearing(true);
    try {
      await api.clearClips("all");
      await refresh();
    } catch (err) {
      setError(`清空失败：${String(err)}`);
    } finally {
      setClearing(false);
    }
  };

  return (
    <div className="app-card">
      <header
        className="app-header"
        data-tauri-drag-region="deep"
        onDoubleClick={(e) => e.stopPropagation()}
      >
        <div className="app-header-side">
          {settingsOpen && (
            <button
              type="button"
              className="app-header-button"
              title="返回历史"
              aria-label="返回历史"
              onClick={() => setSettingsOpen(false)}
            >
              <IconArrowBack size={16} />
            </button>
          )}
        </div>

        <div className="app-brand">
          {settingsOpen ? <IconGear size={16} /> : <IconClipboardArrow size={16} />}
          {settingsOpen ? "剪贴板设置" : "PPCLIP"}
        </div>

        <div className="app-header-side">
          {!settingsOpen && (
            <button
              type="button"
              className="app-header-button app-header-button--primary"
              title="设置"
              aria-label="设置"
              onClick={() => setSettingsOpen(true)}
            >
              <IconGear size={16} />
            </button>
          )}
        </div>
      </header>

      <main className="app-main">
        <span className="app-glow app-glow--primary" aria-hidden="true" />
        <span className="app-glow app-glow--tertiary" aria-hidden="true" />

        {settingsOpen ? (
          <SettingsPage
            onChanged={() => {
              void refresh();
            }}
          />
        ) : (
          <>
            <div className="panel-search">
              <label className="search-wrap">
                <IconSearch size={16} />
                <input
                  ref={searchRef}
                  className="search-input"
                  placeholder="搜索剪贴板历史…"
                  aria-label="搜索剪贴板历史"
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                />
              </label>
                <div className="panel-filters">
                  <div className="filter-pills">
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
                    pinned={pinnedIds.includes(clip.id)}
                    onClick={handleRowClick}
                    onCopy={(item) => void api.copyClip(item.id)}
                    onPin={(item) => void handlePin(item)}
                    onDelete={handleDelete}
                    onContextMenu={(item) => void api.showClipMenu(item.id)}
                  />
                ))
              )}
            </div>

            <div className="panel-status">
              <div className="panel-status-group">
                <span className="panel-status-dot" aria-hidden="true" />
                <span className="panel-status-count">{stats?.total ?? 0}</span>
                <span>/ {maxItems} 条上限</span>
              </div>
              <div className="panel-status-tools">
                <button
                  type="button"
                  className="status-button"
                  title="区域截图"
                  aria-label="区域截图"
                  onClick={() => void runCapture(api.startRegionCapture)}
                >
                  <IconScan size={14} />
                </button>
                <button
                  type="button"
                  className="status-button"
                  title="全屏截图"
                  aria-label="全屏截图"
                  onClick={() => void runCapture(api.captureFullscreen)}
                >
                  <IconMonitor size={14} />
                </button>
                <button
                  type="button"
                  className="status-button status-button--danger"
                  title="清空全部历史"
                  aria-label="清空全部历史"
                  disabled={clearing}
                  onClick={() => void clearAll()}
                >
                  <IconBroom size={14} />
                </button>
              </div>
            </div>
          </>
        )}
      </main>

      {error && <div className="panel-error">{error}</div>}

      <footer className="app-footer">
        <div className="app-footer-group">
          <span>PPClip v0.1</span>
          <span className="app-footer-dot" aria-hidden="true" />
          <span>
            {stats
              ? `${stats.textCount} 文本 · ${stats.imageCount} 图片 · ${formatBytes(stats.imageBytes)}`
              : "Ready"}
          </span>
        </div>
        <div className="app-footer-group">
          <span className="kbd">Esc</span>
          <span>{settingsOpen ? "返回" : "关闭"}</span>
        </div>
      </footer>
    </div>
  );
}
