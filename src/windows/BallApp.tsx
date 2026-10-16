import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { api } from "../api";
import {
  IconClipboardArrow,
  IconHide,
  IconList,
  IconMonitor,
  IconScan,
} from "../components/icons";

/** 超过这个位移（CSS 像素）就判定为拖动而不是点击 */
const DRAG_THRESHOLD = 4;
/** 拖动停止后延迟多久保存位置，避免拖动过程中疯狂写配置 */
const SAVE_DELAY_MS = 400;
/**
 * 鼠标移开后延迟多久收回轮盘。
 * 退场动画得在「窗口还是 200×200」时播完，否则按钮会被缩小的窗口直接裁掉。
 */
const COLLAPSE_DELAY_MS = 160;
/**
 * 鼠标停在胶囊 / 球上满这么久才展开 HUD 轮盘。
 *
 * 为什么要有延迟、以及为什么这么短：
 * - 展开不是纯视觉 —— 它会撑出一块 200×200 的置顶透明区，把桌面点击吃掉，
 *   所以必须区分「鼠标路过」和「真的想用」；停住不动本身就是最强的意图信号。
 * - 人停住不动 0.3 秒已经不像「路过」了，再往上加基本只是在给确定的用户添延迟。
 * - 100ms 是「感觉瞬时」的门槛，1s 是「流程被打断」的门槛，悬停类交互的做法都在
 *   150~400ms 这一带（Windows 自己的鼠标悬停时间默认就是 400ms 左右）。
 */
const HOVER_OPEN_DELAY_MS = 300;

/** 三种形态，和 Rust 侧 `ballhub::state_name` 的取值一一对应 */
type BallState = "tab" | "orb" | "wheel";

interface PointerState {
  x: number;
  y: number;
  dragging: boolean;
}

/**
 * 悬浮入口的三种形态（照 stitch 稿「桌面悬浮球三态规范」）：
 *
 * - **贴边胶囊（Collapsed）**：16×64 的对称胶囊，拖到屏幕左 / 右边缘自动吸附；
 *   三颗小点 + 顶部计数角标；点一下唤出常规态（稿子的说法），拖走就离开边缘。
 * - **常规球（Floating Orb）**：52×52 的渐变环圆盘 + 呼吸光晕 + 右上计数角标，
 *   桌面任意位置，左键开合面板。
 * - **展开态（HUD Wheel）**：鼠标停住 0.3 秒后窗口撑成 200×200，露出玻璃圆盘、
 *   虚线轨道、四个卫星按钮和中心圆盘；鼠标一离开就收回。
 *
 * 窗口尺寸的真身由 Rust 侧 `ballhub` 管（形状中心要钉在原地），这里只负责
 * 「什么时候该展开」和 CSS 状态；`ball:state` 事件用来兜底同步（截图前藏窗口时
 * 前端根本收不到 mouseleave）。
 */
