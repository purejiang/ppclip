//! 悬浮球的**纯几何**：全部是算术，不碰窗口、不碰 AppHandle。
//!
//! 这里是几何的唯一真相。`ballhub.rs` 只负责「什么时候调哪个函数」，所有
//! 「窗口该放哪 / 形状中心在哪 / 该不该吸附 / 轮盘该摆哪」都在这一层算完，
//! 并且全部有单测。
//!
//! ## 两条不变量（改这里之前先记住）
//!
//! 1. **收起态：形状中心 = 窗口中心**。胶囊和球都在窗口里居中，所以「窗口矩形」永远
//!    可以由「形状中心 + 窗口尺寸」反推（`window_pos`），反过来也成立（`anchor_of`）。
//!    贴边胶囊靠把窗口横向推出屏幕来实现「形状贴住屏幕边」，形状中心本身不动 ——
//!    所以**撑开 / 收起 / tab↔orb 切换都不许让形状中心漂走**。
//!    （展开态是唯一的例外：那扇窗里居中装的是**盘**，盘心允许被钳离 anchor 最多 74px，
//!    见 `wheel_layout`。轮盘展开时球已经藏起来了，不会同时出现在屏幕上。）
//! 2. **anchor 是形状中心（物理像素，全局坐标）**。它是几何的唯一入口：读一次
//!    （拖动结束 / 启动）→ 钳制 → 算窗口矩形 → 写回。不再有「读窗口 → 算 → 写窗口」
//!    的来回，因此也不会累积取整误差（旧实现每次悬停都漂一点，几十次之后球就跑偏了）。
//!
//! 窗口尺寸**不是常量**：无边框窗口的宽度被 Windows 的 `SM_CXMINTRACK` 顶着
//! （见 AGENTS 已知问题 12），不同 DPI 下还可能变。所以这里的函数一律**接收实测的
//! 窗口物理尺寸**，不许在内部写死 136 / 96。

use serde::Serialize;
use tauri::{PhysicalPosition, PhysicalSize};

/// 贴边胶囊（稿子 State 01）的逻辑尺寸
pub const TAB_W_LOGICAL: f64 = 16.0;
pub const TAB_H_LOGICAL: f64 = 64.0;
/// 常规悬浮球（稿子 State 02）的逻辑尺寸
pub const ORB_LOGICAL: f64 = 52.0;
/// 展开态轮盘（稿子 State 03 是 240×240，这里按「太大了」的反馈收到 200）
pub const WHEEL_LOGICAL: f64 = 200.0;
/// 形状四周留出的透明余量（逻辑像素）：**投影与角标要画得下**。
///
/// 竖向必须留：窗口宽度我们说了不算（`SM_CXMINTRACK` 会把无边框窗口撑宽，
/// 形状两侧天然有富余），但**高度是我们定的** —— 只要客户区高度 = 形状高度，
/// 形状外的每一像素都在窗口外，`.ball-orb-body` 的投影会被切得干干净净，
/// 右上角计数角标也会被削掉上半截。
pub const SHAPE_PAD_LOGICAL: f64 = 16.0;
/// 轮盘窗口比轮盘本身大一圈（逻辑像素，四周各一份）。
///
/// 200×200 的窗口装 200 的盘看着刚好，其实盘面那圈 `0 12px 40px` 的投影会被窗口边界
/// 切平（以前就是这样，盘看着像贴在一块硬纸板上）。留 16 之后投影画得下，卫星按钮
/// 的 hover 描边也不会顶到窗口边。
pub const WHEEL_PAD_LOGICAL: f64 = 16.0;
/// 吸附判定：**球形状的边**离屏幕左右边多近（逻辑像素）就吸成胶囊。
///
/// 为什么用形状边、不用形状中心：窗口宽 136 物理、球才 52，窗口贴住屏幕时
/// 球心离屏幕边还有 68px —— 按中心判定（旧实现 `DOCK_MARGIN = 60`）在 100% 缩放下
/// **永远判不中**，这就是「拖到边上不吸」的真因。按形状边算，球边距边 ≤ 24 就吸，
/// 与窗口被撑多宽无关。
pub const DOCK_EDGE_MARGIN_LOGICAL: f64 = 24.0;

