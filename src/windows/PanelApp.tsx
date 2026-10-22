import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
} from "react";
import { getVersion } from "@tauri-apps/api/app";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { api, dataDir } from "../api";
import AboutPage from "../components/AboutPage";
import ClipRow from "../components/ClipRow";
import SettingsPage from "../components/SettingsPage";
import {
  IconArrowBack,
  IconClipboardArrow,
  IconClose,
  IconEraser,
  IconGear,
  IconKeepOnTop,
  IconMonitor,
  IconScan,
  IconSearch,
} from "../components/icons";
import type { Clip, ClipStats, KindFilter, Pin } from "../types";

/** 筛选：稿子里是 All / Text / Images / Files，四种都有对应条目类型 */
const FILTERS: { key: KindFilter; label: string }[] = [
  { key: "all", label: "全部" },
  { key: "text", label: "文本" },
  { key: "image", label: "图片" },
  { key: "file", label: "文件" },
];

function formatBytes(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  if (bytes >= 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${bytes} B`;
}

/**
 * 历史面板（照 stitch 稿）：
 *
 * 48px 居中品牌头（左上设置 + PPCLIP + 右上 桌面置顶 / 关闭）
 * → 搜索 + 胶囊筛选 → 扁平玻璃列表 → 状态条（脉冲点 + n / 上限 + 截图 / 清空）
 * → 24px 页脚（Esc 关闭）。设置是同一扇窗里的第二屏（标题栏换成「← 返回 + 剪贴板设置」），
 * 关闭按钮始终在最右侧，Esc 在设置屏只回上一层。
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
  /** 关于 / 版本页（弹层，覆盖设置屏或列表屏） */
  const [aboutOpen, setAboutOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  /** 每分鐘刷新一次，让「N 分钟前」跟着走 */
  const [now, setNow] = useState(() => Date.now());
  /** 正在等第二次点击确认删除的条目 id */
  const [pendingDelete, setPendingDelete] = useState<number | null>(null);
  /** 已经贴在桌面上的条目 id（浮窗按钮据此点亮 / 变成取消贴图） */
  const [pinnedIds, setPinnedIds] = useState<number[]>([]);
  /** 历史条目上限，状态条显示 `n / 上限` */
  const [maxItems, setMaxItems] = useState(300);
  /** 面板是否「桌面置顶」（常驻最上层 + 点外部不关） */
  const [pinned, setPinned] = useState(false);
  /** 面板不透明度（%）：只调玻璃卡片底色，100 = 完全不透明，见 styles 里的 `--panel-opacity` */
  const [panelOpacity, setPanelOpacity] = useState(100);
  const [clearing, setClearing] = useState(false);
  /** 清空全部历史：第一次只进入确认态，确认后才真的删 */
  const [clearConfirm, setClearConfirm] = useState(false);
  /** 清空成功后的结果提示（role="status"） */
  const [clearMessage, setClearMessage] = useState<string | null>(null);
  /** 贴图列表是否成功读到：读不到时确认文案不报「0 个贴图」 */
  const [pinsLoaded, setPinsLoaded] = useState(false);
  /** 应用版本（页脚显示），从 tauri.conf.json 的 version 取，别再手写死 */
  const [version, setVersion] = useState("");
  const searchRef = useRef<HTMLInputElement>(null);
  const clipListRef = useRef<HTMLDivElement>(null);
  const eraserRef = useRef<HTMLButtonElement>(null);
  const clearCancelRef = useRef<HTMLButtonElement>(null);
  const clearConfirmButtonRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 60_000);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    getVersion().then(setVersion).catch(() => {});
  }, []);

  // 确认删除的状态 3 秒后自动撤销，别一直挂在那
  useEffect(() => {
    if (pendingDelete === null) return;
    const timer = window.setTimeout(() => setPendingDelete(null), 3000);
    return () => window.clearTimeout(timer);
  }, [pendingDelete]);

  // 清空成功的提示 2.4 秒后自动收起
  useEffect(() => {
    if (!clearMessage) return;
    const timer = window.setTimeout(() => setClearMessage(null), 2400);
    return () => window.clearTimeout(timer);
  }, [clearMessage]);

  /** 删除要点两次：第一次只是把按钮变成「确认删除」 */
  const handleDelete = (clip: Clip) => {
    if (pendingDelete !== clip.id) {
      setPendingDelete(clip.id);
      return;
    }
    setPendingDelete(null);
    void api.deleteClip(clip.id);
  };

  /** 贴到桌面 / 取消贴图（行里那个按钮是 toggle 语义） */
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
      const [list, nextStats, settings, pinsResult] = await Promise.all([
        api.listClips(debounced, kind),
        api.clipStats(),
        api.getSettings(),
        // 读不到贴图列表时保留「未知」，别在确认文案里谎报 0 个贴图
        api
          .listPins()
          .then((pins) => ({ ok: true, pins }))
          .catch(() => ({ ok: false, pins: [] as Pin[] })),
      ]);
      setClips(list);
      setStats(nextStats);
      setMaxItems(settings.maxItems);
      setPinned(settings.panelPinned);
      // ?? 100：后端比前端旧时（dev 热更、半新半旧的组合）字段可能是 undefined，
      // 直接算会得到 NaN 的 CSS 变量，把玻璃底色整个算废
      setPanelOpacity(settings.panelOpacity ?? 100);
      setPinnedIds(pinsResult.pins.map((pin) => pin.clipId));
      setPinsLoaded(pinsResult.ok);
      setActive((index) => Math.min(index, Math.max(0, list.length - 1)));
      setError(null);
    } catch (err) {
      setError(String(err));
    }
  }, [debounced, kind]);

  /** 第一次点橡皮擦只进入确认态，不碰数据 */
  const requestClearAll = () => {
    if (clearing) return;
    setError(null);
    setClearConfirm(true);
  };

  /** 取消清空：关掉确认条，把焦点还给橡皮擦（若它已禁用则回搜索框） */
  const cancelClearAll = useCallback(() => {
    setClearConfirm(false);
    window.requestAnimationFrame(() => {
      if (eraserRef.current && !eraserRef.current.disabled) {
        eraserRef.current.focus();
      } else {
        searchRef.current?.focus();
      }
    });
  }, []);

  /** 确认清空：沿用原来的 clearAll 行为，成功后才关确认条并播报 */
  const confirmClearAll = async () => {
    if (clearing) return;
    setClearing(true);
    setError(null);
    try {
      const removed = await api.clearClips("all");
      setClearConfirm(false);
      await refresh();
      setClearMessage(`已清空 ${removed} 条记录`);
      window.requestAnimationFrame(() => searchRef.current?.focus());
    } catch (err) {
      setError(`清空失败：${String(err)}`);
    } finally {
      setClearing(false);
    }
  };

  /** Tab 只在「取消 / 清空」之间循环，别把焦点漏回列表 */
  const onClearConfirmKeyDown = (e: ReactKeyboardEvent<HTMLDivElement>) => {
    if (e.key !== "Tab") return;
    const first = clearCancelRef.current;
    const last = clearConfirmButtonRef.current;
    if (!first || !last) return;
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  };

  useEffect(() => {
    void dataDir().then(setDir);
  }, []);

  // 面板不透明度：写进根节点的 CSS 变量，玻璃卡片 / 标题栏 / 状态条 / 页脚一起变透
  useEffect(() => {
    // NaN / 0 一律当 100%（写成 NaN 会让 calc 在计算值阶段失效，底色直接没了）
    const value = panelOpacity > 0 ? panelOpacity / 100 : 1;
    document.documentElement.style.setProperty("--panel-opacity", String(value));
  }, [panelOpacity]);

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

  // 清空确认出现时，把焦点放到安全操作「取消」上；再次点击确认前不默认危险操作
  useEffect(() => {
    if (!clearConfirm) return;
    const frame = window.requestAnimationFrame(() => clearCancelRef.current?.focus());
    return () => window.cancelAnimationFrame(frame);
  }, [clearConfirm]);

  // 面板被收起时丢掉未确认的清空状态，下次打开重新开始，别带着旧计数
  useEffect(() => {
    const unlisten = api.onPanelVisible((visible) => {
      if (!visible) setClearConfirm(false);
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

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
        // 层级：清空确认 → 关于 → 设置 → 收起面板
        if (clearConfirm) {
          e.preventDefault();
          cancelClearAll();
          return;
        }
        if (aboutOpen) setAboutOpen(false);
        else if (settingsOpen) setSettingsOpen(false);
        else void api.hidePanel();
        return;
      }
      if (clearConfirm) return; // 确认态里 Enter / 方向键交给聚焦按钮，别打开 viewer
      if (aboutOpen || settingsOpen) return;
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
  }, [clips, active, settingsOpen, aboutOpen, clearConfirm, cancelClearAll]);

  // 换搜索词 / 筛选：结果换了一批，选中项回到第一条、列表回顶部。
  // 注意这跟下面的「选中项进视野」是两件事：refresh 造成的数组变化不能滚列表。
  useEffect(() => {
    // oxlint-disable-next-line react/set-state-in-effect -- 搜索 / 筛选是外部输入，重置选中项
    setActive(0);
    clipListRef.current?.scrollTo({ top: 0 });
  }, [debounced, kind]);

  // 选中项滚动进视野：**只跟 active（以及从设置屏返回）走，别依赖 clips**。
  // 面板每次 refresh（复制了新内容、关掉详情窗口后重新聚焦）都会拿到一个新数组，
  // 依赖 clips 就会反复把列表滚到 active 那一条（通常是第 0 条）—— 用户「滚到底部
  // 点开详情，关掉后跳回顶部」就是这条触发的。
  useEffect(() => {
    document
      .querySelector(".clip-row.is-active")
      ?.scrollIntoView({ block: "nearest" });
  }, [active, settingsOpen]);

  /** 单击一条：先把它设为选中（选中跟手，别只让方向键改），再看完整内容 */
  const handleRowClick = (clip: Clip) => {
    const index = clips.findIndex((item) => item.id === clip.id);
    if (index >= 0) setActive(index);
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

  /** 桌面置顶开关：后端一次改两处 —— 窗口 `always_on_top` 与落盘的 `panelPinned`。
   * 关掉之后面板会在失焦时自动收起（Rust 侧 `WindowEvent::Focused`），
   * 所以开关的文案要如实说清「点外部会关」。
   */
  const togglePinned = async () => {
    const next = !pinned;
    setPinned(next);
    try {
      setError(null);
      await api.setPanelPinned(next);
    } catch (err) {
      setPinned(!next);
      setError(`切换桌面置顶失败：${String(err)}`);
    }
  };

  /** 清空确认文案：计数拿不到就用「全部 / 未知」，别把未知冒充 0 */
  const clipCount = stats?.total;
  const pinCount = pinsLoaded ? pinnedIds.length : null;
  const clearCountText =
    clipCount == null ? "全部记录" : `${clipCount} 条记录`;
  const clearPinText =
    pinCount == null
      ? "，已贴到桌面的内容也会关闭"
      : pinCount > 0
        ? `，并关闭 ${pinCount} 个桌面贴图`
        : "，当前没有桌面贴图";
  const clearConfirmLabel =
    clipCount == null ? "清空全部" : `清空 ${clipCount} 条`;
  const eraserDisabled =
    clearing || stats === null || (stats?.total ?? 0) === 0;
  const eraserTitle = clearing
    ? "正在清空全部历史…"
    : stats === null
      ? "正在读取历史…"
      : (stats.total === 0)
        ? "没有可清空的历史"
        : "清空全部历史";

  return (
    <div className="app-card">
      {clearConfirm && (
        <div
          className="panel-clear-backdrop"
          aria-hidden="true"
          onClick={cancelClearAll}
        />
      )}
      <header
        className="app-header"
        data-tauri-drag-region="deep"
        onDoubleClick={(e) => e.stopPropagation()}
      >
        <div className="app-header-side">
          {settingsOpen ? (
            <button
              type="button"
              className="app-header-button"
              title="返回历史"
              aria-label="返回历史"
              onClick={() => setSettingsOpen(false)}
            >
              <IconArrowBack size={16} />
            </button>
          ) : (
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

        <div className="app-brand">
          {settingsOpen ? <IconGear size={16} /> : <IconClipboardArrow size={16} />}
          {settingsOpen ? "剪贴板设置" : "PPCLIP"}
        </div>

        <div className="app-header-side app-header-side--end">
          {!settingsOpen && (
            <button
              type="button"
              className={`app-header-button app-header-button--tertiary${pinned ? " is-on" : ""}`}
              title={
                pinned
                  ? "已开启桌面置顶：面板常驻最上层，点外部不会关"
                  : "桌面置顶：常驻最上层，点外部不关闭"
              }
              aria-label="桌面置顶"
              aria-pressed={pinned}
              onClick={() => void togglePinned()}
            >
              <IconKeepOnTop size={16} />
            </button>
          )}
          {/* 最右侧固定是关闭：设置屏也一样，Esc 才只回上一层 */}
          <button
            type="button"
            className="app-header-button"
            title={settingsOpen ? "关闭面板" : "关闭面板（Esc）"}
            aria-label="关闭面板"
            onClick={() => void api.hidePanel()}
          >
            <IconClose size={16} />
          </button>
        </div>
      </header>

      <main className="app-main">
        <span className="app-glow app-glow--primary" aria-hidden="true" />
        <span className="app-glow app-glow--tertiary" aria-hidden="true" />

        {settingsOpen ? (
          <SettingsPage
            version={version}
            onOpenAbout={() => setAboutOpen(true)}
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

            <div className="clip-list" ref={clipListRef}>
              {clips.length === 0 ? (
                <p className="placeholder">
                  {debounced
                    ? "没有匹配的记录"
                    : "还没有记录：复制一段文字、一张图，或几个文件试试"}
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

            {clearConfirm && (
              <div
                className="panel-clear-confirm"
                role="alertdialog"
                aria-modal="true"
                aria-labelledby="panel-clear-title"
                aria-describedby="panel-clear-desc"
                onKeyDown={onClearConfirmKeyDown}
              >
                <div className="panel-clear-copy">
                  <strong id="panel-clear-title" className="panel-clear-title">
                    清空全部历史？
                  </strong>
                  <span id="panel-clear-desc" className="panel-clear-desc">
                    将删除{clearCountText}
                    {clearPinText}；磁盘上的图片文件也会删除。此操作无法撤销。
                  </span>
                  <span className="panel-clear-hint">
                    文件条目的源文件不会被删除。
                  </span>
                </div>
                <div className="panel-clear-actions">
                  <button
                    ref={clearConfirmButtonRef}
                    type="button"
                    className="ghost-button ghost-button--danger"
                    disabled={clearing}
                    onClick={() => void confirmClearAll()}
                  >
                    {clearing ? "清空中…" : clearConfirmLabel}
                  </button>
                  <button
                    ref={clearCancelRef}
                    type="button"
                    className="ghost-button"
                    disabled={clearing}
                    onClick={cancelClearAll}
                  >
                    取消
                  </button>
                </div>
              </div>
            )}

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
                  ref={eraserRef}
                  type="button"
                  className="status-button status-button--danger"
                  title={eraserTitle}
                  aria-label={eraserTitle}
                  disabled={eraserDisabled}
                  onClick={requestClearAll}
                >
                  <IconEraser size={14} />
                </button>
              </div>
            </div>
          </>
        )}
      </main>

      {error && <div className="panel-error">{error}</div>}

      {clearMessage && (
        <div className="toast panel-clear-status" role="status">
          {clearMessage}
        </div>
      )}

      <footer className="app-footer">
        <div className="app-footer-group">
          <button
            type="button"
            className="app-footer-version"
            title="关于 PPClip"
            aria-label="关于 PPClip"
            onClick={() => setAboutOpen(true)}
          >
            {version ? `PPClip v${version}` : "PPClip"}
          </button>
          <span className="app-footer-dot" aria-hidden="true" />
          <span>
            {stats
              ? `${stats.textCount} 文本 · ${stats.imageCount} 图片${
                  stats.fileCount > 0 ? ` · ${stats.fileCount} 文件` : ""
                } · ${formatBytes(stats.imageBytes)}`
              : "Ready"}
          </span>
        </div>
        <div className="app-footer-group">
          <span className="kbd">Esc</span>
          <span>{clearConfirm ? "取消" : settingsOpen ? "返回" : "关闭"}</span>
        </div>
      </footer>

      {aboutOpen && (
        <AboutPage version={version} onClose={() => setAboutOpen(false)} />
      )}
    </div>
  );
}
