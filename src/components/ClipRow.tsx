import { memo } from "react";

import { assetUrl } from "../api";
import type { Clip } from "../types";

interface Props {
  clip: Clip;
  dataDir: string;
  active: boolean;
  onClick: (clip: Clip) => void;
  onContextMenu: (clip: Clip) => void;
}

function formatBytes(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  if (bytes >= 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${bytes} B`;
}

/** 一条历史记录：文本显示预览，图片显示缩略图 */
function ClipRow({ clip, dataDir, active, onClick, onContextMenu }: Props) {
  const thumb =
    clip.kind === "image" && clip.thumbRel
      ? assetUrl(dataDir, clip.thumbRel)
      : null;

  return (
    <button
      type="button"
      className={`clip-row${active ? " is-active" : ""}`}
      onClick={() => onClick(clip)}
      onContextMenu={(e) => {
        e.preventDefault();
        onContextMenu(clip);
      }}
    >
      <span className={`clip-kind clip-kind--${clip.kind}`}>
        {clip.kind === "image" ? "图" : "文"}
      </span>

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

      {clip.source === "screenshot" && <span className="clip-badge">截图</span>}
    </button>
  );
}

export default memo(ClipRow);
