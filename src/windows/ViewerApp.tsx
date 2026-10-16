import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import { api, assetUrl, dataDir } from "../api";
import {
  IconCheck,
  IconClose,
  IconCopy,
  IconCropFree,
  IconFitScreen,
  IconImage,
  IconPin,
  IconPublic,
  IconScan,
  IconSchedule,
  IconSelectAll,
  IconText,
  IconZoomIn,
  IconZoomOut,
} from "../components/icons";
import type { Clip } from "../types";

/** 放大 / 缩小的步进与范围 */
const ZOOM_STEP = 1.25;
const ZOOM_MIN = 0.25;
const ZOOM_MAX = 6;

function formatBytes(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  if (bytes >= 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${bytes} B`;
}

/** RFC3339 → 本地时间；后端存的是 UTC */
function formatTime(raw: string): string {
  const date = new Date(raw);
  return Number.isNaN(date.getTime()) ? raw : date.toLocaleString();
}

/**
 * 完整内容窗口（单例，label `viewer`），照 stitch 稿的「详情页」：
 *
 * 48px 头部（← 关闭 / 居中品牌 + 类型图标 / 贴到桌面）→ 内容卡片
 * （meta 条：来源徽章 · 时间 ／ 尺寸 / 大小 / 格式 chip）→ 点阵网格内容区
 * （图片右下角是缩放控件浮层）→ 底部操作行（复制 / 原始尺寸 / 全选）→ 页脚 MIME。
 *
 * 显示哪条由后端 `ViewerState` 决定：挂载时问一次，之后靠 `viewer:show` 事件换内容
 * （事件不传数据，只当「去问一次」的通知，省掉窗口刚建好还没监听时的丢事件问题）。
 */
export default function ViewerApp() {
  const [clipId, setClipId] = useState<number | null>(null);
  const [clip, setClip] = useState<Clip | null>(null);
  const [src, setSrc] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  /**
   * 图片的显示方式：适应窗口（fit）或按 zoom 倍率原始尺寸滚动。
   * 文本永远按真实高度滚动 —— 这个开关不能套到文本上，否则内容根本滚不动。
   */
  const [fit, setFit] = useState(true);
  const [zoom, setZoom] = useState(1);
  /** 顶部居中的提示条（稿子里的 toast）：复制 / 贴图之后给一次反馈 */
  const [toast, setToast] = useState<string | null>(null);
  /** 当前条目是否已经贴在桌面上（图钉按钮据此变成「取消贴图」） */
  const [pinned, setPinned] = useState(false);
  const textRef = useRef<HTMLPreElement>(null);
  const isImage = clip?.kind === "image";

  /** 提示条 1.8 秒后自己消失 */
  useEffect(() => {
    if (toast === null) return;
    const timer = window.setTimeout(() => setToast(null), 1800);
    return () => window.clearTimeout(timer);
  }, [toast]);

  // 后端告诉我们当前该显示哪条
  useEffect(() => {
    let alive = true;
    const sync = async () => {
      const id = await api.viewerClip().catch(() => null);
      if (alive) setClipId(id ?? null);
    };

    void sync();
    const unlisten = listen("viewer:show", () => void sync());
    return () => {
      alive = false;
      void unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    if (clipId === null) return;
    void (async () => {
      setSrc(null);
      setError(null);
      setFit(true);
      setZoom(1);
      try {
        const loaded = await api.getClip(clipId);
        setClip(loaded);
        if (loaded.imageRel) {
          setSrc(assetUrl(await dataDir(), loaded.imageRel));
        }
        const pins = await api.listPins().catch(() => []);
        setPinned(pins.some((pin) => pin.clipId === clipId));
      } catch (err) {
        setClip(null);
        setError(`读取内容失败：${String(err)}`);
      }
    })();
  }, [clipId]);

  /** 复制到剪贴板 + 提示条 */
  const copy = async () => {
    try {
      await api.copyClip(clipId ?? 0);
      setToast("已复制到剪贴板");
    } catch (err) {
      setError(`复制失败：${String(err)}`);
    }
  };

  /** 贴图开关：已经贴着就关掉，没贴就贴一张（稿子里图钉按钮是 toggle 语义） */
  const togglePin = async () => {
    if (clipId === null) return;
    try {
      const pins = await api.listPins();
      const existing = pins.find((pin) => pin.clipId === clipId);
      if (existing) {
        await api.deletePin(existing.id);
        setPinned(false);
        setToast("已取消贴图");
      } else {
        await api.createPin(clipId);
        setPinned(true);
        setToast("已贴到桌面");
      }
    } catch (err) {
      setError(`贴图操作失败：${String(err)}`);
    }
  };

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") void api.closeViewer();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  const zoomBy = (factor: number) => {
    setFit(false);
    setZoom((value) =>
      Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, Number((value * factor).toFixed(3)))),
    );
  };

  /** 全选正文（文本模式下比手拖更常用） */
  const selectAll = () => {
    const node = textRef.current;
    if (!node) return;
    const selection = window.getSelection();
    selection?.removeAllRanges();
    selection?.selectAllChildren(node);
  };

  const chars = clip?.text?.length ?? clip?.preview.length ?? 0;
  const zoomLabel = fit ? "适应" : `${Math.round(zoom * 100)}%`;

  return (
    <div className="app-card">
      <header className="app-header" data-tauri-drag-region="deep">
        <div className="app-header-side">
          <button
            type="button"
            className="app-header-button"
            title="关闭（Esc）"
            aria-label="关闭查看窗口"
            onClick={() => void api.closeViewer()}
          >
            <IconClose size={16} />
          </button>
        </div>

        <div className="app-brand">
          {isImage ? <IconImage size={16} /> : <IconText size={16} />}
          PPCLIP
        </div>

        <div className="app-header-side">
          <button
            type="button"
            className={`app-header-button app-header-button--tertiary${pinned ? " is-on" : ""}`}
            title={pinned ? "取消贴图" : "贴到桌面"}
            aria-label={pinned ? "取消贴图" : "贴到桌面"}
            aria-pressed={pinned}
            onClick={() => void togglePin()}
          >
            <IconPin size={16} />
          </button>
        </div>
      </header>

      <main className="app-main">
        <span className="app-glow app-glow--primary" aria-hidden="true" />
        <span className="app-glow app-glow--tertiary" aria-hidden="true" />

        {toast && (
          <div className="toast" role="status">
            <IconCheck size={15} />
            {toast}
          </div>
        )}

        <div className="viewer-body">
          <div className="viewer-card">
            {/* meta 条：来源徽章 · 时间 ／ 尺寸 / 大小 / 格式 */}
            <div className="viewer-meta">
              <div className="viewer-meta-left">
                <span className="source-badge">
                  {clip?.source === "screenshot" ? (
                    <IconScan size={13} />
                  ) : (
                    <IconPublic size={13} />
                  )}
                  {clip?.source === "screenshot" ? "区域截图" : "剪贴板"}
                </span>
                <span aria-hidden="true">•</span>
                <span className="meta-time">
                  <IconSchedule size={12} />
                  {clip ? formatTime(clip.lastUsedAt) : "…"}
                </span>
              </div>
              <div className="viewer-meta-right mono">
                {isImage ? (
                  <>
                    <span className="meta-chip meta-chip--secondary">
                      {clip?.width ?? "?"} × {clip?.height ?? "?"}
                    </span>
                    <span className="meta-chip">{formatBytes(clip?.bytes ?? 0)}</span>
                    <span className="meta-chip meta-chip--primary">PNG</span>
                  </>
                ) : (
                  <>
                    <span className="meta-chip meta-chip--secondary">
                      {chars} chars
                    </span>
                    <span className="meta-chip">{formatBytes(clip?.bytes ?? 0)}</span>
                    <span className="meta-chip meta-chip--primary">UTF-8</span>
                  </>
                )}
              </div>
            </div>

            {/* 内容区：图片居中/可滚，文本保持真实高度滚动 */}
            <div
              className={
                "viewer-canvas" +
                (isImage
                  ? fit
                    ? ""
                    : " viewer-canvas--scroll"
                  : " viewer-canvas--text")
              }
            >
              {error && <p className="viewer-error">{error}</p>}
              {src && clip && (
                <img
                  className="viewer-image"
                  src={src}
                  alt=""
                  draggable={false}
                  data-tauri-drag-region
                  style={
                    fit
                      ? undefined
                      : {
                          width: `${(clip.width ?? 0) * zoom}px`,
                          height: `${(clip.height ?? 0) * zoom}px`,
                        }
                  }
                  title="拖动可移动窗口 · 双击切换适应窗口 / 原始尺寸"
                  onDoubleClick={(e) => {
                    e.stopPropagation();
                    setFit((value) => !value);
                    setZoom(1);
                  }}
                />
              )}
              {clip?.kind === "text" && (
                <pre ref={textRef} className="viewer-text">
                  {clip.text}
                </pre>
              )}

              {isImage && src && (
                <div className="viewer-zoom">
                  <button
                    type="button"
                    className="zoom-button"
                    title="缩小"
                    aria-label="缩小"
                    onClick={() => zoomBy(1 / ZOOM_STEP)}
                  >
                    <IconZoomOut size={14} />
                  </button>
                  <span className="zoom-value">{zoomLabel}</span>
                  <button
                    type="button"
                    className="zoom-button"
                    title="放大"
                    aria-label="放大"
                    onClick={() => zoomBy(ZOOM_STEP)}
                  >
                    <IconZoomIn size={14} />
                  </button>
                  <span className="zoom-divider" aria-hidden="true" />
                  <button
                    type="button"
                    className={`zoom-button${fit ? " is-on" : ""}`}
                    title="适应窗口"
                    aria-label="适应窗口"
                    onClick={() => {
                      setFit(true);
                      setZoom(1);
                    }}
                  >
                    <IconFitScreen size={13} />
                  </button>
                  <button
                    type="button"
                    className={`zoom-button${!fit && zoom === 1 ? " is-on" : ""}`}
                    title="原始尺寸（100%）"
                    aria-label="原始尺寸"
                    onClick={() => {
                      setFit(false);
                      setZoom(1);
                    }}
                  >
                    <IconCropFree size={13} />
                  </button>
                </div>
              )}
            </div>

            {/* 底部操作行 */}
            <div className="viewer-actions-row">
              <div className="viewer-actions-left">
                <button
                  type="button"
                  className="ghost-button ghost-button--primary"
                  title="复制到剪贴板"
                  onClick={() => void copy()}
                >
                  <IconCopy size={13} />
                  复制
                </button>
                {isImage ? (
                  <button
                    type="button"
                    className="ghost-button"
                    onClick={() => {
                      setFit((value) => !value);
                      setZoom(1);
                    }}
                  >
                    {fit ? <IconCropFree size={13} /> : <IconFitScreen size={13} />}
                    {fit ? "原始尺寸" : "适应窗口"}
                  </button>
                ) : (
                  <button type="button" className="ghost-button" onClick={selectAll}>
                    <IconSelectAll size={13} />
                    全选
                  </button>
                )}
              </div>
              <span className="meta-chip">
                {clip ? `用过 ${clip.useCount} 次` : ""}
              </span>
            </div>
          </div>
        </div>
      </main>

      <footer className="app-footer">
        <div className="app-footer-group mono">
          <span>MIME: {isImage ? "image/png" : "text/plain"}</span>
        </div>
        <div className="app-footer-group">
          <span className="kbd">Esc</span>
          <span>关闭</span>
          <span className="app-footer-dot" aria-hidden="true" />
          <span>{clip ? `创建 ${formatTime(clip.createdAt)}` : "加载中…"}</span>
        </div>
      </footer>
    </div>
  );
}
