//! 悬浮球的三种形态（照 stitch 稿「桌面悬浮球三态规范」）：
//!
//! | 形态 | 形状尺寸 | 什么时候 |
//! |---|---|---|
//! | 贴边胶囊 Collapsed | 16×64 | 拖到屏幕左 / 右边缘自动吸附（对称胶囊，左右通用） |
//! | 常规悬浮球 Floating Orb | 52×52 | 常态：桌面上任意位置 |
//! | 展开态 HUD Wheel | 200×200 | 鼠标在胶囊 / 球上停 0.3 秒；离开收回 |
//!
//! 收起态的**窗口客户区**比形状大一圈：上下各留 `SHAPE_PAD_LOGICAL`（16 逻辑像素），
//! 让投影与角标画得下 —— 见那个常量的注释，别把它当余量删掉。
//! 稿子里展开态写的是 240×240，实际用下来偏大，这里收到 200（只改 `WHEEL_LOGICAL` 即可再调）。
//!
//! 三条硬约束（撑窗与吸附都绕不开）：
//!
//! 1. **形状中心不动**。三种形态在窗口里都是居中的，所以撑开 / 收起 / 胶囊↔球切换 / 吸附
//!    一律按「窗口中心 = 形状中心」重算左上角（`top_left_for_center` 有单测）；
//!    只有收起例外 —— 收起照撑开那一刻记下的**实测矩形**一比一还原，否则边框与取整的误差
//!    会每次悬停累积一次，几十次之后球就漂走了。
//! 2. **收起尺寸不能按配置算**。Windows 会把无边框窗口按 `SM_CXMINTRACK` 撑到 136 宽
//!    （见 AGENTS 已知问题 12），所以宽度从来不是我们说了算，只能记实测值。
//! 3. **轮盘要钳进显示器**。贴边时按中心撑出来的 200×200 会有一半在屏幕外，
//!    那一侧的卫星按钮就点不到了（`clamp_into` 有单测）。
//!
//! 窗口尺寸的唯一真相在这里：前端只按窗口客户区高度推当前形态（见设计决策 13），
//! 所以改形状 / 余量必须同步 `src/windows/BallApp.tsx` 的阈值。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize};

/// 贴边胶囊（稿子 State 01）的逻辑尺寸
pub const TAB_W_LOGICAL: f64 = 16.0;
pub const TAB_H_LOGICAL: f64 = 64.0;
/// 常规悬浮球（稿子 State 02）的逻辑尺寸
pub const ORB_LOGICAL: f64 = 52.0;
/// 展开态轮盘（稿子 State 03 是 240×240，这里按「太大了」的反馈收到 200）
pub const WHEEL_LOGICAL: f64 = 200.0;
/// 形状四周留出的透明余量（逻辑像素）：**投影与角标要画得下**。
///
/// 竖向必须留：窗口宽度我们说了不算（`SM_CXMINTRACK` 会把无边框窗口撑到 136，
/// 形状两侧天然各有 40+px 富余），但**高度是我们定的** —— 只要客户区高度 = 形状高度，
/// 形状外的每一像素都在窗口外，`.ball-orb-body` 的 `0 4px 12px` 投影（往上 8、往下 16）
/// 会被切得干干净净，实测剖面「形状上下边缘之外 = 0」，横向上却是完整的渐隐；
/// 右上角计数角标（`top:-5px`）也被削掉上半截，贴边胶囊的顶部角标（`top:-9px`）
/// 更是整颗都在窗口外、从来没显示过。留 16px 之后两者都画得下（含 hover 的 scale 1.05 / 1.1）。
///
/// ⚠️ 改这里要同步 `src/windows/BallApp.tsx` 的形态阈值（150 / 90）
/// 与 `ballhub::tests::collapsed_windows_leave_room_for_the_shadow_and_the_badge`。
pub const SHAPE_PAD_LOGICAL: f64 = 16.0;
/// 形状中心离屏幕左右边缘多近（逻辑像素）就吸附成胶囊。
///
/// 判定用的是**形状中心**，所以球半径（26）会吃掉一部分：40 实际只相当于
/// 「球边距屏幕边 14px 内」。实测拖到边缘经常差几像素就判不中，放宽到 60
/// （球边距边 ≈ 34px）更符合「拖到边就该吸住」的直觉。
const DOCK_MARGIN: f64 = 60.0;

