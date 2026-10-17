import { useEffect, useRef, useState, type CSSProperties } from "react";
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
/**
 * 拖动结束后这段时间里不算「悬停意图」。
 *
 * 松手那一瞬鼠标必然还停在球上，立刻当成悬停就会把刚吸上的胶囊打回圆球 ——
 * 贴边留不住，形态来回切还会让窗口位置跳一下（实测日志：同一秒 tab 紧跟 orb）。
 * 但也不能「直到鼠标离开才恢复」：拖完手没动就等于悬停彻底失灵。
 * 600ms 够看清「已经贴边」，又不至于让想立刻唤出的人等太久。
 */
const POST_DRAG_HOVER_GUARD_MS = 600;

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
 *   三颗小点 + 顶部计数角标。
 * - **常规球（Floating Orb）**：52×52 的 2px 渐变环圆盘（不发光晕）+ 右上计数角标，
 *   桌面任意位置。
 * - **展开态（HUD Wheel）**：玻璃圆盘、虚线轨道、四个卫星按钮、中心圆盘。
 *
 * 交互（三段式，鼠标不用点就能一路走到底）：
 * 1. 鼠标放到**贴边胶囊**上 → 立刻变**常规态**（不展开）；
 * 2. 鼠标在**常规态**上停满 0.3 秒 → 展开成**轮盘**，移开就收回；
 * 3. 两种收起态的**单击**都直接开合主面板（拖动则是拖走，右键出原生菜单）。
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
  /** 「胶囊 → 常规态」这次切换是否还在飞（防止 pointermove 连发重复调用命令） */
  const undocking = useRef(false);
  /**
   * 展开瞬间窗口会整体重定位（贴边时轮盘要钳进屏幕，窗口左移最多 100+ px）。
   * WebView2 在这种位移后会误派一次 `pointerleave`，而指针其实一直在轮盘上 ——
   * 不挡掉的话轮盘刚展开就被收回（实测：展开后 1 秒内 → orb）。
   * 这段时间内的 leave 先记账，等窗口稳定后复查。
   */
  const leaveGuardUntil = useRef(0);
  /**
   * 拖动结束的时刻（`Date.now()`）：这段时间内把悬停当成「松手残留」而不是意图，
   * 否则刚吸上的胶囊会被立刻打回圆球。详见 `POST_DRAG_HOVER_GUARD_MS`。
   */
  const dragEndedAt = useRef(0);
  /** 鼠标是否还在窗口里：展开是异步的，回来时得再确认一次才点亮 */
  const hovered = useRef(false);
  const [state, setState] = useState<BallState>("orb");
  /**
   * 展开时窗口被钳进屏幕的位移（逻辑像素，已取反向）。
   * 轮盘贴边撑开会把整扇窗推回屏幕内，球在窗口中心就会跟着跳 ——
   * 这里换成 CSS 变量喂给 `.ball-core`，让球钉在原来的屏幕位置。
   */
  const [wheelShift, setWheelShift] = useState<[number, number]>([0, 0]);
  /** 收起态的形态（tab / orb）：收回轮盘时照它还原，免得先闪一下常规球 */
  const formRef = useRef<"tab" | "orb">("orb");
  /** 逻辑像素换算用（窗口事件给的是物理像素） */
  const scaleRef = useRef(1);

  /**
   * 按窗口客户区高度认形态：展开态 200、贴边胶囊 64+32=96、常规球 52+32=84。
   *
   * 后两个数字 = 形状高度 + 上下各 16px 的透明余量（`ballhub::SHAPE_PAD_LOGICAL`，
   * 留给投影和右上角标，别删），所以阈值取两者中间的 90。
   * **改形状或余量要同步 `src-tauri/src/ballhub.rs`**（那边有单测卡这段关系）。
   */
  const applySize = (height: number) => {
    const logical = height / (scaleRef.current || 1);
    setState((current) => {
      const next: BallState = logical >= 150 ? "wheel" : logical >= 90 ? "tab" : "orb";
      // 收起态的形态记下来：收回轮盘时先用它切换 CSS（等窗口真缩回去再切就看不到淡出）
      if (next !== "wheel") formRef.current = next;
      return next === current ? current : next;
    });
  };

  /** 主动量一次当前尺寸（onMoved 兜底用） */
  const syncFormFromSize = async () => {
    try {
      applySize((await getCurrentWindow().innerSize()).height);
    } catch (err) {
      console.error("读取悬浮球尺寸失败", err);
    }
  };
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

    let timer: number | undefined;
    const unlistenMoved = win.onMoved(({ payload }) => {
      // 形态切换会伴一次移窗，顺手按最新尺寸核对一遍形态（兜底）
      void syncFormFromSize();
      window.clearTimeout(timer);
      timer = window.setTimeout(() => {
        void api.saveBallPosition(Math.round(payload.x), Math.round(payload.y));
      }, SAVE_DELAY_MS);
    });
    // **形态直接由窗口尺寸推**：Rust 是几何的唯一真相，尺寸变了就是形态变了。
    // 之前用自定义事件同步踩过坑：`listen()` 注册是异步的，启动时的吸附/摆正正好落在
    // 「问」与「注册完成」之间 → 事件漏掉、查询又拿到旧值，前端会一直以为自己是常规球
    // （悬停直接展开轮盘），而窗口其实已经是胶囊。尺寸推导不会有这种时序问题。
    const unlistenResized = win.onResized(({ payload }) => applySize(payload.height));
    void win
      .scaleFactor()
      .then((factor) => {
        scaleRef.current = factor;
        return win.innerSize();
      })
      .then((size) => applySize(size.height))
      .catch((err) => console.error("读取悬浮球尺寸失败", err));

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
      void unlistenResized.then((fn) => fn());
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
      .then(([dx, dy]) => {
        // 贴边时轮盘会被钳进屏幕，窗口整体位移，球在窗口中心就跟着跳。
        // 用命令带回来的补偿量把球反向偏移：球留在原地，轮盘从旁边滑出来
        const scale = scaleRef.current || 1;
        setWheelShift([-dx / scale, -dy / scale]);
        if (hovered.current) setState("wheel");
        // 窗口刚被重定位：接下来这一小段里的 pointerleave 不可信（见 leaveGuardUntil）
        leaveGuardUntil.current = Date.now() + 400;
      })
      .catch((err) => console.error("展开悬浮球失败", err));
  };

  /** 开始「停在常规态上」的计时：到点才展开轮盘 */
  const armExpand = () => {
    if (open || openTimer.current !== undefined) return;
    hovered.current = true;
    window.clearTimeout(collapseTimer.current);
    openTimer.current = window.setTimeout(() => {
      openTimer.current = undefined;
      if (hovered.current) expandHub();
    }, HOVER_OPEN_DELAY_MS);
  };

  /** 悬停作废（移出球面 / 开始拖动 / 离开窗口） */
  const disarmExpand = () => {
    window.clearTimeout(openTimer.current);
    openTimer.current = undefined;
  };

  /**
   * 鼠标在窗口里：按**当前位置压在哪个形状上**决定做什么。
   *
   * 用 `event.target` 判断，而不是「进了窗口就算」—— 窗口比形状宽得多
   * （无边框窗口被 SM_CXMINTRACK 撑到 136，形状只有 16 / 52 宽，见已知问题 12），
   * 光标从旁边那块透明区滑过时不该展开轮盘、也不该把那块桌面点击吃掉。
   *
   * 三种形态的语义（照稿子）：
   * - 贴边胶囊上：**先变常规态**（不进展开计时）—— 是不是要继续展开，交给下面的常规态决定；
   * - 常规态上：停满 `HOVER_OPEN_DELAY_MS` 才展开轮盘，中途移开就作废；
   * - 展开态：什么都不做（卫星按钮与中心盘自己管自己）。
   */
  const evaluateHover = (e: React.PointerEvent) => {
    const target = e.target as Element | null;
    if (open) {
      // 轮盘已展开：指针还在轮盘范围里就算「没离开」。
      // 展开瞬间窗口会重定位，WebView2 可能误派 pointerleave，这里用真实位置覆盖掉
      hovered.current = Boolean(target?.closest(".ball-wheel"));
      return;
    }
    if (state === "tab") {
      if (!target?.closest(".ball-tab") || undocking.current) return;
      // 刚松手：这一下悬停是拖动的残留，不是「想唤出常规态」
      if (Date.now() - dragEndedAt.current < POST_DRAG_HOVER_GUARD_MS) return;
      // 折叠态鼠标一放上去就展开成常规态（稿子 State 01 的 hover 语义）
      undocking.current = true;
      void api
        .setBallDocked(false)
        .catch((err) => console.error("唤出常规态失败", err))
        .then(() => {
          undocking.current = false;
        });
      return;
    }
    if (state !== "orb" || open) return;
    if (target?.closest(".ball-orb")) armExpand();
    else disarmExpand();
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
      .then(() => getCurrentWindow().startDragging())
      // startDragging 的 Promise 在松手之后才 resolve：这时才让 Rust 按落点定形态
      // （靠边就吸成胶囊）。挂在位置上报上会把刚唤出来的常规态吸回去
      .then(() => api.finishBallDrag())
      .then(() => {
        // 记下松手时刻：接下来这段时间的悬停不算意图（详见 POST_DRAG_HOVER_GUARD_MS）
        dragEndedAt.current = Date.now();
      })
      .catch((err) => console.error("结束拖动失败", err));
  };

  /** 松手后 Rust 会按落点决定形态（贴边就吸成胶囊），前端只等 `ball:state` */
  const onPointerUp = () => {
    pointer.current = null;
  };

  /** 常规球 / 贴边胶囊：单击直接开合主面板（顺手收回轮盘，别让它从面板边缘探出来） */
  const onShapeClick = () => {
    runSatellite(api.togglePanel);
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
      style={
        {
          opacity: opacity / 100,
          "--hub-shift-x": `${wheelShift[0]}px`,
          "--hub-shift-y": `${wheelShift[1]}px`,
        } as CSSProperties
      }
      onPointerEnter={evaluateHover}
      onPointerOver={evaluateHover}
      onPointerMove={evaluateHover}
      onPointerLeave={() => {
        const guard = leaveGuardUntil.current - Date.now();
        if (guard > 0) {
          // 窗口刚被重定位，这一下 leave 不可信：等它稳定后复查一次，
          // 期间指针若又落回轮盘（evaluateHover 会把 hovered 设回 true）就不收回
          window.clearTimeout(collapseTimer.current);
          collapseTimer.current = window.setTimeout(() => {
            if (!hovered.current) collapseHub(true);
          }, guard + 60) as unknown as number;
          return;
        }
        collapseHub();
      }}
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

        {/* 常规态：52×52 悬浮球（无发光晕，只有 2px 渐变环） */}
        <button
          type="button"
          className="ball-orb"
          title="PPClip：单击开合面板，右键菜单，按住可拖动；停住不动展开快捷按钮"
          aria-label="PPClip 悬浮球"
          onClick={onShapeClick}
          {...pointerHandlers}
        >
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
          title="PPClip：鼠标放上来变常规态，单击开合面板，按住可拖离边缘"
          aria-label="PPClip 悬浮入口（贴边）"
          onClick={onShapeClick}
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