export default function BallApp() {
  const pointer = useRef<PointerState | null>(null);
  const collapseTimer = useRef<number | undefined>(undefined);
  /** 悬停展开的倒计时（中途移开 / 开始拖动就清掉） */
  const openTimer = useRef<number | undefined>(undefined);
  /** 鼠标是否还在窗口里：展开是异步的，回来时得再确认一次才点亮 */
  const hovered = useRef(false);
  const [state, setState] = useState<BallState>("orb");
  /** 收起态的形态（tab / orb）：收回轮盘时照它还原，免得先闪一下常规球 */
  const formRef = useRef<"tab" | "orb">("orb");
  /** 悬浮球不透明度（%，来自设置） */
  const [opacity, setOpacity] = useState(100);
  /** 面板关着的时候新收进来的剪贴板条数（角标） */
  const [fresh, setFresh] = useState(0);
  const panelVisible = useRef(false);
  const open = state === "wheel";

  /** 收回轮盘：改 CSS 状态，并把可能存在的「聚焦滚动」抹掉 */
  const markClosed = () => {
    setState(formRef.current);
    // 窗口一缩小，画面外那个「曾经被聚焦的卫星按钮」会让浏览器把画布滚过去
    // （overflow:hidden 照样能被程序性滚动），形状中心就对不上窗口中心了 ——
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
    // 避免透明窗口先出现一块白底再变形状。
    // 走应用自己的命令而不是 `win.show()`：`core:default` 里**没有**
    // `allow-show`，用 JS 的窗口 API 会被 ACL 拒掉，而 `void` 会把错误吞掉 ——
    // 表现就是「启动后什么都不出现、也没有托盘/任务栏图标」，再也进不来。
    void api.showBall().catch((err) => console.error("显示悬浮球失败", err));

    // 不透明度是设置项：挂载时读一次，之后由 settings:changed 驱动
    void api
      .getSettings()
      .then((s) => setOpacity(s.ballOpacity))
      .catch((err) => console.error("读取悬浮球设置失败", err));

    // 吸附 / 形态切换发生在启动阶段（早于这里挂监听），所以主动问一次当前形态
    void api
      .ballState()
      .then((name) => {
        if (name === "tab" || name === "orb") formRef.current = name;
        setState(name === "tab" || name === "wheel" ? name : "orb");
      })
      .catch((err) => console.error("读取悬浮球形态失败", err));

    let timer: number | undefined;
    const unlistenMoved = win.onMoved(({ payload }) => {
      window.clearTimeout(timer);
      timer = window.setTimeout(() => {
        void api.saveBallPosition(Math.round(payload.x), Math.round(payload.y));
      }, SAVE_DELAY_MS);
    });

    // Rust 是形态的真相：它会把窗口撑大 / 缩回 / 吸附，这里跟着改 CSS
    const unlistenState = api.onBallState((name) => {
      window.clearTimeout(collapseTimer.current);
      if (name === "wheel") {
        setState("wheel");
        return;
      }
      if (name === "tab" || name === "orb") formRef.current = name;
      setState((current) => (current === "wheel" ? current : formRef.current));
    });
    const unlistenSettings = api.onSettingsChanged(() => {
      void api
        .getSettings()
        .then((s) => setOpacity(s.ballOpacity))
        .catch(() => undefined);
    });
    // 面板一露出来就把「新条目」清零：用户已经看到了
    const unlistenPanel = api.onPanelVisible((visible) => {
      panelVisible.current = visible;
      if (visible) setFresh(0);
    });
    // 面板关着时新来的剪贴板才计数，免得把面板里自己复制的那几条也算成「新」
    const unlistenClips = api.onClipsChanged(() => {
      if (!panelVisible.current) setFresh((count) => count + 1);
    });

    return () => {
      window.clearTimeout(timer);
      window.clearTimeout(collapseTimer.current);
      window.clearTimeout(openTimer.current);
      void unlistenMoved.then((fn) => fn());
      void unlistenState.then((fn) => fn());
      void unlistenSettings.then((fn) => fn());
      void unlistenPanel.then((fn) => fn());
      void unlistenClips.then((fn) => fn());
    };
  }, []);

  /** 真正去撑窗：窗口先撑开，再放弹出动画（窗口没撑开时按钮会被裁掉） */
  const expandHub = () => {
    if (open) return;
    void api
      .setBallExpanded(true)
      .then(() => {
        if (hovered.current) setState("wheel");
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

  /** 收回轮盘；默认延迟一点，让退场动画播完再缩窗 */
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

  /** 卫星按钮：先收回轮盘（截图的冻结帧不能把这圈按钮拍进去）再执行动作 */
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
    const pointerState = pointer.current;
    if (!pointerState || pointerState.dragging) return;

    const moved = Math.hypot(e.screenX - pointerState.x, e.screenY - pointerState.y);
    if (moved <= DRAG_THRESHOLD) return;

    pointerState.dragging = true;
    hovered.current = false;
    // 压着形状拖走时，那个还没到点的「悬停展开」倒计时也得作废，
    // 否则松手后手还停在上头，倒计时一到又自己弹开
    window.clearTimeout(openTimer.current);
    window.clearTimeout(collapseTimer.current);
    markClosed();
    // 先收回轮盘再开始拖：拖着 200×200 的大窗跑，松手存下来的位置会偏
    // （Rust 侧也会忽略展开态下的位置上报，这里只是让拖动本身别带着大窗）
    void api
      .setBallExpanded(false)
      .catch((err) => console.error("拖动前收回悬浮球失败", err))
      .then(() => getCurrentWindow().startDragging());
  };

  /** 松手后 Rust 会按落点决定形态（贴边就吸成胶囊），前端只等 `ball:state` */
  const onPointerUp = () => {
    pointer.current = null;
  };

  /** 常规球：单击开合面板（顺手收回轮盘，别让它从面板边缘探出来） */
  const onOrbClick = () => {
    runSatellite(api.togglePanel);
  };

  /** 贴边胶囊：单击唤出常规态（稿子 State 01：「hover / click 唤出常规态」） */
  const onTabClick = () => {
    hovered.current = false;
    window.clearTimeout(openTimer.current);
    window.clearTimeout(collapseTimer.current);
    markClosed();
    void api
      .setBallExpanded(false)
      .catch(() => undefined)
      .then(() => api.setBallDocked(false))
      .then(() => setState("orb"))
      .catch((err) => console.error("唤出常规态失败", err));
  };

  const onContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    void api.showBallMenu();
  };

  const pointerHandlers = {
    onPointerDown,
    onPointerMove,
    onPointerUp,
    onContextMenu,
  };

  return (
    <div
      className={`ball-root is-${state}`}
      style={{ opacity: opacity / 100 }}
      onPointerEnter={onHoverEnter}
      onPointerLeave={() => collapseHub()}
    >
      <div className="ball-hub">
        {/* 展开态：200×200 的 HUD 轮盘 */}
        <div className="ball-wheel" aria-hidden={!open}>
          <span className="ball-orbit" aria-hidden="true" />

          <button
            type="button"
            className="ball-sat ball-sat-up"
            title="区域截图"
            aria-label="区域截图"
            onClick={() => runSatellite(api.startRegionCapture)}
          >
            <IconScan size={16} />
          </button>
          <button
            type="button"
            className="ball-sat ball-sat-right"
            title="全屏截图"
            aria-label="全屏截图"
            onClick={() => runSatellite(api.captureFullscreen)}
          >
            <IconMonitor size={16} />
          </button>
          <button
            type="button"
            className="ball-sat ball-sat-down"
            title="隐藏悬浮球（重新启动 PPClip 可找回）"
            aria-label="隐藏悬浮球"
            onClick={() => runSatellite(api.hideBall)}
          >
            <IconHide size={16} />
          </button>
          <button
            type="button"
            className="ball-sat ball-sat-left"
            title="剪贴板历史"
            aria-label="打开剪贴板历史面板"
            onClick={() => runSatellite(api.togglePanel)}
          >
            <IconList size={16} />
          </button>

          <button
            type="button"
            className="ball-core"
            // 展开后要把 title 摘掉：光标还停在上头，原生提示会在轮盘弹开之后
            // 才冒出来，正好糊在那圈按钮上
            title={open ? undefined : "PPClip：左键开合面板，右键菜单，按住可拖动"}
            aria-label="PPClip 悬浮入口"
            {...pointerHandlers}
          >
            <span className="ball-core-inner">
              <IconClipboardArrow size={18} />
              <span className="ball-core-label">PPCLIP</span>
            </span>
          </button>
        </div>

        {/* 常规态：52×52 悬浮球 */}
        <button
          type="button"
          className="ball-orb"
          title="PPClip：左键开合面板，右键菜单，按住可拖动；悬停展开快捷按钮"
          aria-label="PPClip 悬浮球"
          onClick={onOrbClick}
          {...pointerHandlers}
        >
          <span className="ball-orb-halo" aria-hidden="true" />
          <span className="ball-orb-body">
            <span className="ball-orb-inner">
              <IconClipboardArrow size={22} />
            </span>
          </span>
          {fresh > 0 && (
            <span className="ball-badge ball-badge--corner" aria-label={`${fresh} 条新剪贴板`}>
              {fresh > 99 ? "99+" : fresh}
            </span>
          )}
        </button>

        {/* 收起边缘态：16×64 贴边胶囊 */}
        <button
          type="button"
          className="ball-tab"
          title="PPClip：单击唤出常规态，按住可拖离边缘；悬停展开快捷按钮"
          aria-label="PPClip 悬浮入口（贴边）"
          onClick={onTabClick}
          {...pointerHandlers}
        >
          <span className="ball-tab-dots" aria-hidden="true">
            <span className="ball-tab-dot ball-tab-dot--pulse" />
            <span className="ball-tab-dot ball-tab-dot--dim" />
            <span className="ball-tab-dot" />
          </span>
          {fresh > 0 && (
            <span className="ball-badge ball-badge--top" aria-label={`${fresh} 条新剪贴板`}>
              {fresh > 99 ? "99+" : fresh}
            </span>
          )}
        </button>
      </div>
    </div>
  );
}