/// 展开前那一刻的收起态窗口几何（**实测**值）
#[derive(Clone, Copy)]
struct Collapsed {
    /// 窗口矩形（outer）左上角
    pos: PhysicalPosition<i32>,
    /// 窗口矩形（outer）尺寸
    size: PhysicalSize<u32>,
    /// 边框总厚度 = outer − inner（无边框时为 0；本机实测就是 0）
    frame: u32,
}

#[derive(Default)]
pub struct BallHubState {
    /// 收起时照它原样还原
    collapsed: Mutex<Option<Collapsed>>,
    expanded: AtomicBool,
    /// 收起态是「贴边胶囊」(true) 还是「常规球」(false)
    docked: AtomicBool,
    /// 展开时轮盘被钳进显示器造成的窗口位移（物理像素）。
    /// 前端拿它的反向值把中心球画回原来的屏幕位置 —— 否则观感是「球先跳一下再展开」。
    wheel_shift: Mutex<(i32, i32)>,
}

/// 当前是不是展开态
pub fn is_expanded(app: &AppHandle) -> bool {
    app.try_state::<BallHubState>()
        .is_some_and(|state| state.expanded.load(Ordering::Relaxed))
}

/// 收起态是不是贴边胶囊
pub fn is_docked(app: &AppHandle) -> bool {
    app.try_state::<BallHubState>()
        .is_some_and(|state| state.docked.load(Ordering::Relaxed))
}

/// 展开时窗口被钳进显示器的位移量（物理像素，x/y）。
///
/// 球贴着屏幕边时，以它为中心的 200×200 轮盘必定有一半在屏幕外，
/// `expand_target` 会把整扇窗钳回来 —— 球在窗口中心，于是跟着跳。
/// 前端按这个偏移的反向值把中心球画回原屏幕位置，观感变成「球钉在原地、
/// 轮盘从旁边滑出来」。收起态恒为 (0, 0)。
pub fn wheel_shift(app: &AppHandle) -> (i32, i32) {
    app.try_state::<BallHubState>()
        .map(|state| *state.wheel_shift.lock().unwrap_or_else(|e| e.into_inner()))
        .unwrap_or((0, 0))
}

/// 当前形态名（前端拿它切 CSS）：`tab` / `orb` / `wheel`
pub fn state_name(app: &AppHandle) -> &'static str {
    if is_expanded(app) {
        "wheel"
    } else if is_docked(app) {
        "tab"
    } else {
        "orb"
    }
}

/// 收起态当前形态的**形状**高度（逻辑像素）—— 胶囊 64 / 球 52。
/// 竖向钳制用它，不用窗口高度：形状四周那圈 16px 是留给投影的透明余量，
/// 允许它探出屏幕外（贴边时形状应该能贴到屏幕上下边缘，跟横向一个道理）。
fn collapsed_shape_h_logical(docked: bool) -> f64 {
    if docked {
        TAB_H_LOGICAL
    } else {
        ORB_LOGICAL
    }
}

/// 收起态当前形态的**窗口客户区**高度（逻辑像素）= 形状高度 + 上下各一圈余量。
fn collapsed_h_logical(docked: bool) -> f64 {
    collapsed_shape_h_logical(docked) + SHAPE_PAD_LOGICAL * 2.0
}

/// 收起态当前形态应有的**逻辑**宽度（胶囊细、球方）
fn collapsed_w_logical(docked: bool) -> f64 {
    if docked {
        TAB_W_LOGICAL
    } else {
        ORB_LOGICAL
    }
}

