import { memo } from "react";

import { assetUrl } from "../api";
import type { Clip } from "../types";

interface Props {
  clip: Clip;
  dataDir: string;
  active: boolean;
  /** 当前时间戳：相对时间（几分钟前）要跟着走，由面板每分钟刷一次 */
  now: number;
  /** 这条正在等第二次点击确认删除 */
  confirming: boolean;
  onClick: (clip: Clip) => void;
  onCopy: (clip: Clip) => void;
  onDelete: (clip: Clip) => void;
  onContextMenu: (clip: Clip) => void;
}

function formatBytes(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  if (bytes >= 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${bytes} B`;
}

/** 紧凑的相对时间：刚刚 / N 分钟前 / 今天 HH:MM / 昨天 HH:MM / MM-DD HH:MM / YYYY-MM-DD */
function formatWhen(raw: string, now: number): string {
  const date = new Date(raw);
  if (Number.isNaN(date.getTime())) return raw;

  const minutes = Math.floor((now - date.getTime()) / 60_000);
  if (minutes < 0) return formatAbsolute(date, now);
  if (minutes < 1) return "刚刚";
  if (minutes < 60) return `${minutes} 分钟前`;
  return formatAbsolute(date, now);
}

function formatAbsolute(date: Date, now: number): string {
  const today = new Date(now);
  const hhmm = `${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(2, "0")}`;

  if (date.toDateString() === today.toDateString()) return `今天 ${hhmm}`;
  const yesterday = new Date(today);
  yesterday.setDate(today.getDate() - 1);
  if (date.toDateString() === yesterday.toDateString()) return `昨天 ${hhmm}`;

  const md = `${date.getMonth() + 1}-${date.getDate()}`;
  if (date.getFullYear() === today.getFullYear()) return `${md} ${hhmm}`;
  return `${date.getFullYear()}-${md}`;
}

/** 悬停提示里的完整时间 */
function formatFull(raw: string): string {
  const date = new Date(raw);
  return Number.isNaN(date.getTime()) ? raw : date.toLocaleString();
}

/**
 * 一条历史：单击看完整内容，右侧两个图标按钮负责复制 / 删除，第二行显示时间。
 *
 * 用 `div role="button"` 而不是 `<button>`：里面还要放按钮，嵌套 button 是非法结构，
 * 点击也会互相干扰。键盘走面板那套方向键 + Enter（见 `PanelApp`）。
 * 删除是破坏性操作（文件和库记录都会没了），所以要点两次：第一次把按钮变成「确认删除」。
 */
function ClipRow({
  clip,
  dataDir,
  active,
  now,
  confirming,
  onClick,
  onCopy,
  onDelete,
  onContextMenu,
}: Props) {
  const thumb =
    clip.kind === "image" && clip.thumbRel
      ? assetUrl(dataDir, clip.thumbRel)
      : null;

  return (
    <div
      role="button"
      tabIndex={0}
      className={`clip-row${active ? " is-active" : ""}`}
      title={`单击查看完整内容 · 右侧按钮复制 / 删除 · 右键更多\n创建：${formatFull(clip.createdAt)}\n最后使用：${formatFull(clip.lastUsedAt)}`}
      onClick={() => onClick(clip)}
      onContextMenu={(e) => {
        e.preventDefault();
        onContextMenu(clip);
      }}
    >
      <span className={`clip-kind clip-kind--${clip.kind}`}>
        {clip.kind === "image" ? "图" : "文"}
      </span>

      <span className="clip-main">
        {thumb ? (
          <span className="clip-thumb">
            <img src={thumb} alt="" draggable={false} />
            <span className="clip-thumb-meta">
              {clip.width}×{clip.height} · {formatBytes(clip.bytes)}
            </span>
          </span>
        ) : (
          <span className="clip-text">{clip.preview}</span>
        )}

        <span className="clip-meta">
          {clip.source === "screenshot" && (
            <span className="clip-badge">截图</span>
          )}
          {formatWhen(clip.lastUsedAt, now)}
          {clip.useCount > 1 && ` · 用过 ${clip.useCount} 次`}
        </span>
      </span>

      <span className="clip-actions">
        <button
          type="button"
          className="clip-action"
          title="复制到剪贴板（Ctrl+Enter）"
          aria-label="复制到剪贴板"
          onClick={(e) => {
            e.stopPropagation();
            onCopy(clip);
          }}
        >
          <span className="clip-copy-glyph" aria-hidden="true" />
        </button>

        <button
          type="button"
          className={`clip-action clip-action--danger${confirming ? " is-confirming" : ""}`}
          title={confirming ? "再点一次确认删除" : "删除这条记录"}
          aria-label={confirming ? "确认删除" : "删除这条记录"}
          onClick={(e) => {
            e.stopPropagation();
            onDelete(clip);
          }}
        >
          {confirming ? (
            <span className="clip-del-confirm">确认删除</span>
          ) : (
            <span className="clip-del-glyph" aria-hidden="true" />
          )}
        </button>
      </span>
    </div>
  );
}

export default memo(ClipRow);
