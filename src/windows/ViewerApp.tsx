import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import { api, assetUrl, dataDir } from "../api";
import {
  IconActualSize,
  IconArrowBack,
  IconCheck,
  IconClipboard,
  IconCopy,
  IconFile,
  IconFitScreen,
  IconFolder,
  IconImage,
  IconFloatCard,
  IconScan,
  IconSchedule,
  IconSelectAll,
  IconText,
  IconZoomIn,
  IconZoomOut,
} from "../components/icons";
import type { Clip, FileDetail, FileHashes } from "../types";

/** 放大 / 缩小的步进与范围 */
const ZOOM_STEP = 1.25;
const ZOOM_MIN = 0.25;
const ZOOM_MAX = 6;

/**
 * 字节数：MB 一位小数、100 KB 以上取整、更小的保留一位小数。
 * 文件详情里十几 KB 的文件写成「5 KB」（实际 4.5 KB）会显得尺寸不准。
 */
function formatBytes(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  if (bytes >= 100 * 1024) return `${Math.round(bytes / 1024)} KB`;
  if (bytes >= 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${bytes} B`;
}

/** 修改时间：今年的 `MM-DD HH:MM`，跨年补年份（详情卡那三格很窄，塞不下完整时间） */
function formatStamp(raw: string): string {
  const date = new Date(raw);
  if (Number.isNaN(date.getTime())) return raw;
  const pad = (value: number) => String(value).padStart(2, "0");
  const md = `${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
  const hhmm = `${pad(date.getHours())}:${pad(date.getMinutes())}`;
  return date.getFullYear() === new Date().getFullYear()
    ? `${md} ${hhmm}`
    : `${date.getFullYear()}-${md}`;
}

/** RFC3339 → 本地时间；后端存的是 UTC */
function formatTime(raw: string): string {
  const date = new Date(raw);
  return Number.isNaN(date.getTime()) ? raw : date.toLocaleString();
}

/**
 * 完整内容窗口（单例，label `viewer`），照 stitch 稿的「详情页」：
 *
 * 48px 头部（← 返回 / 居中品牌 + 类型图标 / 贴到桌面）→ 内容卡片
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
  /** 当前条目是否已经贴在桌面上（浮窗按钮据此变成「取消贴图」） */
  const [pinned, setPinned] = useState(false);
  /**
   * 文件条目的**现场**信息（大小 / 修改时间 / 是否还在）：打开详情时后端 stat 一次。
   * 不进数据库 —— 用户可能已经改过、删过文件，拿旧快照显示就是在骗人。
   */
  const [fileDetails, setFileDetails] = useState<FileDetail[]>([]);
  /** 单文件的 SHA-256（按需算：key = 路径，切条目就清空） */
  /** 按需算出来的两种校验和（换条目时清空，见下面加载 effect） */
  const [hashes, setHashes] = useState<FileHashes | null>(null);
  const [hashing, setHashing] = useState(false);
  const textRef = useRef<HTMLPreElement>(null);
  const fileListRef = useRef<HTMLDivElement>(null);
  const isImage = clip?.kind === "image";
  const isFile = clip?.kind === "file";
  /** 正文：文本类就是 `text`（文件类有专门的列表布局，不走这里） */
  const body = clip && !isFile ? (clip.text ?? "") : "";
  const totalFileBytes = fileDetails.reduce((sum, detail) => sum + detail.size, 0);
  /** 多文件时的格式标签（去重；文件夹单独算一类） */
  const extLabels = [
    ...new Set(fileDetails.map((detail) => detail.ext || (detail.isDir ? "文件夹" : "文件"))),
  ];
  /** 最新的修改时间（RFC3339 字典序就是时间序） */
  const latestModified = fileDetails
    .map((detail) => detail.modified)
    .filter((value): value is string => Boolean(value))
    .sort()
    .pop();

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
      setHashes(null);
      setFileDetails([]);
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

  // 文件条目的现场信息：换条目时重新 stat 一次（不在列表接口里做，面板列表不该为几行文件去读磁盘）。
  // 清空在加载那条 effect 里做（同步 setState 会多一轮渲染，也过不了 lint）。
  useEffect(() => {
    const paths = isFile ? (clip?.files ?? []) : [];
    if (paths.length === 0) return;
    let alive = true;
    void api
      .fileDetails(paths)
      .then((details) => {
        if (alive) setFileDetails(details);
      })
      .catch(() => {
        if (alive) setFileDetails([]);
      });
    return () => {
      alive = false;
    };
  }, [clip, isFile]);

  /** 复制一段文本（文件名 / 路径 / 校验码）：走 `copy_text`，监听线程不会多记一条历史 */
  const copyTextValue = async (text: string, note: string) => {
    try {
      await api.copyText(text);
      setToast(note);
    } catch (err) {
      setError(`复制失败：${String(err)}`);
    }
  };

  /** 在资源管理器里定位文件 */
  const revealFile = async (path: string) => {
    try {
      await api.revealPath(path);
    } catch (err) {
      setError(`打开文件位置失败：${String(err)}`);
    }
  };

  /** 按需算校验和（SHA-256 + MD5 一次读盘算两个；几百 MB 的文件不会在读详情时就卡住） */
  const computeHashes = async (path: string) => {
    if (hashing) return;
    setHashing(true);
    try {
      setHashes(await api.fileHashes(path));
    } catch (err) {
      setError(`计算校验和失败：${String(err)}`);
    } finally {
      setHashing(false);
    }
  };

  /** 复制到剪贴板 + 提示条 */
  const copy = async () => {
    try {
      await api.copyClip(clipId ?? 0);
      setToast("已复制到剪贴板");
    } catch (err) {
      setError(`复制失败：${String(err)}`);
    }
  };

  /** 贴图开关：已经贴着就关掉，没贴就贴一张（那个按钮是 toggle 语义） */
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

  const zoomBy = (factor: number) => {
    setFit(false);
    setZoom((value) =>
      Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, Number((value * factor).toFixed(3)))),
    );
  };

  /** 全选正文（文本选 `<pre>`，文件选整个路径列表） */
  const selectAll = useCallback(() => {
    const node = textRef.current ?? fileListRef.current;
    if (!node) return;
    const selection = window.getSelection();
    selection?.removeAllRanges();
    selection?.selectAllChildren(node);
  }, []);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") void api.closeViewer();
      // Ctrl+A = 全选正文（WebView 不会自己拦，这里显式接住）；
      // Ctrl+C 不用管 —— 有选区时原生复制就生效
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "a" && !isImage) {
        e.preventDefault();
        selectAll();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [isImage, selectAll]);

  const chars = body.length;
  const zoomLabel = fit ? "适应" : `${Math.round(zoom * 100)}%`;

  return (
    <div className="app-card">
      <header className="app-header" data-tauri-drag-region="deep">
        <div className="app-header-side">
          <button
            type="button"
            className="app-header-button"
            title="返回（Esc）"
            aria-label="返回"
            onClick={() => void api.closeViewer()}
          >
            <IconArrowBack size={16} />
          </button>
        </div>

        <div className="app-brand">
          {isImage ? <IconImage size={16} /> : isFile ? <IconFile size={16} /> : <IconText size={16} />}
          PPCLIP
        </div>

        <div className="app-header-side">
          {/* 文件条目贴不了桌面（后端会拒），这里干脆不摆按钮 */}
          {!isFile && (
            <button
              type="button"
              className={`app-header-button app-header-button--tertiary${pinned ? " is-on" : ""}`}
              title={pinned ? "取消贴图" : "贴到桌面"}
              aria-label={pinned ? "取消贴图" : "贴到桌面"}
              aria-pressed={pinned}
              onClick={() => void togglePin()}
            >
              <IconFloatCard size={16} />
            </button>
          )}
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
                    <IconClipboard size={13} />
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
                ) : isFile ? (
                  <>
                    <span className="meta-chip meta-chip--secondary">
                      {fileDetails.length > 1
                        ? `${fileDetails.length} 个文件`
                        : (fileDetails[0]?.kind ?? "文件")}
                    </span>
                    <span className="meta-chip">
                      {formatBytes(fileDetails.length > 0 ? totalFileBytes : (clip?.bytes ?? 0))}
                    </span>
                    <span className="meta-chip meta-chip--primary">
                      {fileDetails.length === 1
                        ? (fileDetails[0].ext || "文件")
                        : `${extLabels.length} 种格式`}
                    </span>
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
                  {body}
                </pre>
              )}

              {/* 文件条目：照 stitch 稿「CLIPBOARD DETAIL」——
                  文件卡（类型 + 文件名 + 复制文件名）→ 格式 / 大小 / 修改时间 → 路径块 → 校验和块。
                  多个文件时退化成「总览卡 + 一行一个文件」，每行照样能复制路径 / 定位。
                  全部字段都是**现场**读的（`file_details`），删过、挪过的文件会如实标出来。 */}
              {isFile && clip && (
                <div ref={fileListRef} className="file-detail">
                  {fileDetails.length === 0 ? (
                    <p className="viewer-files-empty">
                      {(clip.files ?? []).length === 0
                        ? "没有可显示的文件路径"
                        : "正在读取文件信息…"}
                    </p>
                  ) : fileDetails.length === 1 ? (
                    <>
                      <div className="file-card">
                        <div className="file-card-icon">
                          <IconFile size={24} />
                        </div>
                        <div className="file-card-text">
                          <span className="file-card-kind">
                            {fileDetails[0].kind}
                            {!fileDetails[0].exists && (
                              <span className="file-card-missing">已不在原位置</span>
                            )}
                          </span>
                          <span className="file-card-name" title={fileDetails[0].path}>
                            {fileDetails[0].name}
                          </span>
                        </div>
                        <button
                          type="button"
                          className="ghost-button"
                          title="复制文件名"
                          onClick={() =>
                            void copyTextValue(fileDetails[0].name, "已复制文件名")
                          }
                        >
                          <IconCopy size={12} />
                          复制文件名
                        </button>
                      </div>

                      <div className="file-facts">
                        <div className="file-fact">
                          <span className="file-fact-label">文件格式</span>
                          <span className="file-fact-value file-fact-value--accent">
                            {fileDetails[0].isDir ? "文件夹" : fileDetails[0].ext || "—"}
                          </span>
                        </div>
                        <div className="file-fact">
                          <span className="file-fact-label">文件大小</span>
                          <span className="file-fact-value">
                            {fileDetails[0].isDir
                              ? "—"
                              : formatBytes(fileDetails[0].size)}
                          </span>
                        </div>
                        <div className="file-fact">
                          <span className="file-fact-label">修改时间</span>
                          <span className="file-fact-value">
                            {fileDetails[0].modified
                              ? formatStamp(fileDetails[0].modified)
                              : "—"}
                          </span>
                        </div>
                      </div>

                      <div className="file-block">
                        <span className="file-block-label">绝对路径</span>
                        <div className="file-block-row">
                          <span className="file-path" title={fileDetails[0].path}>
                            {fileDetails[0].path}
                          </span>
                          <div className="file-block-actions">
                            <button
                              type="button"
                              className="ghost-button"
                              title="复制完整路径"
                              onClick={() =>
                                void copyTextValue(fileDetails[0].path, "已复制路径")
                              }
                            >
                              <IconCopy size={12} />
                              复制路径
                            </button>
                            <button
                              type="button"
                              className="ghost-button"
                              title="在资源管理器里选中它"
                              onClick={() => void revealFile(fileDetails[0].path)}
                            >
                              <IconFolder size={12} />
                              在文件夹中打开
                            </button>
                          </div>
                        </div>
                      </div>

                      <div className="file-block">
                        <div className="file-block-head">
                          <span className="file-block-label">文件校验和</span>
                          {!hashes && (
                            <button
                              type="button"
                              className="ghost-button"
                              disabled={hashing || !fileDetails[0].exists}
                              title={
                                fileDetails[0].exists
                                  ? "读一遍文件，同时算出 SHA-256 与 MD5"
                                  : "文件已不在原位置，算不了"
                              }
                              onClick={() => void computeHashes(fileDetails[0].path)}
                            >
                              {hashing ? "计算中…" : "计算校验和"}
                            </button>
                          )}
                        </div>
                        {(
                          [
                            ["SHA-256", hashes?.sha256, "sha256"],
                            ["MD5", hashes?.md5, "md5"],
                          ] as const
                        ).map(([label, value, key]) => (
                          <div className="file-hash-row" key={key}>
                            <span className="file-hash-label">{label}</span>
                            <span className="file-hash" title={value ?? undefined}>
                              {value ?? (hashing ? "计算中…" : "按需计算")}
                            </span>
                            <button
                              type="button"
                              className="file-icon-button"
                              disabled={!value}
                              aria-label={`复制 ${label} 校验码`}
                              title={value ? `复制 ${label} 校验码` : "先算一次校验和"}
                              onClick={() => void copyTextValue(value ?? "", "已复制校验码")}
                            >
                              <IconCopy size={12} />
                            </button>
                          </div>
                        ))}
                      </div>
                    </>
                  ) : (
                    <>
                      <div className="file-card">
                        <div className="file-card-icon">
                          <IconFile size={24} />
                        </div>
                        <div className="file-card-text">
                          <span className="file-card-kind">
                            {fileDetails.length} 个文件 · {formatBytes(totalFileBytes)}
                          </span>
                          <span className="file-card-name">
                            {extLabels.slice(0, 3).join(" · ")}
                            {latestModified && ` · 最新修改 ${formatStamp(latestModified)}`}
                          </span>
                        </div>
                        <button
                          type="button"
                          className="ghost-button"
                          title="复制全部路径（每行一条）"
                          onClick={() =>
                            void copyTextValue(
                              fileDetails.map((detail) => detail.path).join("\r\n"),
                              "已复制全部路径",
                            )
                          }
                        >
                          <IconCopy size={12} />
                          复制全部路径
                        </button>
                      </div>

                      <div className="file-list">
                        {fileDetails.map((detail) => (
                          <div key={detail.path} className="file-list-row" title={detail.path}>
                            <span className="file-ext-badge">
                              {detail.isDir ? "DIR" : detail.ext || "FILE"}
                            </span>
                            <span className="file-list-name">{detail.name}</span>
                            <span
                              className={
                                "file-list-size" + (detail.exists ? "" : " is-missing")
                              }
                            >
                              {!detail.exists
                                ? "已不在"
                                : detail.isDir
                                  ? "文件夹"
                                  : formatBytes(detail.size)}
                            </span>
                            <button
                              type="button"
                              className="file-icon-button"
                              title="复制完整路径"
                              aria-label="复制完整路径"
                              onClick={() => void copyTextValue(detail.path, "已复制路径")}
                            >
                              <IconCopy size={12} />
                            </button>
                            <button
                              type="button"
                              className="file-icon-button"
                              title="在资源管理器里选中它"
                              aria-label="在资源管理器里选中它"
                              onClick={() => void revealFile(detail.path)}
                            >
                              <IconFolder size={12} />
                            </button>
                          </div>
                        ))}
                      </div>
                    </>
                  )}
                </div>
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
                    <IconActualSize size={13} />
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
                    {fit ? <IconActualSize size={13} /> : <IconFitScreen size={13} />}
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
        {/* 左边只放「这条内容的客观事实」，右边只放快捷键提示 ——
            稿子（600x500_v3）与面板页脚都是右端只有 `Esc Close`，
            时间留在右端会把 Esc 提示从最右边挤开。 */}
        <div className="app-footer-group app-footer-facts mono">
          <span>
            MIME:{" "}
            {isImage
              ? "image/png"
              : isFile
                ? fileDetails.length === 1
                  ? fileDetails[0].mime
                  : "text/uri-list"
                : "text/plain"}
          </span>
          <span className="app-footer-dot" aria-hidden="true" />
          <span>{clip ? `创建 ${formatTime(clip.createdAt)}` : "加载中…"}</span>
        </div>
        <div className="app-footer-group">
          <span className="kbd">Esc</span>
          <span>返回</span>
        </div>
      </footer>
    </div>
  );
}
