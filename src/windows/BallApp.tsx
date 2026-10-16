import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { api } from "../api";

/** 超过这个位移（CSS 像素）就判定为拖动而不是点击 */
const DRAG_THRESHOLD = 4;
/** 拖动停止后延迟多久保存位置，避免拖动过程中疯狂写配置 */
const SAVE_DELAY_MS = 400;
/**
 * 鼠标移开后延迟多久收回枢纽态。
 * 退场动画得在「窗口还是 240×240」时播完，否则四个按钮会被缩小的窗口直接裁掉。
 */
const COLLAPSE_DELAY_MS = 160;
/**
 * 鼠标停在球上满这么久才展开枢纽态。
 *
 * 为什么要有延迟、以及为什么这么短：
 * - 展开不是纯视觉 —— 它会撑出一块 240×240 的置顶透明区，把桌面点击吃掉，
 *   所以必须区分「鼠标路过」和「真的想用」；停住不动本身就是最强的意图信号。
 * - 人停住不动 0.3 秒已经不像「路过」了，再往上加基本只是在给确定的用户添延迟。
 * - 100ms 是「感觉瞬时」的门槛，1s 是「流程被打断」的门槛，悬停类交互的做法都在
 *   150~400ms 这一带（Windows 自己的鼠标悬停时间默认就是 400ms 左右）。
 * - 展开错了也很便宜：移开 160ms 就收回去，中途再进来还能取消。
 */
const HOVER_OPEN_DELAY_MS = 300;

interface PointerState {
  x: number;
  y: number;
  dragging: boolean;
}

/**
 * 悬浮球只有两种形态：
 *
 * - **收起态**：52px 的小球，窗口就是球那么大（Windows 上两侧还会多出透明区）。
 * - **枢纽态**：鼠标停住 0.3 秒后窗口撑成 240×240，露出中心球 + 四个环绕按钮；
 *   鼠标一离开就收回去。
 *
 * 窗口尺寸的真身由 Rust 侧 `ballhub` 管（要把球心钉在原地），这里只负责
 * 「什么时候该展开」和 CSS 状态；`ball:hub` 事件用来兜底同步（截图前藏窗口时
 * 前端根本收不到 mouseleave）。
 */