/// 显示器（或任意矩形）的物理范围
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MonitorRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl MonitorRect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }

    pub const fn right(&self) -> i32 {
        self.x + self.w
    }

    pub const fn bottom(&self) -> i32 {
        self.y + self.h
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    /// 点到这块矩形的**最短距离的平方**（落在矩形内为 0）。用来给拔掉副屏后的
    /// anchor 找一块最近的屏。
    fn distance_sq(&self, x: i32, y: i32) -> i64 {
        let dx = if x < self.x {
            (self.x - x) as i64
        } else if x >= self.right() {
            (x - self.right() + 1) as i64
        } else {
            0
        };
        let dy = if y < self.y {
            (self.y - y) as i64
        } else if y >= self.bottom() {
            (y - self.bottom() + 1) as i64
        } else {
            0
        };
        dx * dx + dy * dy
    }
}

/// 形状中心（物理像素，全局坐标）—— 整个悬浮球的几何唯一真相
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Anchor {
    pub x: i32,
    pub y: i32,
}

impl Anchor {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// 收起态的两种形态。轮盘不在这里 —— 它不是第三种形态，是同一扇窗撑大之后的内容。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BallForm {
    Tab,
    Orb,
}

impl BallForm {
    pub fn name(self) -> &'static str {
        match self {
            BallForm::Tab => "tab",
            BallForm::Orb => "orb",
        }
    }

    pub fn from_docked(docked: bool) -> Self {
        if docked {
            BallForm::Tab
        } else {
            BallForm::Orb
        }
    }
}

/// 胶囊贴哪一侧的屏幕边
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DockSide {
    Left,
    Right,
}

/// 轮盘的窗口矩形
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WheelLayout {
    /// 轮盘窗口左上角（含四周 `WHEEL_PAD_LOGICAL` 的透明余量，允许探出屏幕）
    pub pos: PhysicalPosition<i32>,
    /// 轮盘窗口的期望物理尺寸
    pub size: PhysicalSize<u32>,
}

/// 逻辑像素 → 物理像素（至少 1，别算出 0 尺寸的窗口）
pub fn to_physical(logical: f64, scale: f64) -> i32 {
    let scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
    (logical * scale).round().max(1.0) as i32
}

/// 形状的物理尺寸（tab 16×64 / orb 52×52）
pub fn shape_size(form: BallForm, scale: f64) -> (i32, i32) {
    match form {
        BallForm::Tab => (
            to_physical(TAB_W_LOGICAL, scale),
            to_physical(TAB_H_LOGICAL, scale),
        ),
        BallForm::Orb => {
            let side = to_physical(ORB_LOGICAL, scale);
            (side, side)
        }
    }
}

/// 收起态窗口的**期望**物理尺寸：宽度装得下最宽的形状 + 两侧余量，
/// 高度装得下最高的形状 + 上下余量。
///
/// 实际尺寸只能更宽（Windows 的 `SM_CXMINTRACK`），不会更窄；所以调用方拿到它之后
/// 要跟**实测值**取 max，再决定要不要 `set_size` 一次（启动时窗口还藏着，看不见）。
pub fn ideal_window_size(scale: f64) -> PhysicalSize<u32> {
    let widest = TAB_W_LOGICAL.max(ORB_LOGICAL);
    let tallest = TAB_H_LOGICAL.max(ORB_LOGICAL);
    PhysicalSize::new(
        to_physical(widest + SHAPE_PAD_LOGICAL * 2.0, scale) as u32,
        to_physical(tallest + SHAPE_PAD_LOGICAL * 2.0, scale) as u32,
    )
}

/// 窗口左上角 + 窗口尺寸 → 形状中心（形状在窗口里居中）
pub fn anchor_of(pos: PhysicalPosition<i32>, win: PhysicalSize<u32>) -> Anchor {
    Anchor::new(
        pos.x + win.width as i32 / 2,
        pos.y + win.height as i32 / 2,
    )
}

/// 形状中心 + 窗口尺寸 → 窗口左上角（形状在窗口里居中）
///
/// 就这一行，但它是「形状中心不动」的全部实现：tab↔orb、吸附、启动摆正都只调它。
pub fn window_pos(anchor: Anchor, win: PhysicalSize<u32>) -> PhysicalPosition<i32> {
    PhysicalPosition::new(
        anchor.x - win.width as i32 / 2,
        anchor.y - win.height as i32 / 2,
    )
}