/// 撑开 / 收回。
///
/// 同方向重复调用是空操作 —— 前端的 pointerenter 会重复触发。
pub fn set_expanded(app: &AppHandle, expanded: bool) -> Result<(), String> {
    let (Some(state), Some(win)) = (
        app.try_state::<BallHubState>(),
        app.get_webview_window("ball"),
    ) else {
        return Ok(());
    };
    if state.expanded.load(Ordering::Relaxed) == expanded {
        return Ok(());
    }
    let docked = state.docked.load(Ordering::Relaxed);

    let scale = win
        .scale_factor()
        .map_err(|e| format!("读取悬浮球缩放失败: {e}"))?;
    let size = win
        .outer_size()
        .map_err(|e| format!("读取悬浮球尺寸失败: {e}"))?;
    let pos = win
        .outer_position()
        .map_err(|e| format!("读取悬浮球位置失败: {e}"))?;

    // 收起态客户区高度（记录几何与兜底分支都要用）
    let collapsed_inner_h = (collapsed_h_logical(docked) * scale).round().max(1.0) as u32;

    if expanded {
        // 记录当前（收起）几何。**frame 一律记 0**：本机无边框窗口实测 outer == inner，
        // 而窗口刚创建时 inner_size 会读到脏值（实测算出 56），照它定位会整体偏上 28px
        let geom = Collapsed {
            pos,
            size,
            frame: 0,
        };
        *state.collapsed.lock().unwrap_or_else(|e| e.into_inner()) = Some(geom);

        let edge = (WHEEL_LOGICAL * scale).round().max(1.0) as u32;
        state.expanded.store(true, Ordering::Relaxed);

        // 先撑尺寸，再按**实测** outer 定位 —— 与 set_docked 同一套做法，不推算 frame
        win.set_size(PhysicalSize::new(edge, edge))
            .map_err(|e| format!("调整悬浮球尺寸失败: {e}"))?;
        let actual = win
            .outer_size()
            .map_err(|e| format!("读取悬浮球尺寸失败: {e}"))?;

        let cx = pos.x as f64 + size.width as f64 / 2.0;
        let cy = pos.y as f64 + size.height as f64 / 2.0;
        // 钳制范围按**形状中心**所在的那块屏算（见 monitor_bounds 的说明）
        let bounds = monitor_bounds(app, cx.round() as i32, cy.round() as i32);
        let (target, shift) = wheel_position(cx, cy, actual, bounds);
        *state.wheel_shift.lock().unwrap_or_else(|e| e.into_inner()) = shift;
        win.set_position(target)
            .map_err(|e| format!("调整悬浮球位置失败: {e}"))?;

        log::info!(
            "悬浮球 → wheel：{}×{} @({}, {}) → {}×{} @({}, {})，钳制位移 ({}, {})",
            size.width,
            size.height,
            pos.x,
            pos.y,
            actual.width,
            actual.height,
            target.x,
            target.y,
            shift.0,
            shift.1
        );
        return Ok(());
    }

    // 收回：照撑开那一刻记下的矩形一比一还原（尺寸去掉边框就是客户区）
    let recorded = *state.collapsed.lock().unwrap_or_else(|e| e.into_inner());
    let (target_size, target_pos) = match recorded {
        Some(geom) => collapse_target(geom),
        // 没记过（正常流程不会发生）：退回按中心反推，至少别把球留在窗口外
        None => {
            let cx = pos.x as f64 + size.width as f64 / 2.0;
            let cy = pos.y as f64 + size.height as f64 / 2.0;
            (
                PhysicalSize::new(size.width, collapsed_inner_h),
                top_left_for_center(cx, cy, size),
            )
        }
    };
    state.expanded.store(false, Ordering::Relaxed);
    *state.wheel_shift.lock().unwrap_or_else(|e| e.into_inner()) = (0, 0);
    win.set_size(target_size)
        .map_err(|e| format!("调整悬浮球尺寸失败: {e}"))?;
    win.set_position(target_pos)
        .map_err(|e| format!("调整悬浮球位置失败: {e}"))?;
    log::info!(
        "悬浮球 → {}：{}×{} @({}, {}) → {}×{} @({}, {})",
        state_name(app),
        size.width,
        size.height,
        pos.x,
        pos.y,
        target_size.width,
        target_size.height,
        target_pos.x,
        target_pos.y
    );
    Ok(())
}