export default function BallApp() {
  const pointer = useRef<PointerState | null>(null);
  const collapseTimer = useRef<number | undefined>(undefined);
  /** 悬停展开的倒计时（中途移开 / 开始拖动就清掉） */
  const openTimer = useRef<number | undefined>(undefined);
  /** 鼠标是否还在窗口里：展开是异步的，回来时得再确认一次才点亮 */
  const hovered = useRef(false);
  const [open, setOpen] = useState(false);

  /** 收起枢纽态：改 CSS 状态，并把可能存在的「聚焦滚动」抹掉 */
  const markClosed = () => {
    setOpen(false);
    // 窗口一缩小，画面外那个「曾经被聚焦的环绕按钮」会让浏览器把画布滚过去
    // （overflow:hidden 照样能被程序性滚动），球心就对不上窗口中心了 ——
    // 所以顺手 blur + 把滚动位置拉回原点
    const active = document.activeElement;
    if (active instanceof HTMLElement && active !== document.body) active.blur();
    document.documentElement.scrollTop = 0;
    document.documentElement.scrollLeft = 0;
    document.body.scrollTop = 0;
    document.body.scrollLeft = 0;
  };

  useEffect(() => {
    const win = getCurrentWindow();
    // 窗口在 tauri.conf.json 里是隐藏启动的：等前端挂载后再显示，
    // 避免透明窗口先出现一块白底再变圆。
    // 走应用自己的命令而不是 `win.show()`：`core:default` 里**没有**
    // `allow-show`，用 JS 的窗口 API 会被 ACL 拒掉，而 `void` 会把错误吞掉 ——
    // 表现就是「启动后什么都不出现、也没有托盘/任务栏图标」，再也进不来。
    void api.showBall().catch((err) => console.error("显示悬浮球失败", err));

    let timer: number | undefined;
    const unlistenMoved = win.onMoved(({ payload }) => {
      window.clearTimeout(timer);
      timer = window.setTimeout(() => {
        void api.saveBallPosition(Math.round(payload.x), Math.round(payload.y));
      }, SAVE_DELAY_MS);
    });

    // Rust 是枢纽态的真相：它会把窗口撑大 / 缩回，这里跟着改 CSS
    const unlistenHub = api.onBallHub((expanded) => {
      window.clearTimeout(collapseTimer.current);
      if (expanded) setOpen(true);
      else markClosed();
    });

    return () => {
      window.clearTimeout(timer);
      window.clearTimeout(collapseTimer.current);
      window.clearTimeout(openTimer.current);
      void unlistenMoved.then((fn) => fn());
      void unlistenHub.then((fn) => fn());
    };
  }, []);

  /** 真正去撑窗：窗口先撑开，再放弹出动画（窗口没撑开时按钮会被裁掉） */
  const expandHub = () => {
    if (open) return;
    void api
      .setBallExpanded(true)
      .then(() => {
        if (hovered.current) setOpen(true);
      })
      .catch((err) => console.error("展开悬浮球失败", err));
  };

  /** 鼠标进窗口：停满 HOVER_OPEN_DELAY_MS 才展开，中途移开就作废 */
  const onHoverEnter = () => {
    hovered.current = true;
    window.clearTimeout(collapseTimer.current);
    window.clearTimeout(openTimer.current);
    if (open) return;
    openTimer.current = window.setTimeout(() => {
      if (hovered.current) expandHub();
    }, HOVER_OPEN_DELAY_MS);
  };

  /** 收回枢纽态；默认延迟一点，让退场动画播完再缩窗 */
  const collapseHub = (immediate = false) => {
    hovered.current = false;
    // 还没到点就走了：倒计时直接作废，不要补展开
    window.clearTimeout(openTimer.current);
    window.clearTimeout(collapseTimer.current);
    const apply = () => {
      markClosed();
      void api
        .setBallExpanded(false)
        .catch((err) => console.error("收回悬浮球失败", err));
    };
    if (immediate) apply();
    else collapseTimer.current = window.setTimeout(apply, COLLAPSE_DELAY_MS);
  };

  /** 卫星按钮：先收回枢纽态（截图的冻结帧不能把这圈按钮拍进去）再执行动作 */
  const runSatellite = (action: () => Promise<void>) => {
    hovered.current = false;
    window.clearTimeout(openTimer.current);
    window.clearTimeout(collapseTimer.current);
    markClosed();
    void api
      .setBallExpanded(false)
      .catch((err) => console.error("收回悬浮球失败", err))
      .then(action)
      .catch((err) => console.error("悬浮球按钮执行失败", err));
  };

  const onPointerDown = (e: React.PointerEvent) => {
    if (e.button !== 0) return;
    pointer.current = { x: e.screenX, y: e.screenY, dragging: false };
  };

  const onPointerMove = (e: React.PointerEvent) => {
    const state = pointer.current;
    if (!state || state.dragging) return;

    const moved = Math.hypot(e.screenX - state.x, e.screenY - state.y);
    if (moved <= DRAG_THRESHOLD) return;

    state.dragging = true;
    hovered.current = false;
    // 压着球拖走时，那个还没到点的「悬停展开」倒计时也得作废，
    // 否则松手后手还停在球上，倒计时一到又自己弹开
    window.clearTimeout(openTimer.current);
    window.clearTimeout(collapseTimer.current);
    markClosed();
    // 先缩回小球再开始拖：拖着 240×240 的大窗跑，松手存下来的位置会偏
    // （Rust 侧也会忽略枢纽态下的位置上报，这里只是让拖动本身别带着大窗）
    void api
      .setBallExpanded(false)
      .catch((err) => console.error("拖动前收回悬浮球失败", err))
      .then(() => getCurrentWindow().startDragging());
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
    <div
      className={open ? "ball-root is-open" : "ball-root"}
      onPointerEnter={onHoverEnter}
      onPointerLeave={() => collapseHub()}
    >
      <div className="ball-hub">
        <span className="ball-halo" aria-hidden="true" />
        <span className="ball-orbit" aria-hidden="true" />

        <button
          type="button"
          className="ball-sat ball-sat-up"
          title="区域截图"
          aria-label="区域截图"
          onClick={() => runSatellite(api.startRegionCapture)}
        >
          <ScanIcon />
        </button>
        <button
          type="button"
          className="ball-sat ball-sat-right"
          title="全屏截图"
          aria-label="全屏截图"
          onClick={() => runSatellite(api.captureFullscreen)}
        >
          <ScreenIcon />
        </button>
        <button
          type="button"
          className="ball-sat ball-sat-down"
          title="隐藏悬浮球（重新启动 PPClip 可找回）"
          aria-label="隐藏悬浮球"
          onClick={() => runSatellite(api.hideBall)}
        >
          <HideIcon />
        </button>
        <button
          type="button"
          className="ball-sat ball-sat-left"
          title="剪贴板历史"
          aria-label="打开剪贴板历史面板"
          onClick={() => runSatellite(api.togglePanel)}
        >
          <ListIcon />
        </button>

        <button
          type="button"
          className="ball"
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={onPointerUp}
          onContextMenu={onContextMenu}
          // 展开后要把 title 摘掉：光标还停在球上，原生提示会在枢纽态弹开之后
          // 才冒出来，正好糊在那圈按钮上
          title={open ? undefined : "PPClip：左键开合面板，右键菜单，按住可拖动；悬停展开快捷按钮"}
          aria-label="PPClip 悬浮球"
        >
          <span className="ball-glyph">PP</span>
          <span className="ball-face" aria-hidden="true">
            <ClipIcon />
            <span className="ball-label">PPCLIP</span>
          </span>
        </button>
      </div>
    </div>
  );
}