/// 形状中心离屏幕哪一侧更近（只用于胶囊贴哪边）
pub fn dock_side(anchor: Anchor, mon: MonitorRect) -> DockSide {
    if (anchor.x - mon.x) * 2 <= mon.w {
        DockSide::Left
    } else {
        DockSide::Right
    }
}

/// 把 anchor 钳到「整个形状都在显示器内」的范围。
///
/// - `Orb`：横竖都按球径钳（球整块可见）；
/// - `Tab`：横向由边决定（贴住就是贴住，**钳制会把它推离屏幕边**，所以直接赋值），
///   纵向按胶囊高度钳 —— 胶囊能贴到屏幕上下边缘。
///
/// 注意这里钳的是**形状**不是窗口：窗口两侧那片透明区允许探出屏幕（贴边态正是靠它
/// 把吃点击的透明区推到屏幕外）。
pub fn clamp_anchor(
    form: BallForm,
    side: DockSide,
    anchor: Anchor,
    mon: MonitorRect,
    scale: f64,
) -> Anchor {
    let (sw, sh) = shape_size(form, scale);
    let half_w = (sw / 2).max(0);
    let x = match form {
        BallForm::Tab => match side {
            DockSide::Left => mon.x + half_w,
            DockSide::Right => mon.right() - half_w,
        },
        BallForm::Orb => clamp_center(anchor.x, mon.x, mon.w, sw),
    };
    let y = clamp_center(anchor.y, mon.y, mon.h, sh);
    Anchor::new(x, y)
}

/// 把**形状中心**钳到「形状整块留在区间内」的范围里（纯算术，单测）
pub fn clamp_center(center: i32, start: i32, extent: i32, shape: i32) -> i32 {
    let half = (shape / 2).max(0);
    center.clamp(start + half, (start + extent - half).max(start + half))
}

/// 拖动落在屏幕边缘附近了吗 —— **整个项目里唯一判定吸附的地方**。
///
/// 判据是**当前形态的形状左右边**离屏幕边的空隙 ≤ `margin_logical`：
/// - 按形状中心算 → 窗口被撑宽之后球心恒距窗口左沿 68 物理像素，100% 缩放下永远判不中
///   （旧实现 `DOCK_MARGIN = 60`，这就是「拖到边上不吸」的真因）；
/// - 按窗口边算 → 窗口贴住屏幕时球离边还有 42px，会在球明明离得很远的时候就吸住。
///
/// 传**当前形态**是为了拿到天然的回差：常规球（52）边距 ≤ 24 就吸（中心 ≤ 50），
/// 胶囊（16）边距 > 24 才弹回（中心 > 32）—— 吸上去之后不会因为手抖一两像素就松掉。
///
/// 幂等：胶囊已经贴边时边距是 0，仍判定为「该吸」，所以「拖到边 → 吸 → 再松手」
/// 不会来回切换。只在拖动结束时调用。
pub fn should_dock(
    form: BallForm,
    anchor: Anchor,
    mon: MonitorRect,
    scale: f64,
    margin_logical: f64,
) -> bool {
    let half = shape_size(form, scale).0 / 2;
    let margin = to_physical(margin_logical, scale);
    let left_gap = (anchor.x - half) - mon.x;
    let right_gap = mon.right() - (anchor.x + half);
    left_gap <= margin || right_gap <= margin
}

/// 轮盘窗口应该摆在哪。
///
/// **钳的是盘，不是窗口**：贴边时以球心为圆心的 200×200 有一半在屏幕外，那一侧的
/// 卫星按钮就点不到了 —— 所以盘必须整块进屏幕。窗口四周那圈 `WHEEL_PAD` 是透明的
/// （画投影用），允许探出去。
///
/// 盘被钳向内之后**盘心不再等于球心**（最多 74px）。这是有意的、也是唯一能同时成立的
/// 摆法：盘必须整块可见，中心盘又必须在盘心（CSS 里它就居中在盘里）。**别再加一个
/// 「把中心盘画回球的位置」的补偿** —— 那会把中心球推到盘边缘、看起来像球飞了
/// （真机反馈过）。轮盘展开时球已经藏起来了，没人看见盘心的这点位移。
pub fn wheel_layout(anchor: Anchor, mon: MonitorRect, scale: f64) -> WheelLayout {
    let disc = to_physical(WHEEL_LOGICAL, scale);
    let pad = to_physical(WHEEL_PAD_LOGICAL, scale);
    wheel_layout_in(anchor, mon, scale, PhysicalSize::new((disc + pad * 2).max(1) as u32, (disc + pad * 2).max(1) as u32))
}

