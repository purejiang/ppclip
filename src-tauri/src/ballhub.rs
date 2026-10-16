//! 悬浮球的三种形态（照 stitch 稿「桌面悬浮球三态规范」）：
//!
//! | 形态 | 尺寸 | 什么时候 |
//! |---|---|---|
//! | 贴边胶囊 Collapsed | 16×64 | 拖到屏幕左 / 右边缘自动吸附（对称胶囊，左右通用） |
//! | 常规悬浮球 Floating Orb | 52×52 | 常态：桌面上任意位置 |
//! | 展开态 HUD Wheel | 200×200 | 鼠标在胶囊 / 球上停 0.3 秒；离开收回 |
//!
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
//! 窗口尺寸的唯一真相在这里：前端只按 `ball:state` 事件切 CSS（窗口被藏起来时
//! 前端收不到 mouseleave，只能靠事件兜底）。

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
/// 形状中心离屏幕左右边缘多近（逻辑像素）就吸附成胶囊
const DOCK_MARGIN: f64 = 40.0;

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

/// 收起态当前形态应有的窗口**逻辑**高度
fn collapsed_h_logical(docked: bool) -> f64 {
    if docked {
        TAB_H_LOGICAL
    } else {
        ORB_LOGICAL
    }
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

    // 边框厚度用高度算：宽度会被 SM_CXMINTRACK 撑（那是窗口矩形的下限，不是边框）
    let collapsed_inner_h = (collapsed_h_logical(docked) * scale).round().max(1.0) as u32;
    let frame = size.height.saturating_sub(collapsed_inner_h);

    let (target_size, target_pos) = if expanded {
        let geom = Collapsed { pos, size, frame };
        *state.collapsed.lock().unwrap_or_else(|e| e.into_inner()) = Some(geom);

        let edge = (WHEEL_LOGICAL * scale).round().max(1.0) as u32;
        let bounds = monitor_bounds(&win);
        expand_target(size, pos, frame, edge, bounds)
    } else {
        let recorded = *state.collapsed.lock().unwrap_or_else(|e| e.into_inner());
        match recorded {
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
        }
    };

    // 先改尺寸再挪位置：中间那一帧 WebView 还没重排，形状看着还在原处；
    // 反过来先挪位置会让它在屏幕上先跳一下。
    state.expanded.store(expanded, Ordering::Relaxed);
    win.set_size(target_size)
        .map_err(|e| format!("调整悬浮球尺寸失败: {e}"))?;
    win.set_position(target_pos)
        .map_err(|e| format!("调整悬浮球位置失败: {e}"))?;

    log::debug!(
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
    // 边框厚度照**当前实测**的 outer−inner 算，不按目标形态反推：
    // 胶囊（64 高）转常规球（52 高）时按 52 反推会凭空多出 12px，位置算歪、球会跳一下
    let frame = outer.height.saturating_sub(inner.height);
    let outer_h = inner_h + frame;
    let (win_w, win_h) = (outer.width as i32, outer.height as i32);

    // 形状中心（= 窗口中心）
    let cx = pos.x as f64 + outer.width as f64 / 2.0;
    let cy = pos.y as f64 + outer.height as f64 / 2.0;

    let mut x = pos.x;
    let mut y = (cy - outer_h as f64 / 2.0).round() as i32;
    if let Some((mx, my, mw, mh)) = monitor_bounds(&win) {
        // 钳制一律按**形状**算，不按窗口：窗口比形状宽得多（136 vs 16/52），
        // 按窗口钳会把球从鼠标底下推走 60px —— 贴边态转常规态时鼠标就停在形状上，
        // 那一下推走直接让鼠标落在球外面（点不到、也悬停不到）。
        let center_y = clamp_center(cy.round() as i32, my, mh, inner_h as i32);
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

    state.docked.store(docked, Ordering::Relaxed);
    if inner.height != inner_h {
        win.set_size(PhysicalSize::new(inner.width, inner_h))
            .map_err(|e| format!("调整悬浮球尺寸失败: {e}"))?;
    }
    if x != pos.x || y != pos.y || outer_h != outer.height {
        win.set_position(PhysicalPosition::new(x, y))
            .map_err(|e| format!("调整悬浮球位置失败: {e}"))?;
    }
    log::info!(
        "悬浮球形态 → {}：{}×{} @({}, {}) → {}×{} @({}, {})",
        state_name(app),
        win_w,
        win_h,
        pos.x,
        pos.y,
        outer.width,
        outer_h,
        x,
        y
    );
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
    let Some((mx, _my, mw, _mh)) = monitor_bounds(&win) else {
        return Ok(is_docked(app));
    };

    // 判定用**形状**中心离边缘的距离：窗口中心会因为窗口被撑宽而偏（136 宽里形状只占 52）
    let shape_w = (collapsed_w_logical(is_docked(app)) * scale).round().max(1.0) as i32;
    let shape_left = pos.x + (size.width as i32 - shape_w) / 2;
    let shape_center = shape_left + shape_w / 2;
    let margin = (DOCK_MARGIN * scale).round() as i32;
    let near_left = shape_center - mx <= margin;
    let near_right = (mx + mw) - shape_center <= margin;
    let want_docked = near_left || near_right;

    // 即使形态没变也要走一遍 `set_docked`：它会把窗口尺寸与位置摆正
    // （窗口高度按形态是 64 / 52，位置按形状中心重新算）
    set_docked(app, want_docked)
}

/// 当前显示器的物理范围 `(x, y, w, h)`
fn monitor_bounds(win: &tauri::WebviewWindow) -> Option<(i32, i32, i32, i32)> {
    win.current_monitor()
        .ok()
        .flatten()
        .map(|monitor| {
            let origin = monitor.position();
            let area = monitor.size();
            (origin.x, origin.y, area.width as i32, area.height as i32)
        })
}

/// 展开成轮盘：inner 取 `edge`，位置让**窗口矩形**的中心停在形状中心，再整体钳进显示器。
///
/// 钳制很关键：胶囊是贴屏幕边缘停靠的，200×200 的轮盘以它为中心会有一半在屏幕外 ——
/// 那一侧的卫星按钮就点不到了。钳进来之后轮盘会看着「从边缘滑进来」，
/// 收起重置时照旧一比一还原，不会漂。
fn expand_target(
    size: PhysicalSize<u32>,
    pos: PhysicalPosition<i32>,
    frame: u32,
    edge: u32,
    bounds: Option<(i32, i32, i32, i32)>,
) -> (PhysicalSize<u32>, PhysicalPosition<i32>) {
    let cx = pos.x as f64 + size.width as f64 / 2.0;
    let cy = pos.y as f64 + size.height as f64 / 2.0;
    let outer = PhysicalSize::new(edge + frame, edge + frame);
    let mut target = top_left_for_center(cx, cy, outer);

    if let Some((bx, by, bw, bh)) = bounds {
        target.x = clamp_into(target.x, bx, bw, outer.width as i32);
        target.y = clamp_into(target.y, by, bh, outer.height as i32);
    }
    (PhysicalSize::new(edge, edge), target)
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
        // 常规球：窗口实测 136×52（52 被 SM_CXMINTRACK 撑宽），球心在 (1000, 500)
        let geom = Collapsed {
            pos: PhysicalPosition::new(932, 474),
            size: PhysicalSize::new(136, 52),
            frame: 0,
        };

        let (size, pos) = expand_target(geom.size, geom.pos, geom.frame, 200, None);

        assert_eq!(size, PhysicalSize::new(200, 200));
        assert_eq!(pos, PhysicalPosition::new(900, 400));
        // 窗口矩形中心仍是原来的形状中心
        assert_eq!(pos.x as f64 + size.width as f64 / 2.0, 1000.0);
        assert_eq!(pos.y as f64 + size.height as f64 / 2.0, 500.0);
    }

    #[test]
    fn expanding_clamps_the_wheel_into_the_monitor() {
        // 胶囊贴右边缘（窗口 1844..1980、形状中心 1912）：不钳制的话轮盘有一半在屏幕外，
        // 右侧那颗卫星按钮就点不到了 —— 钳进来正好贴住右边
        let geom = Collapsed {
            pos: PhysicalPosition::new(1844, 420),
            size: PhysicalSize::new(136, 64),
            frame: 0,
        };

        let (_, pos) = expand_target(
            geom.size,
            geom.pos,
            geom.frame,
            200,
            Some((0, 0, 1920, 1080)),
        );
        assert_eq!(pos, PhysicalPosition::new(1720, 352));
    }

    #[test]
    fn collapsing_restores_the_recorded_rect_exactly() {
        // 有边框（总厚度 6）时：展开补边框、收起减回去，来回都不许漂
        let geom = Collapsed {
            pos: PhysicalPosition::new(900, 400),
            size: PhysicalSize::new(136, 70),
            frame: 6,
        };

        let (edge, hub_pos) = expand_target(geom.size, geom.pos, geom.frame, 200, None);
        assert_eq!(edge, PhysicalSize::new(200, 200));
        assert_eq!(hub_pos, PhysicalPosition::new(865, 332));
        // 客户区（inner）中心 = 左上角 + 边框/2 + inner/2，仍应落在原形状中心 (968, 435)
        assert_eq!(hub_pos.x as f64 + 3.0 + 100.0, 968.0);
        assert_eq!(hub_pos.y as f64 + 3.0 + 100.0, 435.0);

        let (back_size, back_pos) = collapse_target(geom);
        assert_eq!(back_size, PhysicalSize::new(130, 64));
        assert_eq!(back_pos, geom.pos);
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