/** 图标统一 24 网格、细描边，跟界面其余部分的手写 SVG 一个风格 */
const ICON_PROPS = {
  width: 20,
  height: 20,
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.7,
  strokeLinecap: "round",
  strokeLinejoin: "round",
} as const;

/** 区域截图：取景框四角 + 中心小框 */
function ScanIcon() {
  return (
    <svg {...ICON_PROPS} aria-hidden="true">
      <path d="M4 9V6.5A2.5 2.5 0 0 1 6.5 4H9" />
      <path d="M15 4h2.5A2.5 2.5 0 0 1 20 6.5V9" />
      <path d="M20 15v2.5a2.5 2.5 0 0 1-2.5 2.5H15" />
      <path d="M9 20H6.5A2.5 2.5 0 0 1 4 17.5V15" />
      <rect x="9.4" y="9.4" width="5.2" height="5.2" rx="1.2" />
    </svg>
  );
}

/** 全屏截图：显示器 */
function ScreenIcon() {
  return (
    <svg {...ICON_PROPS} aria-hidden="true">
      <rect x="3" y="4.6" width="18" height="12.6" rx="2" />
      <path d="M12 17.2v3.3" />
      <path d="M8.8 20.5h6.4" />
    </svg>
  );
}

/** 隐藏悬浮球：收进托盘 */
function HideIcon() {
  return (
    <svg {...ICON_PROPS} aria-hidden="true">
      <path d="M12 4.2v9.2" />
      <path d="M8.2 9.7 12 13.5l3.8-3.8" />
      <path d="M4.8 19.6h14.4" />
    </svg>
  );
}

/** 剪贴板历史：列表 */
function ListIcon() {
  return (
    <svg {...ICON_PROPS} aria-hidden="true">
      <path d="M9.2 6.6h10.6" />
      <path d="M9.2 12h10.6" />
      <path d="M9.2 17.4h10.6" />
      <circle cx="4.9" cy="6.6" r="1.1" />
      <circle cx="4.9" cy="12" r="1.1" />
      <circle cx="4.9" cy="17.4" r="1.1" />
    </svg>
  );
}

/** 中心：剪贴板 + 出箭头（PPClip 干的就是「把剪贴板里的东西拿出来用」） */
function ClipIcon() {
  return (
    <svg {...ICON_PROPS} width={26} height={26} aria-hidden="true">
      <rect x="3.4" y="5.2" width="12.4" height="15" rx="2.6" />
      <path d="M7.6 3.4h4a.8.8 0 0 1 .8.8v1.6H6.8V4.2a.8.8 0 0 1 .8-.8Z" />
      <path d="M6.4 11.2h6.4" />
      <path d="M16.6 12.7h3.8" />
      <path d="M18.3 10.6 20.4 12.7l-2.1 2.1" />
    </svg>
  );
}
