import { useEffect, useRef } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { api } from "../api";

/** 超过这个位移（CSS 像素）就判定为拖动而不是点击 */
const DRAG_THRESHOLD = 4;
/** 拖动停止后延迟多久保存位置，避免拖动过程中疯狂写配置 */
const SAVE_DELAY_MS = 400;

interface PointerState {
  x: number;
  y: number;
  dragging: boolean;
}

export default function BallApp() {
  const pointer = useRef<PointerState | null>(null);

  useEffect(() => {
    const win = getCurrentWindow();
    // 窗口在 tauri.conf.json 里是隐藏启动的：等前端挂载后再显示，
    // 避免透明窗口先出现一块白底再变圆。
    void win.show();

    let timer: number | undefined;
    const unlisten = win.onMoved(({ payload }) => {
      window.clearTimeout(timer);
      timer = window.setTimeout(() => {
        void api.saveBallPosition(Math.round(payload.x), Math.round(payload.y));
      }, SAVE_DELAY_MS);
    });

    return () => {
      window.clearTimeout(timer);
      void unlisten.then((fn) => fn());
    };
  }, []);

  const onPointerDown = (e: React.PointerEvent) => {
    if (e.button !== 0) return;
    pointer.current = { x: e.screenX, y: e.screenY, dragging: false };
  };

  const onPointerMove = (e: React.PointerEvent) => {
    const state = pointer.current;
    if (!state || state.dragging) return;

    const moved = Math.hypot(e.screenX - state.x, e.screenY - state.y);
    if (moved > DRAG_THRESHOLD) {
      state.dragging = true;
      void getCurrentWindow().startDragging();
    }
  };

  const onPointerUp = () => {
    const state = pointer.current;
    pointer.current = null;
    // 没发生拖动 → 当成点击：开合面板
    if (state && !state.dragging) void api.togglePanel();
  };

  const onContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    void api.showBallMenu();
  };

  return (
    <div className="ball-root">
      <button
        type="button"
        className="ball"
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onContextMenu={onContextMenu}
        title="PPClip：左键开合面板，右键菜单，按住可拖动"
        aria-label="PPClip 悬浮球"
      >
        <span className="ball-glyph">PP</span>
      </button>
    </div>
  );
}
