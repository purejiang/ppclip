//! 悬浮球的「枢纽态」：鼠标悬停时把 56×56 的小窗口临时撑成 240×240，
//! 里面画中心球 + 四个环绕按钮（HTML 画到窗口外的部分一律被裁掉，只能撑窗）。
//!
//! 两条硬约束：
//!
//! 1. **球的屏幕位置不能动**。球在窗口里居中（收起 / 展开都是），所以撑窗要按
//!    「窗口中心 = 球心」反推新左上角；收起则照展开前记下的矩形**一比一还原** ——
//!    只按中心反推的话，边框与取整的误差会每次悬停累积一次，几十次之后球就漂走了。
//! 2. **收起尺寸不能按配置里的 56 算**。Windows 会把无边框窗口按 `SM_CXMINTRACK`
//!    撑到 136×56（见 AGENTS 已知问题 12），所以收起几何一律用展开那一刻的实测值。
//!
//! 窗口尺寸的唯一真相在这里：前端只按 `ball:hub` 事件同步 CSS（窗口被藏起来时
//! 前端收不到 mouseleave，只能靠事件兜底）。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize};

/// 收起态的窗口逻辑边长（与 `tauri.conf.json` 里的 ball 窗口一致）
pub const COLLAPSED_LOGICAL: f64 = 56.0;
/// 枢纽态的窗口逻辑边长：要放得下直径 112 的中心球 + 四个落在半径 84 圆环上的按钮
pub const HUB_LOGICAL: f64 = 240.0;
/// 枢纽态变化时广播的事件名（payload 是 bool）
pub const HUB_EVENT: &str = "ball:hub";

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
}

/// 当前是不是枢纽态
pub fn is_expanded(app: &AppHandle) -> bool {
    app.try_state::<BallHubState>()
        .is_some_and(|state| state.expanded.load(Ordering::Relaxed))
}

/// 切换枢纽态。同方向重复调用是空操作 —— 前端的 pointerenter 会重复触发。
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
    let collapsed_inner = (COLLAPSED_LOGICAL * scale).round().max(1.0) as u32;
    let frame = size.height.saturating_sub(collapsed_inner);

    let (target_size, target_pos) = if expanded {
        let geom = Collapsed { pos, size, frame };
        *state.collapsed.lock().unwrap_or_else(|e| e.into_inner()) = Some(geom);

        let edge = (HUB_LOGICAL * scale).round().max(1.0) as u32;
        expand_target(size, pos, frame, edge)
    } else {
        let recorded = *state.collapsed.lock().unwrap_or_else(|e| e.into_inner());
        match recorded {
            Some(geom) => collapse_target(geom),
            // 没记过（正常流程不会发生）：退回按中心反推，至少别把球留在窗口外
            None => {
                let cx = pos.x as f64 + size.width as f64 / 2.0;
                let cy = pos.y as f64 + size.height as f64 / 2.0;
                (
                    PhysicalSize::new(collapsed_inner, collapsed_inner),
                    top_left_for_center(cx, cy, size),
                )
            }
        }
    };

    // 先改尺寸再挪位置：中间那一帧 WebView 还没重排，球看着还在原处；
    // 反过来先挪位置会让球在屏幕上先跳一下。
    state.expanded.store(expanded, Ordering::Relaxed);
    win.set_size(target_size)
        .map_err(|e| format!("调整悬浮球尺寸失败: {e}"))?;
    win.set_position(target_pos)
        .map_err(|e| format!("调整悬浮球位置失败: {e}"))?;

    // 前端只是镜像：同步它那边的 CSS 状态（截图前藏窗口 / 隐藏悬浮球时收不到 mouseleave）
    let _ = app.emit_to("ball", HUB_EVENT, expanded);
    log::debug!(
        "悬浮球{}：{}×{} @({}, {}) → {}×{} @({}, {})",
        if expanded { "展开为枢纽" } else { "收回" },
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

/// 展开成枢纽：inner 取 `edge`，位置让**窗口矩形**的中心停在原来的球心上。
/// `set_size` 收的是 inner，所以算位置要先自己补上边框。
fn expand_target(
    size: PhysicalSize<u32>,
    pos: PhysicalPosition<i32>,
    frame: u32,
    edge: u32,
) -> (PhysicalSize<u32>, PhysicalPosition<i32>) {
    let cx = pos.x as f64 + size.width as f64 / 2.0;
    let cy = pos.y as f64 + size.height as f64 / 2.0;
    let outer = PhysicalSize::new(edge + frame, edge + frame);
    (PhysicalSize::new(edge, edge), top_left_for_center(cx, cy, outer))
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

/// 按「窗口矩形中心不动」反推左上角（物理像素）。
/// 纯算术，抽出来单测：展开 / 收起都不许让鼠标底下的球跳走。
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
    fn expanding_keeps_the_ball_center_still() {
        // 本机收起态实测 136×56（56 被 SM_CXMINTRACK 撑宽），球心在 (1000, 500)
        let geom = Collapsed {
            pos: PhysicalPosition::new(932, 472),
            size: PhysicalSize::new(136, 56),
            frame: 0,
        };

        let (size, pos) = expand_target(geom.size, geom.pos, geom.frame, 240);

        assert_eq!(size, PhysicalSize::new(240, 240));
        assert_eq!(pos, PhysicalPosition::new(880, 380));
        // 窗口矩形中心仍是原来的球心
        assert_eq!(pos.x as f64 + size.width as f64 / 2.0, 1000.0);
        assert_eq!(pos.y as f64 + size.height as f64 / 2.0, 500.0);
    }

    #[test]
    fn collapsing_restores_the_recorded_rect_exactly() {
        // 有边框（总厚度 6）时：展开补边框、收起减回去，来回都不许漂
        let geom = Collapsed {
            pos: PhysicalPosition::new(900, 400),
            size: PhysicalSize::new(136, 62),
            frame: 6,
        };

        let (edge, hub_pos) = expand_target(geom.size, geom.pos, geom.frame, 240);
        assert_eq!(edge, PhysicalSize::new(240, 240));
        assert_eq!(hub_pos, PhysicalPosition::new(845, 308));
        // 客户区（inner）中心 = 左上角 + 边框/2 + inner/2，仍应落在原球心 (968, 431)
        assert_eq!(hub_pos.x as f64 + 3.0 + 120.0, 968.0);
        assert_eq!(hub_pos.y as f64 + 3.0 + 120.0, 431.0);

        let (back_size, back_pos) = collapse_target(geom);
        assert_eq!(back_size, PhysicalSize::new(130, 56));
        assert_eq!(back_pos, geom.pos);
    }

    #[test]
    fn odd_sizes_round_to_the_nearest_pixel() {
        // 收起宽度是奇数时窗口中心落在半像素上：四舍五入即可，别来回抖
        let pos = top_left_for_center(100.0, 100.0, PhysicalSize::new(135, 57));
        assert_eq!(pos, PhysicalPosition::new(33, 72));
    }
}