/// 切换「贴边胶囊 ↔ 常规球」。
///
/// 形状在窗口里居中，所以切换时**保持形状中心**，只改窗口高度；
/// 吸附成胶囊时再把窗口横向推出去，让 16 宽的胶囊正好贴住屏幕边
/// （顺带把窗口两侧那块吃点击的透明区推到屏幕外）。
/// 关于横向钳制：一律按**形状**算（`clamp_center`），不按窗口 —— 窗口宽 136、形状才 52，
/// 按窗口钳会把球从鼠标底下推走 60px，鼠标一转眼就落在球外面（点不到也悬停不到）。
/// 返回值是**生效后**的形态：展开态里不切形态，这时返回原来的形态。
/// 幂等：目标状态已经满足时不动窗口。
pub fn set_docked(app: &AppHandle, docked: bool) -> Result<bool, String> {
    let (Some(state), Some(win)) = (
        app.try_state::<BallHubState>(),
        app.get_webview_window("ball"),
    ) else {
        return Ok(docked);
    };
    // 展开态下不切形态：先收回（前端负责顺序），免得把轮盘改成小球
    if state.expanded.load(Ordering::Relaxed) {
        return Ok(state.docked.load(Ordering::Relaxed));
    }

    let scale = win
        .scale_factor()
        .map_err(|e| format!("读取悬浮球缩放失败: {e}"))?;
    let outer = win
        .outer_size()
        .map_err(|e| format!("读取悬浮球尺寸失败: {e}"))?;
    let inner = win
        .inner_size()
        .map_err(|e| format!("读取悬浮球客户区尺寸失败: {e}"))?;
    let pos = win
        .outer_position()
        .map_err(|e| format!("读取悬浮球位置失败: {e}"))?;

    let inner_h = (collapsed_h_logical(docked) * scale).round().max(1.0) as u32;
    let shape_w = (collapsed_w_logical(docked) * scale).round().max(1.0) as i32;
    let shape_h = (collapsed_shape_h_logical(docked) * scale).round().max(1.0) as i32;

    // 形状中心（= 客户区中心），必须在改尺寸**之前**记下来
    let cx = pos.x as f64 + outer.width as f64 / 2.0;
    let cy = pos.y as f64 + outer.height as f64 / 2.0;

    state.docked.store(docked, Ordering::Relaxed);
    // 先改尺寸，再读**实测** outer 来定位。
    // 绝对不能用 `outer − inner` 推算 frame：窗口刚创建时 inner_size 还没稳定
    // （本机实测算出 56，真实是 0），多出来的 56 会让窗口整体偏上 28px ——
    // 球于是落到屏幕外甚至隔壁显示器，鼠标点不到，悬停自然也失效。
    if inner.height != inner_h {
        win.set_size(PhysicalSize::new(inner.width, inner_h))
            .map_err(|e| format!("调整悬浮球尺寸失败: {e}"))?;
    }
    let actual = win
        .outer_size()
        .map_err(|e| format!("读取悬浮球尺寸失败: {e}"))?;
    let (win_w, outer_h) = (actual.width as i32, actual.height as i32);

    let mut x = pos.x;
    let mut y = (cy - outer_h as f64 / 2.0).round() as i32;
    if let Some((mx, my, mw, mh)) = monitor_bounds(app, cx.round() as i32, cy.round() as i32) {
        // 钳制一律按**形状**算，不按窗口：窗口比形状宽得多（136 vs 16/52），
        // 按窗口钳会把球从鼠标底下推走 60px —— 贴边态转常规态时鼠标就停在形状上，
        // 那一下推走直接让鼠标落在球外面（点不到、也悬停不到）。
        // 纵向同理用 `shape_h`：那一圈余量是透明的，允许探出屏幕，
        // 否则胶囊会永远离屏幕上下边缘差 16px，贴不到边。
        let center_y = clamp_center(cy.round() as i32, my, mh, shape_h);
        if docked {
            x = docked_x(cx.round() as i32, win_w, shape_w, mx, mw);
        } else {
            let center_x = clamp_center(cx.round() as i32, mx, mw, shape_w);
            x = (center_x as f64 - win_w as f64 / 2.0).round() as i32;
        }
        y = (center_y as f64 - outer_h as f64 / 2.0).round() as i32;
    } else if docked {
        x = cx.round() as i32 - win_w / 2 - (win_w - shape_w) / 2;
    }
    if x != pos.x || y != pos.y {
        win.set_position(PhysicalPosition::new(x, y))
            .map_err(|e| format!("调整悬浮球位置失败: {e}"))?;
    }
    log::info!(
        "悬浮球形态 → {}：{}×{} @({}, {}) → {}×{} @({}, {})",
        state_name(app),
        outer.width,
        outer.height,
        pos.x,
        pos.y,
        actual.width,
        outer_h,
        x,
        y
    );
    // 实测值：`frame` 是猜的（outer−inner），一旦 Windows 给了非零边框，
    // 前端按窗口高度认形态就会认错（182/1.5=121 会被当成 tab 而不是 orb）
    if let (Ok(ao), Ok(ai)) = (win.outer_size(), win.inner_size()) {
        log::info!(
            "  实测 outer {}×{} inner {}×{}，前端应判 {}（logical h = {:.1}）",
            ao.width,
            ao.height,
            ai.width,
            ai.height,
            state_name(app),
            ai.height as f64 / scale
        );
    }
    Ok(state.docked.load(Ordering::Relaxed))
}

