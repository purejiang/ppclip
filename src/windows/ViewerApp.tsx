import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import { api, assetUrl, dataDir } from "../api";
import type { Clip } from "../types";

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
 * 完整内容窗口（单例，label `viewer`）。
 *
 * 面板里只有 120 字预览和 320px 缩略图，这里显示全文 / 原图；
 * 显示哪条由后端 `ViewerState` 决定：挂载时问一次，之后靠 `viewer:show` 事件换内容
 * （事件不传数据，只当「去问一次」的通知，省掉窗口刚建好还没监听时的丢事件问题）。
 */
export default function ViewerApp() {
  const [clipId, setClipId] = useState<number | null>(null);
  const [clip, setClip] = useState<Clip | null>(null);
  const [src, setSrc] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  /**
   * 只有图片需要「适应窗口 / 原始尺寸」，
   * 文本永远按真实高度滚动 —— 这个开关不能套到文本上，
   * 否则 `is-fit` 的 `overflow: hidden` 会让长文本根本滚不动。
   */
  const [fitImage, setFitImage] = useState(true);
  const bodyRef = useRef<HTMLDivElement>(null);
  const isImage = clip?.kind === "image";

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
      setFitImage(true);
      try {
        const loaded = await api.getClip(clipId);
        setClip(loaded);
        if (loaded.imageRel) {
          setSrc(assetUrl(await dataDir(), loaded.imageRel));
        }
      } catch (err) {
        setClip(null);
        setError(`读取内容失败：${String(err)}`);
      }
    })();
  }, [clipId]);

  // 正文拿到焦点后 PageUp / PageDown / 方向键才能滚动
  useEffect(() => {
    if (clip?.kind === "text") bodyRef.current?.focus();
  }, [clip]);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") void api.closeViewer();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  const title = !clip
    ? "加载中…"
    : clip.kind === "image"
      ? `图片 ${clip.width}×${clip.height}`
      : `文本 ${clip.text?.length ?? 0} 字`;

  return (
    <div className="viewer">
      <header className="viewer-header" data-tauri-drag-region="deep">
        <span className="viewer-title">{title}</span>
        <div className="viewer-actions">
          {isImage && (
            <button
              type="button"
              className="btn viewer-toggle"
              onClick={() => setFitImage((value) => !value)}
            >
              {fitImage ? "原始尺寸" : "适应窗口"}
            </button>
          )}
          <button
            type="button"
            className="icon-button"
            title="复制到剪贴板"
            onClick={() => void api.copyClip(clipId ?? 0)}
          >
            复
          </button>
          <button
            type="button"
            className="icon-button"
            title="关闭（Esc）"
            onClick={() => void api.closeViewer()}
          >
            ✕
          </button>
        </div>
      </header>

      {/* 拖动：标题栏整片可拖；正文的留白与图片也能拖（文本本体不拖，否则没法选中复制） */}
      <div
        ref={bodyRef}
        tabIndex={-1}
        data-tauri-drag-region
        className={`viewer-body${isImage && fitImage ? " is-fit" : ""}`}
      >
        {error && <p className="viewer-error">{error}</p>}
        {src && (
          <img
            className="viewer-image"
            src={src}
            alt=""
            draggable={false}
            data-tauri-drag-region
            title="拖动可移动窗口 · 双击切换适应窗口 / 原始尺寸"
            onDoubleClick={(e) => {
              e.stopPropagation();
              setFitImage((value) => !value);
            }}
          />
        )}
        {clip?.kind === "text" && <pre className="viewer-text">{clip.text}</pre>}
      </div>

      <footer className="viewer-footer" data-tauri-drag-region>
        <span>
          {clip
            ? `创建 ${formatTime(clip.createdAt)} · 最后使用 ${formatTime(clip.lastUsedAt)}`
            : ""}
        </span>
        <span>
          {clip
            ? `${formatBytes(clip.bytes)} · 用过 ${clip.useCount} 次 · `
            : ""}
          {isImage
            ? "双击图片切尺寸 · 拖动图片/标题栏移动 · Esc 关闭"
            : "滚轮/PageDown 翻页 · 选中后 Ctrl+C · Esc 关闭"}
        </span>
      </footer>
    </div>
  );
}
