import { memo } from "react";

import { assetUrl } from "../api";
import { IconCopy, IconFile, IconFloatCard, IconImage, IconText, IconTrash } from "./icons";
import type { Clip } from "../types";

interface Props {
  clip: Clip;
  dataDir: string;
  active: boolean;
  /** 当前时间戳：相对时间（几分钟前）要跟着走，由面板每分钟刷一次 */
  now: number;
  /** 这条正在等第二次点击确认删除 */
  confirming: boolean;
  /** 这条已经贴在桌面上了（浮窗按钮点亮，再点=取消贴图） */
  pinned: boolean;
  onClick: (clip: Clip) => void;
  onCopy: (clip: Clip) => void;
  onPin: (clip: Clip) => void;
  onDelete: (clip: Clip) => void;
  onContextMenu: (clip: Clip) => void;
}

/**
 * 字节数：MB 一位小数、100 KB 以上取整、更小的保留一位小数。
 *
 * 大文件稿子写 `2.4 MB`；十几 KB 的文件写成「5 KB」会让人觉得尺寸不准（实际 4.5 KB），
 * 所以 100 KB 以下保留一位小数，`240 KB` 这种仍然干净。
 */
function formatBytes(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  if (bytes >= 100 * 1024) return `${Math.round(bytes / 1024)} KB`;
  if (bytes >= 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${bytes} B`;
}

/**
 * 文件条目的扩展名标签（大写）：`C:\a\Q4_计划账单.pdf` → `PDF`。
 * 走完整路径而不是列表预览（预览会被截断，长文件名会把扩展名截掉）；
 * 没有扩展名 / 结尾是点 / 隐藏文件（`.gitignore`）一律给空串，不显示半个标签。
 */
function fileExt(path: string): string {
  const name = path.split(/[\\/]/).pop() ?? path;
  const cut = name.lastIndexOf(".");
  if (cut <= 0 || cut === name.length - 1) return "";
  return name.slice(cut + 1).toUpperCase();
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
  onClick,
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
  const isFile = clip.kind === "file";
  const fileCount = clip.files?.length ?? 0;
  const size = formatBytes(clip.bytes);
  const when = formatWhen(clip.lastUsedAt, now);
  /** 单个文件才挂类型标签（多个文件时那格留给「N 个文件」） */
  const extChip =
    isFile && fileCount === 1 ? fileExt(clip.files?.[0] ?? clip.preview) : "";
  /** 第二行 meta 的第一段：各类型各说各的重点 */
  const metaLead = isImage
    ? `${clip.width ?? "?"}×${clip.height ?? "?"} · ${size}`
    : isFile
      ? // 单个文件：类型标签已经在标题旁边，这里只说大小（照稿子的 `PDF 2.4 MB`）
        fileCount > 1
        ? `${fileCount} 个文件 · ${size}`
        : size
      : `${clip.text?.length ?? clip.preview.length} chars`;

  return (
    <div
      role="button"
      tabIndex={0}
      className={`clip-row${active ? " is-active" : ""}`}
      title={`单击查看完整内容 · 右侧按钮复制${isFile ? " / 删除" : " / 贴图 / 删除"} · 右键更多\n创建：${formatFull(clip.createdAt)}\n最后使用：${formatFull(clip.lastUsedAt)}`}
      onClick={() => onClick(clip)}
      onContextMenu={(e) => {
        e.preventDefault();
        onContextMenu(clip);
      }}
    >
      <div className="clip-row-body">
        {/* 类型块 40×40：三种类型图标按**视觉重量**配尺寸（方形相框比三行文字重，
            所以图片给 20、文本与折角纸给 18），不是随手写的数字 —— 改之前先并排看一眼 */}
        <div
          className={`clip-kind${isImage ? " clip-kind--image" : ""}`}
          title={isImage ? "图片" : isFile ? "文件" : "文本"}
        >
          {isImage ? (
            <IconImage size={20} />
          ) : isFile ? (
            <IconFile size={18} />
          ) : (
            <IconText size={18} />
          )}
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
              {extChip && (
                <span className="clip-badge" title={`${extChip} 文件`}>
                  {extChip}
                </span>
              )}
              {clip.source === "screenshot" && (
                <span className="clip-badge">截图</span>
              )}
            </div>
          )}

          <div className="clip-meta">
            <div className="clip-meta-line">
              {metaLead}
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
        >
          <IconCopy size={14} />
        </button>

        {/* 文件条目没有可画的像素，贴到桌面只会是一屏路径字符串 —— 后端也会拒，这里干脆不给入口 */}
        {!isFile && (
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
          >
            <IconFloatCard size={14} />
          </button>
        )}

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
            <IconTrash size={14} />
          )}
        </button>
      </div>
    </div>
  );
}

export default memo(ClipRow);