/// **拖动结束时**按落点决定形态（`finish_ball_drag`）：**幂等**。
///
/// 判定只看形状中心离屏幕左右边缘有多远：进了 40 逻辑像素以内就吸附成胶囊，
/// 否则回到常规球。因为吸附后窗口会故意探出屏幕 60px（中心离边 8px），
/// 第二次调用依然判定为「贴边」，不会来回抖。
///
/// ⚠️ 只在**拖动结束**时调用，别挂在每次位置上报上：常规球被钳进可视区之后
/// 左边缘同样贴着屏幕，位置上报一多就会被误判成「想贴边」，把刚唤出来的常规态吸回去。
/// 返回值是生效后的形态。
pub fn dock(app: &AppHandle) -> Result<bool, String> {
    let Some(win) = app.get_webview_window("ball") else {
        return Ok(is_docked(app));
    };
    // 展开态下窗口是按形状中心重定位过的大窗，此时吸附没有意义
    if is_expanded(app) {
        return Ok(is_docked(app));
    }
    let scale = win
        .scale_factor()
        .map_err(|e| format!("读取悬浮球缩放失败: {e}"))?;
    let size = win
        .outer_size()
        .map_err(|e| format!("读取悬浮球尺寸失败: {e}"))?;
    let pos = win
        .outer_position()
        .map_err(|e| format!("读取悬浮球位置失败: {e}"))?;
    // 判定用**形状**中心离边缘的距离：窗口中心会因为窗口被撑宽而偏（136 宽里形状只占 52）
    let shape_w = (collapsed_w_logical(is_docked(app)) * scale).round().max(1.0) as i32;
    let shape_left = pos.x + (size.width as i32 - shape_w) / 2;
    let shape_center = shape_left + shape_w / 2;
    let shape_center_y = pos.y + size.height as i32 / 2;

    // 按形状中心找显示器：此刻窗口可能正跨着屏幕边界，`current_monitor` 会判到隔壁屏
    let Some((mx, _my, mw, _mh)) = monitor_bounds(app, shape_center, shape_center_y) else {
        return Ok(is_docked(app));
    };

    let margin = (DOCK_MARGIN * scale).round() as i32;
    let near_left = shape_center - mx <= margin;
    let near_right = (mx + mw) - shape_center <= margin;
    let want_docked = near_left || near_right;

    // 即使形态没变也要走一遍 `set_docked`：它会把窗口尺寸与位置摆正
    // （窗口高度按形态是 64 / 52，位置按形状中心重新算）
    set_docked(app, want_docked)
}

/// 形状中心所在的显示器物理范围 `(x, y, w, h)`。
///
/// **不能直接用 `current_monitor()`**：贴边时窗口（180 宽）会跨屏幕边界 ——
/// 球停在右边缘、窗口右半边已经在隔壁屏上，`current_monitor()` 有时会判到隔壁那块，
/// 钳制范围就整个错位：轮盘被推到隔壁屏、鼠标留在原地，于是「一展开就收回」。
/// 按形状中心去问就不会有这种歧义（形状中心永远在球所在的那块屏上）。
fn monitor_bounds(app: &AppHandle, cx: i32, cy: i32) -> Option<(i32, i32, i32, i32)> {
    app.monitor_from_point(cx as f64, cy as f64)
        .ok()
        .flatten()
        .map(|monitor| {
            let origin = monitor.position();
            let area = monitor.size();
            (origin.x, origin.y, area.width as i32, area.height as i32)
        })
}

