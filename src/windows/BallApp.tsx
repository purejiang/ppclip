import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { UnlistenFn } from "@tauri-apps/api/event";

import { api } from "../api";
import type { BallForm, BallSnapshot } from "../types";
import {
  IconClipboardArrow,
  IconHide,
  IconList,
  IconMonitor,
  IconScan,
} from "../components/icons";

/** 超过这个位移（CSS 像素）就判定为拖动而不是点击 */
const DRAG_THRESHOLD = 4;
/**
 * 鼠标停在常规球上满这么久才展开轮盘。
 *
 * 为什么要有延迟：展开不是纯视觉 —— 窗口会从约 96×96 撑到 232×232 并置顶，把桌面
 * 点击吃掉，所以必须区分「鼠标路过」和「真的想用」；停住不动本身就是最强的意图信号。
 *
 * 为什么是 2 秒：按产品要求定的，宁可慢也不误触（窗口一撑开就吃掉桌面点击）。
 * 前 0.3 秒的短延迟实测太容易在「鼠标顺手划过球」时误展开。
 */
const HOVER_OPEN_DELAY_MS = 2000;
/** 轮盘退场动画时长（CSS 里 `.is-wheel-closing` 的过渡要和它一致） */
const EXIT_MS = 140;
/** 鼠标移开后延迟多久去复核光标（太短会误判真离开，太长会觉得卡） */
const LEAVE_CHECK_MS = 120;
/**
 * 拖动结束后这段时间里不算「悬停意图」。
 *
 * 松手那一瞬鼠标必然还停在球上，立刻当成悬停就会把刚吸上的胶囊打回圆球 ——
 * 贴边留不住，形态来回切还会让窗口位置跳一下（实测日志：同一秒 tab 紧跟 orb）。
 * 600ms 够看清「已经贴边」，又不至于让想立刻唤出的人等太久。
 */
const POST_DRAG_HOVER_GUARD_MS = 600;

interface PointerState {
  x: number;
  y: number;
  dragging: boolean;
}

/** 轮盘的显示阶段：几何变化期间内容一律藏起来（见 `switching`） */
type Stage = "hidden" | "ready" | "open" | "closing";

/**
 * 等这一帧真的画出去（两个 rAF：React 提交 → 浏览器绘制）。
 *
 * 换窗口几何前**必须**先让「把内容藏掉」这一帧画出去，否则会看到一次错位闪现：
 * - `set_size` 只改右下边，左上角不动 → 窗口里的居中内容跟着跑到右下方；
 * - 紧接的 `set_position` 才把窗口挪到盘应该在的位置（贴边时是往左上挪）。
 * 两次调用之间只要漏出一帧，观感就是「悬停时往左上角闪一下」（实测反馈）。
 *
 * 关键点：`setSwitching(true)` 只是排了一次 React 渲染，后面那行 IPC 不会等它 ——
 * 所以必须显式等一帧，光靠「调用顺序」盖不住这个闪现。
 */
const nextPaint = () =>
  new Promise<void>((resolve) => {
    requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
  });

/**
 * 悬浮入口：**一扇窗、三个形状**（稿子「桌面悬浮球三态规范」）。
 *
 * - **贴边胶囊** 16×64，拖到屏幕左 / 右边缘自动吸附；
 * - **常规球** 52×52，桌面任意位置；
 * - **展开态** 200×200 HUD 轮盘（窗口此时是 232×232，四周各 16 的透明余量给投影）。
 *
 * ## 为什么现在不跳了
 *
 * 旧实现三个形态靠窗口 resize 切换，而 Windows 上「改尺寸」和「改位置」是两次 OS 调用，
 * 中间那一帧按旧窗口中心绘制 → 每次开合都跳；再加上贴边时轮盘被钳进屏幕、球在窗口中心
 * 跟着跳，前端补偿又是异步补的，观感就是「跳两下」。
 *
 * 现在三件事一起解决：
 * 1. **形态由 Rust 单向决定**（`ball:form` 快照 + 事件，带单调递增的 `rev`），
 *    前端不再按窗口尺寸猜；
 * 2. **窗口位置只由 anchor（形状中心）推导**，不读当前窗口几何 → 不累积漂移；
 * 3. **几何变化那一帧把内容藏起来**（`switching`）：换尺寸 / 换位置之间必然有一帧
 *    错位，但那一帧画的是空白，用户看到的是「球消失 → 轮盘淡入」。
 *
 * 贴边时轮盘为了整块进屏幕会被钳向内（最多 74px），盘心因此离开球的位置 —— 中心盘
 * 就是盘心（盘里居中），**不做反向补偿**：补偿会让中心球贴到盘边上，看着像球飞了。
 * 轮盘展开时球已经藏起来了，盘心这点位移看出来的是「轮盘从球那儿往内展开」。
 */
