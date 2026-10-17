import { memo } from "react";

import { assetUrl } from "../api";
import { IconCopy, IconImage, IconPin, IconText, IconTrash } from "./icons";
import type { Clip } from "../types";

interface Props {
  clip: Clip;
  dataDir: string;
  active: boolean;
  /** 当前时间戳：相对时间（几分钟前）要跟着走，由面板每分钟刷一次 */
  now: number;
  /** 这条正在等第二次点击确认删除 */
  confirming: boolean;
  /** 这条已经贴在桌面上了（图钉按钮点亮，再点=取消贴图） */
  pinned: boolean;
  /** 面板的「单击即复制」开关：只影响提示文案（行为在 `PanelApp` 里） */
  clickToCopy: boolean;
  onClick: (clip: Clip) => void;
  /** 双击：无条件看详情。「单击即复制」开启时，这是鼠标唯一的详情入口 */
  onDoubleClick: (clip: Clip) => void;
  onCopy: (clip: Clip) => void;
  onPin: (clip: Clip) => void;
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
 * 一条历史（照 stitch 稿的行结构）：
 *
 * `40×40 类型图标块` + 内容列（文本类是「预览 + 两行 meta」，图片类是「160×96 缩略图 + 两行 meta」）
 * + 右侧三个动作（复制 / 贴图 / 删除）。
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
  pinned,
  clickToCopy,
  onClick,
  onDoubleClick,
  onCopy,
  onPin,
  onDelete,
  onContextMenu,
}: Props) {
  const thumb =
    clip.kind === "image" && clip.thumbRel
      ? assetUrl(dataDir, clip.thumbRel)
      : null;
  const isImage = clip.kind === "image";
  const size = formatBytes(clip.bytes);
  const when = formatWhen(clip.lastUsedAt, now);

  return (
    <div
      role="button"
      tabIndex={0}
      className={`clip-row${active ? " is-active" : ""}`}
      title={`${clickToCopy ? "单击复制 · 双击查看完整内容" : "单击查看完整内容"} · 右侧按钮复制 / 贴图 / 删除 · 右键更多\n创建：${formatFull(clip.createdAt)}\n最后使用：${formatFull(clip.lastUsedAt)}`}
      onClick={() => onClick(clip)}
      onDoubleClick={() => onDoubleClick(clip)}
      onContextMenu={(e) => {
        e.preventDefault();
        onContextMenu(clip);
      }}
    >
      <div className="clip-row-body">
        <div
          className={`clip-kind${isImage ? " clip-kind--image" : ""}`}
          title={isImage ? "图片" : "文本"}
        >
          {isImage ? <IconImage size={20} /> : <IconText size={18} />}
        </div>

        <div className="clip-main">
          {thumb ? (
            <div className="clip-thumb">
              <img src={thumb} alt="" draggable={false} />
              {clip.source === "screenshot" && (
                <span className="clip-badge clip-badge--corner">截图</span>
              )}
            </div>
          ) : (
            <div className="clip-title-row">
              <span className="clip-text">{clip.preview}</span>
              {clip.source === "screenshot" && (
                <span className="clip-badge">截图</span>
              )}
            </div>
          )}

          <div className="clip-meta">
            <div className="clip-meta-line">
              {/* 文本条目没有图片字节，别显示「0 B」；用真实字数（列表带全文） */}
              {isImage
                ? `${clip.width ?? "?"}×${clip.height ?? "?"} · ${size}`
                : `${clip.text?.length ?? clip.preview.length} chars`}
              {clip.useCount > 1 && ` · 用过 ${clip.useCount} 次`}
            </div>
            <div className="clip-meta-time">{when}</div>
          </div>
        </div>
      </div>

      <div className="clip-actions">
        <button
          type="button"
          className="clip-action"
          title="复制到剪贴板（Ctrl+Enter）"
          aria-label="复制到剪贴板"
          onClick={(e) => {
            e.stopPropagation();
            onCopy(clip);
          }}
          onDoubleClick={(e) => e.stopPropagation()}
        >
          <IconCopy size={14} />
        </button>

        <button
          type="button"
          className={`clip-action clip-action--pin${pinned ? " is-on" : ""}`}
          title={pinned ? "取消贴图" : "贴到桌面"}
          aria-label={pinned ? "取消贴图" : "贴到桌面"}
          aria-pressed={pinned}
          onClick={(e) => {
            e.stopPropagation();
            onPin(clip);
          }}
          onDoubleClick={(e) => e.stopPropagation()}
        >
          <IconPin size={14} />
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
          onDoubleClick={(e) => e.stopPropagation()}
        >
          {confirming ? (
            <span className="clip-del-confirm">确认删除</span>
          ) : (
            <IconTrash size={14} />
          )}
        </button>
      </div>
    </div>
  );
}

export default memo(ClipRow);