/// 轮盘定位：窗口矩形中心停在形状中心，再整体钳进显示器。
///
/// 钳制很关键：胶囊贴屏幕边缘停靠，200×200 的轮盘以它为中心会有一半在屏幕外 ——
/// 那一侧的卫星按钮就点不到了。钳进来之后轮盘会看着「从边缘滑进来」，收起重置时照旧
/// 一比一还原，不会漂。
///
/// 返回值第二项是**钳制造成的位移**（`target − 不钳制时的位置`）：球位于窗口中心，
/// 窗口一移球就跟着跳，前端要用它把球反向画回去。没有 `bounds` 时恒为 `(0, 0)`。
///
/// `outer` 必须传**实测**的窗口尺寸（`set_size` 之后再读），不要用推算值 ——
/// 窗口刚创建时 `inner_size` 会给出脏读数，推算出来的 frame 能让球整体偏 28px。
fn wheel_position(
    cx: f64,
    cy: f64,
    outer: PhysicalSize<u32>,
    bounds: Option<(i32, i32, i32, i32)>,
) -> (PhysicalPosition<i32>, (i32, i32)) {
    let centered = top_left_for_center(cx, cy, outer);
    let mut target = centered;

    if let Some((bx, by, bw, bh)) = bounds {
        target.x = clamp_into(target.x, bx, bw, outer.width as i32);
        target.y = clamp_into(target.y, by, bh, outer.height as i32);
    }
    (target, (target.x - centered.x, target.y - centered.y))
}

/// 收起：把展开前记下的窗口矩形一比一还原（尺寸去掉边框就是 inner）
fn collapse_target(geom: Collapsed) -> (PhysicalSize<u32>, PhysicalPosition<i32>) {
    (
        PhysicalSize::new(
            geom.size.width.saturating_sub(geom.frame),
            geom.size.height.saturating_sub(geom.frame),
        ),
        geom.pos,
    )
}

/// 把长度 `size` 的窗口钳进 [start, start+extent]；显示器比窗口还小时贴左上角
fn clamp_into(value: i32, start: i32, extent: i32, size: i32) -> i32 {
    value.clamp(start, (start + extent - size).max(start))
}

/// 把**形状中心**钳到「形状整块留在显示器内」的范围里（纯算术，单测）。
///
/// 与 `clamp_into` 的区别是这里管的是形状不是窗口：形状在窗口里居中，
/// 窗口两侧那圈透明区允许探出屏幕（贴边态正是靠这个把吃点击的透明区推出去）。
fn clamp_center(center: i32, start: i32, extent: i32, shape: i32) -> i32 {
    let half = (shape / 2).max(0);
    center.clamp(start + half, (start + extent - half).max(start + half))
}

/// 吸附成胶囊后的窗口左上角 x（纯算术，单测）。
///
/// 胶囊在窗口里居中，所以「胶囊贴左边」= 窗口左上角 = 屏幕左边界 − (窗口宽 − 胶囊宽)/2；
/// 「贴右边」= 屏幕右边界 − 窗口宽 + 同一个内缩量。形状中心偏屏幕左半边就贴左，否则贴右。
fn docked_x(center_x: i32, win_w: i32, tab_w: i32, mon_x: i32, mon_w: i32) -> i32 {
    let inset = ((win_w - tab_w) / 2).max(0);
    if (center_x - mon_x) * 2 < mon_w {
        mon_x - inset
    } else {
        mon_x + mon_w - win_w + inset
    }
}