/// 同上，但按**实测的窗口尺寸**重算。
///
/// `set_size` 之后才能知道窗口的真尺寸（跨 DPI、或者将来改了 pad 都可能与期望不符），
/// 这时不能盲信 `wheel_layout` 的推算值 —— 盘必须继续居中、继续整块在屏幕里。
/// 隐藏状态下改几何，用户看不见。
pub fn wheel_layout_in(
    anchor: Anchor,
    mon: MonitorRect,
    scale: f64,
    win: PhysicalSize<u32>,
) -> WheelLayout {
    let disc = to_physical(WHEEL_LOGICAL, scale);
    let (win_w, win_h) = (win.width as i32, win.height as i32);

    // 钳的是**盘**，不是窗口：盘必须整块进屏幕（否则那一侧的卫星按钮点不到），
    // 窗口四周那圈 pad 是透明的，允许探出去（画投影用）。
    let disc_x = clamp_into(anchor.x - disc / 2, mon.x, mon.w, disc);
    let disc_y = clamp_into(anchor.y - disc / 2, mon.y, mon.h, disc);

    // 盘在窗口里居中 → 由盘的位置反推窗口左上角（窗口比盘宽出来的部分是 pad，两侧均分）
    let pos = PhysicalPosition::new(
        disc_x - (win_w - disc).max(0) / 2,
        disc_y - (win_h - disc).max(0) / 2,
    );

    WheelLayout { pos, size: win }
}

/// 拔掉副屏后 anchor 可能落在所有显示器之外：找最近的那块兜底。
pub fn nearest_monitor(anchor: Anchor, monitors: &[MonitorRect]) -> Option<MonitorRect> {
    monitors
        .iter()
        .copied()
        .min_by_key(|m| m.distance_sq(anchor.x, anchor.y))
}

/// anchor 落在哪块显示器上
pub fn monitor_at(anchor: Anchor, monitors: &[MonitorRect]) -> Option<MonitorRect> {
    monitors
        .iter()
        .copied()
        .find(|m| m.contains(anchor.x, anchor.y))
}

