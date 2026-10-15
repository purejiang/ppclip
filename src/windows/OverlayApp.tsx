import { useEffect, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";

import { api } from "../api";

interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/**
 * 区域截图 overlay。
 *
 * 窗口是**不透明**的，铺满冻结图；「暗化」靠选区四周的 4 块半透明遮罩实现，
 * 选区内部直接露出未变暗的原图 —— 视觉上就是冻结画面，还不用依赖窗口透明。
 */
export default function OverlayApp() {
  const [src, setSrc] = useState<string | null>(null);
  const [rect, setRect] = useState<Rect | null>(null);
  /** 冻结图的物理像素 / CSS 像素。由图片加载后测得，与 DPI 缩放无关。 */
  const [scale, setScale] = useState(1);
  const startRef = useRef<{ x: number; y: number } | null>(null);

  useEffect(() => {
    void (async () => {
      try {
        const path = await api.getOverlayImage();
        setSrc(convertFileSrc(path));
      } catch {
        // 会话已经丢了，直接收工
        void api.cancelCapture();
      }
    })();
  }, []);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") void api.cancelCapture();
    };
    const onContextMenu = (e: MouseEvent) => {
      e.preventDefault();
      void api.cancelCapture();
    };

    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("contextmenu", onContextMenu);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("contextmenu", onContextMenu);
    };
  }, []);

  /** CSS 像素 → 冻结图的物理像素 */
  const toPhysical = (r: Rect, factor: number) => ({
    x: Math.round(r.x * factor),
    y: Math.round(r.y * factor),
    w: Math.round(r.w * factor),
    h: Math.round(r.h * factor),
  });

  const onPointerDown = (e: React.PointerEvent) => {
    if (e.button !== 0) return;
    startRef.current = { x: e.clientX, y: e.clientY };
    setRect({ x: e.clientX, y: e.clientY, w: 0, h: 0 });
  };

  const onPointerMove = (e: React.PointerEvent) => {
    const start = startRef.current;
    if (!start) return;
    setRect({
      x: Math.min(start.x, e.clientX),
      y: Math.min(start.y, e.clientY),
      w: Math.abs(e.clientX - start.x),
      h: Math.abs(e.clientY - start.y),
    });
  };

  const onPointerUp = () => {
    const start = startRef.current;
    startRef.current = null;
    if (!start || !rect) return;

    const physical = toPhysical(rect, scale);
    // 误点一下（没拖出面积）不算取消，允许继续重选
    if (physical.w < 2 || physical.h < 2) {
      setRect(null);
      return;
    }
    void api.finishRegionSelection(physical.x, physical.y, physical.w, physical.h);
  };

  const physical = rect ? toPhysical(rect, scale) : null;
  const dragging = rect !== null && rect.w > 0;

  return (
    <div
      className="overlay"
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
    >
      {src && (
        <img
          className="overlay-image"
          src={src}
          alt=""
          draggable={false}
          onLoad={(e) => {
            const img = e.currentTarget;
            if (img.clientWidth > 0) {
              setScale(img.naturalWidth / img.clientWidth);
            }
            // 首帧就绪后才显示窗口，避免白底闪一下
            void api.overlayReady();
          }}
          onError={() => void api.overlayReady()}
        />
      )}

      {dragging && rect && (
        <>
          <div className="overlay-dim" style={{ left: 0, top: 0, right: 0, height: rect.y }} />
          <div
            className="overlay-dim"
            style={{ left: 0, top: rect.y + rect.h, right: 0, bottom: 0 }}
          />
          <div
            className="overlay-dim"
            style={{ left: 0, top: rect.y, width: rect.x, height: rect.h }}
          />
          <div
            className="overlay-dim"
            style={{ left: rect.x + rect.w, top: rect.y, right: 0, height: rect.h }}
          />
          <div
            className="overlay-selection"
            style={{ left: rect.x, top: rect.y, width: rect.w, height: rect.h }}
          />
          {physical && (
            <div
              className="overlay-size"
              style={{ left: rect.x, top: Math.max(4, rect.y - 24) }}
            >
              {physical.w} × {physical.h}
            </div>
          )}
        </>
      )}

      {!dragging && <div className="overlay-hint">拖拽选择截图区域 · Esc 取消</div>}
    </div>
  );
}
