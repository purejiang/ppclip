import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";

import { api } from "../api";
import { IconCheck, IconClose } from "../components/icons";

interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** 小于这个位移（CSS 像素）算误点，不算「重新画选区」 */
const MIN_DRAG = 3;
/** 小于这个尺寸（冻结图的物理像素）的选区不值得截 */
const MIN_SELECTION = 2;
/** 动作条的估算宽度：贴左边界时改从左往右展开 */
const ACTION_BAR_W = 200;

interface DragState {
  /** 按下点（CSS 像素） */
  x: number;
  y: number;
  /** 按下前已有的选区，用于误点回滚 / 拖动移动 */
  base: Rect | null;
  mode: "draw" | "move";
  /** 本次拖拽真正画出来的选区 */
  current: Rect | null;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), Math.max(min, max));
}

/**
 * 区域截图 overlay。
 *
 * 窗口是**不透明**的，铺满冻结图；「暗化」靠选区四周的 4 块半透明遮罩实现，
 * 选区内部直接露出未变暗的原图 —— 视觉上就是冻结画面，还不用依赖窗口透明。
 *
 * 交互：拖拽画选区 → 选区保留、右下角出现「确定 / 取消」；
 * 再拖一次可重画，在选区里拖动可整体挪位置，双击/Enter 确定，Esc/右键取消。
 */
export default function OverlayApp() {
  const [src, setSrc] = useState<string | null>(null);
  const [rect, setRect] = useState<Rect | null>(null);
  /** 冻结图的物理像素 / CSS 像素。由图片加载后测得，与 DPI 缩放无关。 */
  const [scale, setScale] = useState(1);
  const drag = useRef<DragState | null>(null);
  /** 确定 / 取消只能触发一次 */
  const fired = useRef(false);

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

  /** CSS 像素 → 冻结图的物理像素 */
  const toPhysical = (r: Rect, factor: number) => ({
    x: Math.round(r.x * factor),
    y: Math.round(r.y * factor),
    w: Math.round(r.w * factor),
    h: Math.round(r.h * factor),
  });

  const physical = useMemo(
    () => (rect ? toPhysical(rect, scale) : null),
    [rect, scale],
  );
  const ready =
    physical !== null &&
    physical.w >= MIN_SELECTION &&
    physical.h >= MIN_SELECTION;

  const confirm = useCallback(() => {
    if (!physical || !ready || fired.current) return;
    fired.current = true;
    void api.finishRegionSelection(
      physical.x,
      physical.y,
      physical.w,
      physical.h,
    );
  }, [physical, ready]);

  const cancel = useCallback(() => {
    if (fired.current) return;
    fired.current = true;
    void api.cancelCapture();
  }, []);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") cancel();
      else if (e.key === "Enter") confirm();
    };
    const onContextMenu = (e: MouseEvent) => {
      e.preventDefault();
      cancel();
    };

    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("contextmenu", onContextMenu);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("contextmenu", onContextMenu);
    };
  }, [cancel, confirm]);

  const onPointerDown = (e: React.PointerEvent) => {
    if (e.button !== 0) return;
    const inside =
      rect !== null &&
      e.clientX >= rect.x &&
      e.clientX <= rect.x + rect.w &&
      e.clientY >= rect.y &&
      e.clientY <= rect.y + rect.h;

    drag.current = {
      x: e.clientX,
      y: e.clientY,
      base: rect,
      mode: inside ? "move" : "draw",
      current: null,
    };
    // 在选区外按下 = 开始画新的（旧选区先留着，误点时会回滚）
    if (!inside) setRect({ x: e.clientX, y: e.clientY, w: 0, h: 0 });
  };

  const onPointerMove = (e: React.PointerEvent) => {
    const state = drag.current;
    if (!state) return;
    const dx = e.clientX - state.x;
    const dy = e.clientY - state.y;

    // 在选区里拖动 = 整体挪位置
    if (state.mode === "move" && state.base) {
      const next: Rect = {
        x: clamp(state.base.x + dx, 0, window.innerWidth - state.base.w),
        y: clamp(state.base.y + dy, 0, window.innerHeight - state.base.h),
        w: state.base.w,
        h: state.base.h,
      };
      state.current = next;
      setRect(next);
      return;
    }

    // 还没真的拖起来：不动选区，免得手抖把已有选区弄没
    if (Math.abs(dx) < MIN_DRAG && Math.abs(dy) < MIN_DRAG) return;

    const next: Rect = {
      x: Math.min(state.x, e.clientX),
      y: Math.min(state.y, e.clientY),
      w: Math.abs(dx),
      h: Math.abs(dy),
    };
    state.current = next;
    setRect(next);
  };

  const onPointerUp = () => {
    const state = drag.current;
    drag.current = null;
    if (!state) return;
    // 只是点了一下（没画出选区）→ 保留原来的选区，没有就保持空
    if (state.mode === "draw" && state.current === null) setRect(state.base);
    // 画出来的选区直接留着，等用户点「确定」
  };

  /** 动作条贴着选区右下角；贴到屏幕左/下边界时换到另一侧 */
  const actionBarStyle = (r: Rect): React.CSSProperties => {
    const viewW = window.innerWidth;
    const viewH = window.innerHeight;
    const below = r.y + r.h + 10;
    const top = below + 40 > viewH ? Math.max(8, r.y - 44) : below;
    const right = r.x + r.w;

    if (right - ACTION_BAR_W < 8) {
      return { top, left: clamp(r.x, 8, Math.max(8, viewW - ACTION_BAR_W - 8)) };
    }
    return { top, left: right, transform: "translateX(-100%)" };
  };

  const drawing =
    rect !== null && rect.w > 0 && rect.h > 0 ? rect : null;

  return (
    <div
      className="overlay"
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onDoubleClick={() => confirm()}
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

      {drawing && (
        <>
          <div
            className="overlay-dim"
            style={{ left: 0, top: 0, right: 0, height: drawing.y }}
          />
          <div
            className="overlay-dim"
            style={{ left: 0, top: drawing.y + drawing.h, right: 0, bottom: 0 }}
          />
          <div
            className="overlay-dim"
            style={{
              left: 0,
              top: drawing.y,
              width: drawing.x,
              height: drawing.h,
            }}
          />
          <div
            className="overlay-dim"
            style={{
              left: drawing.x + drawing.w,
              top: drawing.y,
              right: 0,
              height: drawing.h,
            }}
          />
          <div
            className="overlay-selection"
            style={{
              left: drawing.x,
              top: drawing.y,
              width: drawing.w,
              height: drawing.h,
            }}
          />
          {physical && (
            <div
              className="overlay-size"
              style={{ left: drawing.x, top: Math.max(4, drawing.y - 24) }}
            >
              {physical.w} × {physical.h}
            </div>
          )}
        </>
      )}

      {ready && drawing && (
        <div
          className="overlay-actions"
          style={actionBarStyle(drawing)}
          onPointerDown={(e) => e.stopPropagation()}
          onDoubleClick={(e) => e.stopPropagation()}
        >
          <button
            type="button"
            className="overlay-btn overlay-btn--primary"
            onClick={confirm}
          >
            <IconCheck size={14} />
            确定 (Enter)
          </button>
          <button type="button" className="overlay-btn" onClick={cancel}>
            <IconClose size={14} />
            取消 (Esc)
          </button>
        </div>
      )}

      <div className="overlay-hint">
        {ready
          ? "拖动选区可移动 · 重新拖拽可重选 · 双击或 Enter 确定 · Esc 取消"
          : "拖拽选择截图区域 · Esc 取消"}
      </div>
    </div>
  );
}