/// 把长度 `size` 的矩形钳进 `[start, start+extent]`（显示器比它还小时贴左上角）
pub fn clamp_into(value: i32, start: i32, extent: i32, size: i32) -> i32 {
    value.clamp(start, (start + extent - size).max(start))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: MonitorRect = MonitorRect::new(0, 0, 1920, 1080);

    fn size(w: u32, h: u32) -> PhysicalSize<u32> {
        PhysicalSize::new(w, h)
    }

    #[test]
    fn shapes_scale_with_dpi() {
        assert_eq!(shape_size(BallForm::Tab, 1.0), (16, 64));
        assert_eq!(shape_size(BallForm::Orb, 1.0), (52, 52));
        assert_eq!(shape_size(BallForm::Tab, 1.5), (24, 96));
        assert_eq!(shape_size(BallForm::Orb, 1.5), (78, 78));
        // 0 / 负数 / NaN 一律当 1，绝不产出 0 尺寸
        assert_eq!(to_physical(52.0, 0.0), 52);
        assert_eq!(to_physical(0.0, 1.0), 1);
    }

    /// 窗口必须装得下最宽 / 最高的形状，还要留出投影与角标的余量。
    #[test]
    fn window_fits_every_shape_with_padding() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let win = ideal_window_size(scale);
            for form in [BallForm::Tab, BallForm::Orb] {
                let (sw, sh) = shape_size(form, scale);
                assert!(
                    sw as u32 + 2 * to_physical(SHAPE_PAD_LOGICAL, scale) as u32 <= win.width,
                    "{:?} 宽放不下：形状 {} + 两侧余量 > 窗口 {}",
                    form,
                    sw,
                    win.width
                );
                assert!(
                    sh as u32 + 2 * to_physical(SHAPE_PAD_LOGICAL, scale) as u32 <= win.height,
                    "{:?} 高放不下",
                    form
                );
            }
        }
    }

    /// 形状中心 → 窗口左上角 → 形状中心，必须完全还原（这是「球不跳」的机器保证）
    #[test]
    fn window_pos_and_anchor_roundtrip() {
        for win in [size(136, 96), size(135, 65), size(203, 143)] {
            for anchor in [Anchor::new(0, 0), Anchor::new(1000, 500), Anchor::new(-1920, 12)] {
                let pos = window_pos(anchor, win);
                let back = anchor_of(pos, win);
                assert_eq!(back, anchor, "window={:?} anchor={:?}", win, anchor);
            }
        }
    }

    /// 贴边胶囊：窗口被横向推出屏幕，形状正好贴住那侧屏幕边
    #[test]
    fn docked_tab_sits_flush_against_the_edge() {
        // 窗口 136 宽、胶囊 16 宽 → 窗口左上角 = 屏幕边 − 60
        let tab = Anchor::new(8, 500);
        let pos = window_pos(tab, size(136, 96));
        assert_eq!(pos.x, -60);
        // 形状仍整块在屏幕内：0..16
        assert_eq!(pos.x + (136 - 16) / 2, 0);

        // 右边缘：形状右沿贴住 1920
        let tab_r = Anchor::new(1912, 500);
        let pos_r = window_pos(tab_r, size(136, 96));
        assert_eq!(pos_r.x + (136 + 16) / 2, 1920);

        // 负坐标副屏（显示器在主屏左边）同样成立
        let mon = MonitorRect::new(-1920, 0, 1920, 1080);
        let tab_l = clamp_anchor(BallForm::Tab, DockSide::Left, Anchor::new(-1900, 500), mon, 1.0);
        assert_eq!(tab_l.x, -1912);
        let pos_l = window_pos(tab_l, size(136, 96));
        assert_eq!(pos_l.x, -1980);
    }

    /// 贴边 → 常规球：球要整块留在屏幕内，而且鼠标（仍在原胶囊中心）得落在球上
    #[test]
    fn undocking_keeps_the_orb_under_the_pointer() {
        // 胶囊贴左：形状中心 8，转常规球（52）时中心被钳到 26 → 球占 0..52
        let center = clamp_anchor(BallForm::Orb, DockSide::Left, Anchor::new(8, 500), SCREEN, 1.0);
        assert_eq!(center.x, 26);
        assert!(0 <= center.x - 26 && center.x + 26 <= 1920, "球要整块可见");
        assert!((center.x - 26..=center.x + 26).contains(&8), "鼠标仍在球上");
        // 窗口跟着摆到 -42（136 宽、球居中）
        assert_eq!(window_pos(center, size(136, 96)).x, -42);

        // 球本来就在屏幕内，不该动它
        assert_eq!(
            clamp_anchor(BallForm::Orb, DockSide::Left, Anchor::new(900, 500), SCREEN, 1.0).x,
            900
        );
        // 屏幕角落 / 屏幕外，都钳到距边 26
        assert_eq!(
            clamp_anchor(BallForm::Orb, DockSide::Left, Anchor::new(-100, -100), SCREEN, 1.0),
            Anchor::new(26, 26)
        );
        assert_eq!(
            clamp_anchor(BallForm::Orb, DockSide::Left, Anchor::new(5000, 5000), SCREEN, 1.0),
            Anchor::new(1894, 1054)
        );
    }

    /// 胶囊的钳制按形状高度算，允许贴到屏幕上下边缘（窗口外那圈余量是透明的）
    #[test]
    fn docking_lets_the_tab_touch_the_top_and_bottom() {
        let clamped = clamp_anchor(BallForm::Tab, DockSide::Left, Anchor::new(8, 10), SCREEN, 1.0);
        assert_eq!(clamped.y, 32);
        let clamped = clamp_anchor(BallForm::Tab, DockSide::Left, Anchor::new(8, 2000), SCREEN, 1.0);
        assert_eq!(clamped.y, 1048);
    }

    /// **P1 回归测试**：吸附判定用形状的边，不用形状中心。
    ///
    /// 旧实现按中心算、阈值 60×scale，而球心恒距窗口左沿 68 物理像素 —— 100% 缩放下
    /// 拖到屏幕边也判不中（这就是「没有自动贴边」）。
    #[test]
    fn docking_is_judged_by_the_orb_edge_not_the_center() {
        let orb = BallForm::Orb;
        // 球 52 宽、margin 24 → 球左边 = 中心 − 26
        assert!(should_dock(orb, Anchor::new(50, 500), SCREEN, 1.0, 24.0), "球边距边 24");
        assert!(!should_dock(orb, Anchor::new(51, 500), SCREEN, 1.0, 24.0), "球边距边 25");
        assert!(!should_dock(orb, Anchor::new(80, 500), SCREEN, 1.0, 24.0), "球边距边 54");
        // 球心 68 正是「窗口贴住屏幕」那一刻（窗口 136 宽、球居中）—— 但球离边还有 42，
        // 用户把球拖到边时球心会继续往 0 走（窗口允许探出屏幕），所以这里不该吸。
        assert!(!should_dock(orb, Anchor::new(68, 500), SCREEN, 1.0, 24.0));
        // 球真正贴住边（球边 0）就吸
        assert!(should_dock(orb, Anchor::new(26, 500), SCREEN, 1.0, 24.0));
        // 右边缘对称：球右边 = 中心 + 26，贴到 1894 才够 24 的空隙
        assert!(should_dock(orb, Anchor::new(1894, 500), SCREEN, 1.0, 24.0));
        assert!(!should_dock(orb, Anchor::new(1868, 500), SCREEN, 1.0, 24.0));
        assert!(!should_dock(orb, Anchor::new(1840, 500), SCREEN, 1.0, 24.0));
        // 屏幕中间谁都不吸
        assert!(!should_dock(orb, Anchor::new(960, 500), SCREEN, 1.0, 24.0));
    }

    /// 吸附带天然回差：球要贴到 50 才吸，吸成胶囊之后要拖到 33 才松 —— 手抖不会来回切
    #[test]
    fn docking_has_hysteresis_between_the_two_shapes() {
        let tab = BallForm::Tab;
        // 胶囊贴边时中心在 8；拖到 32 仍然吸（边距 24）
        assert!(should_dock(tab, Anchor::new(32, 500), SCREEN, 1.0, 24.0));
        // 拖过 32 就松手变回球
        assert!(!should_dock(tab, Anchor::new(33, 500), SCREEN, 1.0, 24.0));
        // 球那一侧：中心 50（球边距 24）才吸，51 就不吸 —— 与胶囊的 32 之间留出回差带
        assert!(should_dock(BallForm::Orb, Anchor::new(50, 500), SCREEN, 1.0, 24.0));
        assert!(!should_dock(BallForm::Orb, Anchor::new(51, 500), SCREEN, 1.0, 24.0));
    }

    /// 吸住之后再判一次仍然吸（不会「吸上 → 松手 → 弹回 → 再吸」来回抖）
    #[test]
    fn docking_is_idempotent() {
        let first = clamp_anchor(
            BallForm::Tab,
            dock_side(Anchor::new(50, 500), SCREEN),
            Anchor::new(50, 500),
            SCREEN,
            1.0,
        );
        assert_eq!(first.x, 8);
        assert!(
            should_dock(BallForm::Tab, first, SCREEN, 1.0, 24.0),
            "贴边态必须仍判定为贴边"
        );
        // 再摆一次，几何完全不变
        let again = clamp_anchor(
            BallForm::Tab,
            dock_side(first, SCREEN),
            first,
            SCREEN,
            1.0,
        );
        assert_eq!(first, again);
        assert_eq!(
            window_pos(first, size(136, 96)),
            window_pos(again, size(136, 96))
        );
    }

    #[test]
    fn dock_side_follows_the_half_of_the_screen() {
        assert_eq!(dock_side(Anchor::new(100, 500), SCREEN), DockSide::Left);
        assert_eq!(dock_side(Anchor::new(1800, 500), SCREEN), DockSide::Right);
        // 正中线归左（固定行为，别让它随浮点抖动）
        assert_eq!(dock_side(Anchor::new(960, 500), SCREEN), DockSide::Left);
    }

    /// 轮盘：盘必须整块进屏幕（否则那一侧的卫星按钮点不到），并且**盘始终居中在窗口里**
    /// —— CSS 的中心盘 `.ball-core` 就摆在盘心，盘心一偏，中心球就贴到盘边上去了。
    #[test]
    fn wheel_disc_is_clamped_inside_the_monitor_and_centred_in_the_window() {
        let disc = to_physical(WHEEL_LOGICAL, 1.0);
        let half = disc / 2;

        // 屏幕中间：盘心就是球心，窗口正是盘 + 四周 16 的余量
        let mid = wheel_layout(Anchor::new(1000, 500), SCREEN, 1.0);
        assert_eq!(mid.pos, PhysicalPosition::new(884, 384)); // 1000-100-16, 500-100-16
        assert_eq!(mid.size, size(232, 232));

        // 贴右边缘（球心 1894）：盘被钳到 1720..1920，窗口跟着左移
        let right = wheel_layout(Anchor::new(1894, 500), SCREEN, 1.0);
        assert_eq!(right.pos.x, 1720 - 16);

        // 左上角：盘贴 (0,0)，窗口探出去那圈是透明的
        let corner = wheel_layout(Anchor::new(26, 26), SCREEN, 1.0);
        assert_eq!(corner.pos, PhysicalPosition::new(-16, -16));

        for anchor in [
            Anchor::new(1000, 500),  // 屏幕中间
            Anchor::new(1894, 500),  // 右边缘
            Anchor::new(26, 500),    // 左边缘
            Anchor::new(26, 26),     // 左上角
            Anchor::new(26, 1054),   // 左下角
            Anchor::new(1894, 1054), // 右下角
            Anchor::new(-30, 500),   // 屏幕外（副屏拔掉后的残影）
        ] {
            let layout = wheel_layout(anchor, SCREEN, 1.0);
            let disc_x = layout.pos.x + (layout.size.width as i32 - disc) / 2;
            let disc_y = layout.pos.y + (layout.size.height as i32 - disc) / 2;
            assert!(
                disc_x >= SCREEN.x && disc_x + disc <= SCREEN.right(),
                "盘横向出界：anchor={anchor:?} disc_x={disc_x}"
            );
            assert!(
                disc_y >= SCREEN.y && disc_y + disc <= SCREEN.bottom(),
                "盘纵向出界：anchor={anchor:?} disc_y={disc_y}"
            );
            // 盘心 = 窗口中心：CSS 的居中布局天然把中心盘放在盘心
            assert_eq!(layout.pos.x + layout.size.width as i32 / 2, disc_x + half);
            assert_eq!(layout.pos.y + layout.size.height as i32 / 2, disc_y + half);
            // 窗口必须严格大于盘（那圈 pad 是投影的地方，不许被优化成 0）
            assert!(layout.size.width as i32 > disc);
            assert!(layout.size.height as i32 > disc);
        }
    }

    /// 轮盘在超小显示器上也不能算出负数宽度
    #[test]
    fn wheel_survives_a_tiny_monitor() {
        let tiny = MonitorRect::new(0, 0, 150, 150);
        let layout = wheel_layout(Anchor::new(75, 75), tiny, 1.0);
        assert!(layout.size.width >= 232);
        // 盘比屏幕还大 → 贴左上角，别算出负的盘位置
        assert_eq!(layout.pos, PhysicalPosition::new(-16, -16));
    }

    #[test]
    fn clamp_into_keeps_a_rect_inside_the_monitor() {
        assert_eq!(clamp_into(-60, 0, 1920, 136), 0);
        assert_eq!(clamp_into(1900, 0, 1920, 136), 1784);
        // 显示器比窗口还小：贴左上角，别算出负数
        assert_eq!(clamp_into(50, 0, 100, 136), 0);
    }

    /// 拔掉副屏之后 anchor 可能落在所有显示器之外
    #[test]
    fn nearest_monitor_rescues_an_orphan_anchor() {
        let monitors = [SCREEN, MonitorRect::new(1920, 0, 1280, 1024)];
        assert_eq!(
            monitor_at(Anchor::new(2000, 100), &monitors).map(|m| m.x),
            Some(1920)
        );
        assert_eq!(monitor_at(Anchor::new(2000, 100), &[SCREEN]), None);
        // 副屏拔掉后 anchor 还在 (2000, 100) → 最近的是主屏右边缘
        assert_eq!(
            nearest_monitor(Anchor::new(2000, 100), &[SCREEN]).map(|m| m.x),
            Some(0)
        );
        assert_eq!(nearest_monitor(Anchor::new(0, 0), &[]), None);
    }

    #[test]
    fn odd_sizes_round_to_the_nearest_pixel() {
        // 窗口宽是奇数时中心落在半像素上：取整即可，别来回抖
        let pos = window_pos(Anchor::new(100, 100), size(135, 65));
        assert_eq!(pos, PhysicalPosition::new(33, 68));
    }
}