/// 按「窗口中心不动」反推左上角（物理像素）。
/// 纯算术，抽出来单测：撑开 / 收起都不许让鼠标底下的形状跳走。
fn top_left_for_center(cx: f64, cy: f64, outer: PhysicalSize<u32>) -> PhysicalPosition<i32> {
    PhysicalPosition::new(
        (cx - outer.width as f64 / 2.0).round() as i32,
        (cy - outer.height as f64 / 2.0).round() as i32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expanding_keeps_the_orb_center_still() {
        // 常规球：窗口实测 136×84（52 高 + 上下各 16 余量；52 宽被 SM_CXMINTRACK 撑宽），
        // 球心在 (1000, 500)
        // 形状中心 (1000, 500)、轮盘 200×200、没有显示器边界
        let (pos, shift) = wheel_position(1000.0, 500.0, PhysicalSize::new(200, 200), None);

        assert_eq!(pos, PhysicalPosition::new(900, 400));
        // 没有边界就没有钳制，也就没有需要补偿的位移
        assert_eq!(shift, (0, 0));
        // 窗口矩形中心仍停在原来的形状中心
        assert_eq!(pos.x as f64 + 100.0, 1000.0);
        assert_eq!(pos.y as f64 + 100.0, 500.0);
    }

    #[test]
    fn expanding_clamps_the_wheel_into_the_monitor() {
        // 胶囊贴右边缘（窗口 1844..1980、形状中心 1912）：不钳制的话轮盘有一半在屏幕外，
        // 右侧那颗卫星按钮就点不到了 —— 钳进来正好贴住右边
        // 形状中心 (1912, 468)，轮盘 200×200，屏幕 1920×1080
        let (pos, shift) = wheel_position(
            1912.0,
            468.0,
            PhysicalSize::new(200, 200),
            Some((0, 0, 1920, 1080)),
        );
        assert_eq!(pos, PhysicalPosition::new(1720, 368));
        // 按中心本该在 x = 1912 − 100 = 1812，被钳到 1720（左移 92）——
        // 球在窗口中心会跟着左移 92，前端要按这个值反向补偿回去
        assert_eq!(shift, (-92, 0));
    }

    #[test]
    fn collapsing_restores_the_recorded_rect_exactly() {
        // 有边框（总厚度 6）时：展开补边框、收起减回去，来回都不许漂
        let geom = Collapsed {
            pos: PhysicalPosition::new(900, 400),
            size: PhysicalSize::new(136, 102),
            frame: 6,
        };

        // outer = 轮盘 200 + 边框 6 = 206；形状中心在 (968, 451)
        let (hub_pos, _) = wheel_position(968.0, 451.0, PhysicalSize::new(206, 206), None);
        assert_eq!(hub_pos, PhysicalPosition::new(865, 348));
        // 客户区（inner）中心 = 左上角 + 边框/2 + inner/2，仍应落在原形状中心 (968, 451)
        assert_eq!(hub_pos.x as f64 + 3.0 + 100.0, 968.0);
        assert_eq!(hub_pos.y as f64 + 3.0 + 100.0, 451.0);

        let (back_size, back_pos) = collapse_target(geom);
        assert_eq!(back_size, PhysicalSize::new(130, 96));
        assert_eq!(back_pos, geom.pos);
    }

    /// 收起态窗口必须给形状四周留出余量，否则投影与角标会被窗口边界切平。
    ///
    /// 现场实测（纯白底 + 亮色探针投影，160×160 样张的中心剖面）：
    /// 不留余量时竖向剖面在形状上下边缘之外**全为 0**（投影整块被切掉），
    /// 而横向 x 41..118（形状才 52 宽）是完整的渐隐 —— 因为窗口宽度被
    /// `SM_CXMINTRACK` 撑到 136，横向天然有富余，竖向只有我们肯让才行。
    /// 顺带被切掉的还有角标：球的 `top:-5px` 削掉上半截，贴边胶囊的 `top:-9px` 整颗在窗口外。
    #[test]
    fn collapsed_windows_leave_room_for_the_shadow_and_the_badge() {
        assert_eq!(collapsed_shape_h_logical(false), ORB_LOGICAL);
        assert_eq!(collapsed_shape_h_logical(true), TAB_H_LOGICAL);
        assert_eq!(collapsed_h_logical(false), ORB_LOGICAL + SHAPE_PAD_LOGICAL * 2.0);
        assert_eq!(collapsed_h_logical(true), TAB_H_LOGICAL + SHAPE_PAD_LOGICAL * 2.0);
        // 投影（球的 0 4px 12px 往下要 16px、胶囊的 0 2px 8px 往下要 10px）画得下
        assert!(SHAPE_PAD_LOGICAL >= 16.0, "常规球投影往下要 16px");
        assert!(SHAPE_PAD_LOGICAL >= 10.0, "胶囊投影往下要 10px");
        // 角标也得在窗口里：这俩值是 CSS 里 .ball-badge--corner / --top 的 top
        assert!(SHAPE_PAD_LOGICAL >= 5.0, "常规球右上角标 top:-5px");
        assert!(SHAPE_PAD_LOGICAL >= 9.0, "贴边胶囊顶部角标 top:-9px");
        // 前端的形态阈值（`BallApp.tsx` 里写的 90）必须卡在 orb 与 tab 之间
        assert!(
            collapsed_h_logical(false) < 90.0 && 90.0 < collapsed_h_logical(true),
            "BallApp 的阈值要跟着这里的余量走：orb {} / tab {}",
            collapsed_h_logical(false),
            collapsed_h_logical(true)
        );
    }

    #[test]
    fn clamping_the_shape_lets_it_touch_the_screen_edge() {
        // 竖向钳制按**形状**算：形状是 64 高、窗口是 96 高（上下各 16 透明余量），
        // 所以形状中心可以停在距边 32 处 —— 胶囊整条贴住屏幕上下边缘，
        // 探出去的是透明余量（它本来就看不见，也不吃屏幕外的点击）。
        assert_eq!(clamp_center(10, 0, 1080, 64), 32);
        assert_eq!(clamp_center(2000, 0, 1080, 64), 1048);
    }

    #[test]
    fn docking_puts_the_tab_flush_against_the_nearest_edge() {
        // 1920 宽的屏幕，窗口 136 宽、胶囊 16 宽 → 内缩 60
        // 形状中心在屏幕左半边：贴左边 = 0 − 60
        assert_eq!(docked_x(200, 136, 16, 0, 1920), -60);
        // 形状中心在右半边：贴右边 = 1920 − 136 + 60
        assert_eq!(docked_x(1800, 136, 16, 0, 1920), 1844);
        // 负坐标的副屏（显示器在主屏左边）也要算对
        assert_eq!(docked_x(-1800, 136, 16, -1920, 1920), -1980);
        // 已经贴在右边就等于它自己（幂等，不会被 onMoved 反复推）
        assert_eq!(docked_x(1844 + 8, 136, 16, 0, 1920), 1844);
    }

    #[test]
    fn clamping_keeps_a_window_inside_the_monitor() {
        assert_eq!(clamp_into(-60, 0, 1920, 136), 0);
        assert_eq!(clamp_into(1900, 0, 1920, 136), 1784);
        // 显示器比窗口还小：贴左上角，别算出负数
        assert_eq!(clamp_into(50, 0, 100, 136), 0);
    }

    #[test]
    fn undocking_keeps_the_orb_under_the_pointer() {
        // 胶囊贴左边缘：窗口 -60..76，形状（16 宽）占 0..16、中心 8，鼠标停在形状中心
        // 转常规态（52 宽）时，中心 8 会让球占到 -18..34 —— 一半在屏幕外、鼠标还落在球外。
        // 按形状钳到中心 26、窗口于是停在 -42，球占 0..52：整块可见，鼠标（8）仍在球上。
        let center = clamp_center(8, 0, 1920, 52);
        assert_eq!(center, 26);
        let win_x = center - 136 / 2;
        assert_eq!(win_x, -42);
        assert!(win_x <= 8 && 8 <= win_x + 136, "鼠标仍应落在窗口里");
        let (orb_left, orb_right) = (center - 26, center + 26);
        assert!(orb_left >= 0 && orb_right <= 1920, "球要整块留在屏幕内");
        assert!((orb_left..=orb_right).contains(&8), "鼠标仍应落在球上");
        // 球本来就在屏幕内时不该动它
        assert_eq!(clamp_center(900, 0, 1920, 52), 900);
        // 屏幕角落：中心被钳到距边 26
        assert_eq!(clamp_center(-100, 0, 1920, 52), 26);
        assert_eq!(clamp_center(5000, 0, 1920, 52), 1894);
    }

    #[test]
    fn odd_sizes_round_to_the_nearest_pixel() {
        // 收起宽度是奇数时窗口中心落在半像素上：四舍五入即可，别来回抖
        let pos = top_left_for_center(100.0, 100.0, PhysicalSize::new(135, 65));
        assert_eq!(pos, PhysicalPosition::new(33, 68));
    }
}