export default function BallApp() {
  const pointer = useRef<PointerState | null>(null);
  /** 悬停展开的倒计时（中途移开 / 开始拖动就清掉） */
  const openTimer = useRef<number | undefined>(undefined);
  /** 「胶囊 → 常规态」这次切换是否还在飞（防止 pointermove 连发重复调用命令） */
  const undocking = useRef(false);
  /**
   * 拖动结束的时刻（`Date.now()`）：这段时间内把悬停当成「松手残留」而不是意图，
   * 否则刚吸上的胶囊会被立刻打回圆球。详见 `POST_DRAG_HOVER_GUARD_MS`。
   */
  const dragEndedAt = useRef(0);
  const hovered = useRef(false);
  /**
   * 左键按住着（含正在拖动）。
   *
   * 按住期间**不接受任何悬停驱动的状态变化** —— 展开轮盘、胶囊唤出常规态都算。
   * 不加这个守卫会出两个错：
   * 1. 拖动开始那一帧的 pointermove 会冒泡到根节点重新 `armExpand()`（形状上的处理器
   *    刚 `disarmExpand()` 完，根节点的 `evaluateHover` 又给装回去了），于是拖着球不动
   *    两秒 → **拖动中途**轮盘直接展开、窗口跳成 232×232；
   * 2. 松手后那个计时器还活着 → 一到点就展开，刚吸上的胶囊立刻被打回圆球，
   *    「拖到边上贴不住」。
   */
  const holding = useRef(false);
  /** 轮盘开着时球这边一律不响应悬停（那时窗口就是轮盘） */
  const wheelOpen = useRef(false);
  /** 正在改窗口几何：这期间窗口会自己动一下，别把它的 pointerleave 当真 */
  const switching = useRef(false);
  const stageRef = useRef<Stage>("hidden");
  /** 关轮盘之后要执行的动作（卫星按钮：截图不能把自己拍进去） */
  const pendingAction = useRef<(() => Promise<void>) | null>(null);
  const lastRev = useRef(-1);

  const [form, setForm] = useState<BallForm>("orb");
  /** 渲染用的展开标志（`wheelOpen` 那个 ref 只给事件回调判断，不参与渲染） */
  const [wheelShown, setWheelShown] = useState(false);
  const [stage, setStageState] = useState<Stage>("hidden");
  const [hiddenContent, setHiddenContent] = useState(false);
  /** 悬浮球不透明度（%，来自设置） */
  const [opacity, setOpacity] = useState(100);
  /** 面板关着的时候新收进来的剪贴板条数（角标） */
  const [fresh, setFresh] = useState(0);
  const panelVisible = useRef(false);
  const collapseTimer = useRef<number | undefined>(undefined);
  const leaveTimer = useRef<number | undefined>(undefined);

  const setStage = (next: Stage) => {
    stageRef.current = next;
    setStageState(next);
  };

  const setSwitching = (on: boolean) => {
    switching.current = on;
    setHiddenContent(on);
  };

  /** 应用一份快照（事件与命令返回值共用同一条路径） */
  const applySnap = (snap: BallSnapshot, nextStage?: Stage) => {
    if (snap.rev < lastRev.current) return;
    lastRev.current = snap.rev;
    wheelOpen.current = snap.wheelOpen;
    setWheelShown(snap.wheelOpen);
    setForm(snap.form);
    if (!snap.wheelOpen) setStage("hidden");
    else if (nextStage) setStage(nextStage);
  };

  useEffect(() => {
    let disposed = false;
    const pending: Promise<UnlistenFn>[] = [];

    const boot = async () => {
      // **先订阅、再拉快照**：中间到的事件靠 `rev` 挡住旧快照
      pending.push(api.onBallForm((snap) => applySnap(snap)));
      pending.push(
        api.onSettingsChanged(() => {
          void api
            .getSettings()
            .then((s) => setOpacity(s.ballOpacity))
            .catch(() => undefined);
        }),
      );
      pending.push(
        api.onPanelVisible((visible) => {
          panelVisible.current = visible;
          if (visible) setFresh(0);
        }),
      );
      pending.push(
        api.onClipsChanged(() => {
          if (!panelVisible.current) setFresh((count) => count + 1);
        }),
      );

      const snap = await api.ballSnapshot();
      if (disposed) return;
      applySnap(snap);
      try {
        await api.showBall();
      } catch (err) {
        console.error("显示悬浮球失败", err);
      }
      void api
        .getSettings()
        .then((s) => setOpacity(s.ballOpacity))
        .catch(() => undefined);
    };
    void boot().catch((err) => console.error("悬浮球初始化失败", err));

    return () => {
      disposed = true;
      window.clearTimeout(openTimer.current);
      window.clearTimeout(collapseTimer.current);
      window.clearTimeout(leaveTimer.current);
      for (const p of pending) void p.then((fn) => fn());
    };
  }, []);

  // ------------------------------------------------------------------ 轮盘

  /** 展开轮盘：先藏内容（等这一帧画出去）→ 改窗口几何 → 再淡入 */
  const expandWheel = async () => {
    if (wheelOpen.current || switching.current) return;
    setSwitching(true);
    try {
      await nextPaint();
      // 这一帧的工夫里可能已经按下去 / 拖走了 —— 按住与拖动中一律不展开
      if (holding.current || !hovered.current) {
        setSwitching(false);
        return;
      }
      const snap = await api.setBallWheel(true);
      applySnap(snap, "ready");
      setSwitching(false);
      // 两帧 rAF：让浏览器先把「盘面 opacity 0」这一帧画出来，过渡才有起点
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          if (stageRef.current === "ready") setStage("open");
        });
      });
    } catch (err) {
      console.error("展开轮盘失败", err);
      setSwitching(false);
    }
  };

  /** 收起轮盘：淡出 → 藏内容（等这一帧画出去）→ 还原窗口几何 → 回来时的动作 */
  const collapseWheel = (immediate = false) => {
    if (!wheelOpen.current) return;
    if (stageRef.current === "closing") return;
    hovered.current = false;
    window.clearTimeout(openTimer.current);
    openTimer.current = undefined;

    const restore = async () => {
      setSwitching(true);
      try {
        // immediate（卫星按钮 / 右键）没有淡出垫场，窗口还在画着盘面就被 resize，
        // 所以这里同样要等隐藏的那一帧画出去
        await nextPaint();
        const snap = await api.setBallWheel(false);
        applySnap(snap);
      } catch (err) {
        console.error("收起轮盘失败", err);
      } finally {
        setStage("hidden");
        setSwitching(false);
        const action = pendingAction.current;
        pendingAction.current = null;
        if (action) {
          try {
            await action();
          } catch (err) {
            console.error("轮盘按钮执行失败", err);
          }
        }
      }
    };

    if (immediate) {
      void restore();
      return;
    }
    setStage("closing");
    collapseTimer.current = window.setTimeout(() => void restore(), EXIT_MS);
  };

  /** 卫星按钮：先收轮盘再执行（截图 / 面板都不能把轮盘拍进去） */
  const runAfterClose = (action: () => Promise<void>) => {
    pendingAction.current = action;
    collapseWheel(true);
  };

  // ------------------------------------------------------------------ 悬停

  /** 开始「停在常规态上」的计时：到点才开轮盘 */
  const armExpand = () => {
    if (wheelOpen.current || openTimer.current !== undefined) return;
    hovered.current = true;
    openTimer.current = window.setTimeout(() => {
      openTimer.current = undefined;
      if (!hovered.current || wheelOpen.current) return;
      void expandWheel();
    }, HOVER_OPEN_DELAY_MS);
  };

  /** 悬停作废（移出球面 / 开始拖动） */
  const disarmExpand = () => {
    window.clearTimeout(openTimer.current);
    openTimer.current = undefined;
  };

  /**
   * 鼠标在窗口里：按**当前位置压在哪个形状上**决定做什么。
   *
   * 用 `event.target` 判断，而不是「进了窗口就算」—— 窗口比形状宽得多
   * （无边框窗口被 SM_CXMINTRACK 撑宽，形状只有 16 / 52 宽），
   * 光标从旁边那块透明区滑过时不该展开轮盘、也不该把那块桌面点击吃掉。
   */
  const evaluateHover = (e: React.PointerEvent) => {
    // 事件自带的按键状态最可靠：`startDragging()` 会吞掉 pointerup，光靠 onPointerUp
    // 解除按住态，遇到「在形状外松手」就会卡在按住上（悬停彻底失灵）。以 buttons 为准兜底。
    if (e.buttons === 0) {
      holding.current = false;
      pointer.current = null;
    }
    // 按住（拖动）/ 换几何 / 轮盘开着：一律不响应悬停
    if (holding.current || switching.current || wheelOpen.current) return;
    // 刚松手：这一下悬停是拖动的残留，不是「想唤出常规态」或者「想展开轮盘」
    if (Date.now() - dragEndedAt.current < POST_DRAG_HOVER_GUARD_MS) return;
    const target = e.target as Element | null;
    if (form === "tab") {
      if (!target?.closest(".ball-tab") || undocking.current) return;
      undocking.current = true;
      void api
        .setBallForm(false)
        .then((snap) => applySnap(snap))
        .catch((err) => console.error("唤出常规态失败", err))
        .finally(() => {
          undocking.current = false;
        });
      return;
    }
    if (target?.closest(".ball-orb")) armExpand();
    else disarmExpand();
  };

  // ------------------------------------------------------------------ 拖动 / 点击

  const onPointerDown = (e: React.PointerEvent) => {
    if (e.button !== 0) return;
    holding.current = true;
    // 按下去就说明「要拖 / 要按」，之前攒的悬停意图作废
    hovered.current = false;
    disarmExpand();
    pointer.current = { x: e.screenX, y: e.screenY, dragging: false };
  };

  const onPointerMove = (e: React.PointerEvent) => {
    const state = pointer.current;
    if (!state) return;
    // 左键已经不在了：`startDragging()` 会把 pointerup 吞掉，`pointer.current` 会一直留着
    if (e.buttons === 0) {
      pointer.current = null;
      holding.current = false;
      return;
    }
    if (state.dragging) return;
    if (Math.hypot(e.screenX - state.x, e.screenY - state.y) <= DRAG_THRESHOLD) return;

    state.dragging = true;
    hovered.current = false;
    disarmExpand();
    // 先给 Rust 侧开看门狗（吸附判定由它按真实左键状态收尾），**不等它返回**再进原生拖动：
    // 移动循环一起来主线程就被占住，await 只会给拖动加延迟。
    // 为什么不只靠下面那条 Promise：模态移动循环里它的落地时机不可靠，实测会漏掉
    // 「松手 → 吸附」这一步（拖到边上不贴边、日志里没有「拖动结束」）。
    void api.beginBallDrag().catch(() => undefined);
    // `startDragging()` 的 Promise 在松手之后才 resolve：再补一次判定（幂等，看门狗已判过就没事）
    void getCurrentWindow()
      .startDragging()
      .then(() => api.finishBallDrag())
      .then((snap) => {
        // 松手了：解除按住态，并把拖动期间可能残留的悬停计时彻底清掉 ——
        // 不清的话它一到点就把刚贴上的胶囊展开成轮盘（「拖到边上贴不住」）
        holding.current = false;
        hovered.current = false;
        disarmExpand();
        // 记下松手时刻：接下来这段时间的悬停不算意图
        dragEndedAt.current = Date.now();
        applySnap(snap);
      })
      .catch((err) => {
        holding.current = false;
        console.error("结束拖动失败", err);
      });
  };

  const onPointerUp = () => {
    holding.current = false;
    pointer.current = null;
  };

  const onContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    // 轮盘里右键固定是「收起」（窗口是 focusable 的，但收起更符合直觉）
    if (wheelOpen.current) {
      collapseWheel();
      return;
    }
    void api.showBallMenu();
  };

  const shapeHandlers = {
    onPointerDown,
    onPointerMove,
    onPointerUp,
    onContextMenu,
  };

  /** 常规球 / 贴边胶囊：单击直接开合主面板 */
  const onShapeClick = () => {
    void api.togglePanel().catch((err) => console.error("开合面板失败", err));
  };

  // ------------------------------------------------------------------ 渲染

  const formClass = wheelShown ? "is-wheel" : `is-${form}`;
  const stageClass =
    stage === "open" ? " is-wheel-open"
    : stage === "closing" ? " is-wheel-closing"
    : "";

  return (
    <div
      className={`ball-root ${formClass}${stageClass}${hiddenContent ? " is-switching" : ""}`}
      style={{ opacity: opacity / 100 }}
      onPointerEnter={(e) => {
        // 展开态：指针还在轮盘范围里就算「没离开」，把待收起清掉
        if (wheelOpen.current) {
          window.clearTimeout(leaveTimer.current);
          leaveTimer.current = undefined;
          return;
        }
        evaluateHover(e);
      }}
      onPointerOver={evaluateHover}
      onPointerMove={evaluateHover}
      onPointerLeave={() => {
        hovered.current = false;
        disarmExpand();
        if (!wheelOpen.current || switching.current) return;
        // 窗口改尺寸会让 Chromium 派一次假 leave，而且因为指针没动不会再补
        // pointerover —— 光看事件分不清，所以延迟一点去问真实光标
        window.clearTimeout(leaveTimer.current);
        leaveTimer.current = window.setTimeout(() => {
          void api
            .hubCursorInside()
            .then((inside) => {
              if (!inside) collapseWheel();
            })
            .catch(() => collapseWheel());
        }, LEAVE_CHECK_MS);
      }}
    >
      <div className="ball-hub">
        {/* 展开态：200×200 HUD 轮盘 */}
        <div className="ball-wheel" aria-hidden={stage !== "open"}>
          <span className="ball-orbit" aria-hidden="true" />

          <button
            type="button"
            className="ball-sat ball-sat-up"
            title="区域截图"
            aria-label="区域截图"
            onClick={() => runAfterClose(api.startRegionCapture)}
          >
            <IconScan size={16} />
          </button>
          <button
            type="button"
            className="ball-sat ball-sat-right"
            title="全屏截图"
            aria-label="全屏截图"
            onClick={() => runAfterClose(api.captureFullscreen)}
          >
            <IconMonitor size={16} />
          </button>
          <button
            type="button"
            className="ball-sat ball-sat-down"
            title="隐藏悬浮球（重新启动 PPClip 可找回）"
            aria-label="隐藏悬浮球"
            onClick={() => runAfterClose(api.hideBall)}
          >
            <IconHide size={16} />
          </button>
          <button
            type="button"
            className="ball-sat ball-sat-left"
            title="剪贴板历史"
            aria-label="打开剪贴板历史面板"
            onClick={() => runAfterClose(api.togglePanel)}
          >
            <IconList size={16} />
          </button>

          <button
            type="button"
            className="ball-core"
            title="PPClip：单击开合面板，右键收起"
            aria-label="PPClip 中心圆盘"
            onClick={() => runAfterClose(api.togglePanel)}
            onContextMenu={onContextMenu}
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
          {...shapeHandlers}
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
          {...shapeHandlers}
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
