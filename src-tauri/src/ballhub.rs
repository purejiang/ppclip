//! 悬浮入口：**一扇窗、三种形态、一个锚点**。
//!
//! | 形态 | 内容 | 窗口 |
//! |---|---|---|
//! | 贴边胶囊 16×64 | 贴屏幕左 / 右边，拖到边自动吸附 | `ball`（收起尺寸 96×96，恒定） |
//! | 常规球 52×52 | 桌面任意位置，停 2 秒展开轮盘 | 同上（尺寸不变，只换 CSS 形状） |
//! | 展开轮盘 200×200 | HUD 盘 + 4 颗卫星 + 中心盘 | 同上（232×232 = 盘 + 四周各 16 余量） |
//!
//! ## 为什么只有一扇窗
//!
//! 试过把轮盘拆成第二扇窗（"摆好位置再 show，Windows 就没有错位帧了"），结果是 WebView2
//! 的死结：`visible: false` 声明的窗口，前端脚本根本不跑（轮盘永远空白）；改成先摆到
//! 屏幕外再 show，WebView2 判定不可见、渲染直接挂住；在 `setup` 里用
//! `WebviewWindowBuilder` 现建则死锁整个应用（wry#583）。**别再试第二扇窗。**
//!
//! ## 为什么现在不跳
//!
//! 以前三种形态共用一扇窗，靠 `set_size` 在 16×64 / 52×52 / 200×200 之间来回切。
//! Windows 上「改尺寸」和「改位置」是两次独立的 OS 调用，中间必然有一帧按旧窗口中心
//! 绘制；再加上贴边时轮盘要被钳进屏幕（球在窗口中心，于是跟着跳 92px），以及前端
//! `--hub-shift` 反向补偿晚一帧才生效 —— 观感就是「开合一次跳两下」。
//!
//! 现在三件事分头兜住：
//! 1. **收起态**：两种形态共用同一个窗口尺寸，换形态只改**位置**（单次 `set_position`，
//!    原子操作）→ 没有尺寸帧；
//! 2. **位置只由 anchor 推导**（纯函数）→ 不读当前窗口几何，来回多少次都不漂；
//! 3. **展开态**：换尺寸 / 换位置之间必然有一帧错位，但前端在调用前后把内容藏掉
//!    （`BallApp` 的 `switching` → `visibility: hidden`），那一帧画的是空白，看不见。
//!
//! ## 三条不变量
//!
//! 1. **anchor 是几何的唯一真相**（形状中心，物理像素）。读一次（拖动结束 / 启动）→
//!    钳制 → 算窗口矩形 → 写回。不再「读窗口 → 算 → 写窗口」，因此不累积取整误差。
//! 2. **收起态：形状中心 = 窗口中心**。所以只要 `window_pos(anchor, 实测尺寸)`，
//!    撑开 / 收起 / tab↔orb / 吸附都不会让鼠标底下的东西跳走。
//! 3. **形态由 Rust 单向决定**，前端只吃 `ball:form` 快照 + 事件（带单调递增的 `rev`）。
//!    以前前端靠「客户区高度 >= 90/150」反推形态，读到一帧旧尺寸就挂错 CSS 类。
//!
//! 展开态是唯一偏离第 2 条的地方：盘被钳进屏幕之后盘心不再等于 anchor（最多 74px），
//! 而中心盘就摆在盘心。**不要为了「把球钉在原地」再加偏移补偿** —— 那会把中心球推到
//! 盘边缘，看起来像球飞了（真机反馈过）。
//!
//! ## 吸附 + 悬停唤出 + 自动贴回（三条路径，别混）
//!
//! 1. **落点吸附**：只在拖动结束（[`finish_drag`]）判定，判据是当前形态的形状边距
//!    屏幕边 ≤ [`geom::DOCK_EDGE_MARGIN_LOGICAL`]。判成胶囊后走 [`apply_dock`]，
//!    窗口用 **120ms ease-out** 滑到屏幕边（`start_dock_animation`）——以前是单次
//!    `set_position` 瞬移，观感「太突然」。
//! 2. **悬停唤出**：贴边胶囊上悬停 → [`set_form`]`(false)`，立刻换位置，**不动画**；
//!    位置由 [`geom::clamp_orb_inset`] 决定：球整块在屏内**且离屏幕边留
//!    [`geom::UNDOCK_EDGE_INSET_LOGICAL`]（6px）**，否则右上角标 / hover 描边 /
//!    投影会被屏幕边切掉（用户反馈「球太靠右，右侧被挡住」）。
//! 3. **自动贴回**：悬停唤出之后，鼠标离开球（真实 `GetCursorPos`，≥10px 连续 3 拍）
//!    且球仍在贴边带里 → [`arm_redock`] 看门狗调 [`apply_dock`] 滑回原侧胶囊。
//!    **不是位置上报驱动**（否则刚唤出就会被吸回去）；只在命令层 `set_ball_docked(false)`
//!    之后武装，拖动 / 隐藏会让它作废，轮盘开着时暂停。
//!
//! 动画与两条看门狗都用**单调递增的世代号**（`anim_gen` / `redock_gen`）取消，
//! 不跨线程持锁：动画线程里的 `set_position` 在 Windows 上要等窗口线程，
//! 主线程拿着锁等它就是死锁。
//!
//! 几何本身（纯算术 + 单测）在 [`geom`]，这里只负责「什么时候调哪个函数」。

pub mod geom;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize};

use crate::models::AppSettings;
use crate::settings;
use crate::{DataDir, SettingsState};

use geom::{Anchor, BallForm, DockSide, MonitorRect};

pub const BALL_LABEL: &str = "ball";
/// 形态 / 轮盘开合的快照与事件（payload 都是 [`BallSnapshot`]）
pub const FORM_EVENT: &str = "ball:form";
/// 吸附时的滑入动画时长（毫秒）。用户反馈「松手瞬间跳过去」太生硬，这里给一段
/// ease-out 的位移过渡；真机嫌快/慢只改这一个常量。
pub const DOCK_ANIM_MS: u64 = 120;
/// 滑入动画的分步间隔：120 / 8 = 15 步（每步一次 `set_position`）
const DOCK_ANIM_STEP_MS: u64 = 8;
/// 自动贴回看门狗的轮询间隔
const REDOCK_TICK_MS: u64 = 100;
/// 光标连续这么多拍都在球外，才认为「真的离开了」（去抖，~300ms）
const REDOCK_LEAVE_TICKS: u32 = 3;
/// 光标离球形状超过这么多逻辑像素才算离开。
///
/// **必须大于 `(TAB_H_LOGICAL - ORB_LOGICAL)/2` = 6**：贴回胶囊后胶囊比球高出来
/// 6px，阈值小了光标会正好落在胶囊上，前端悬停又把胶囊唤回常规态 → 来回闪。
/// `ballhub` 的常量不变量测试守住这条。
const REDOCK_LEAVE_GAP_LOGICAL: f64 = 10.0;
/// 看门狗最长挂多久（鼠标一直不离开球就别常驻轮询了）
const REDOCK_GIVE_UP_MS: u64 = 5 * 60 * 1000;

struct Inner {
    /// 单调递增：前端只接受 `rev >= lastRev` 的快照 / 事件。
    /// 没有它，「先 listen 再拉快照」仍有竞态（快照回来得比事件晚，会把新状态盖成旧的）。
    rev: u64,
    form: BallForm,
    side: DockSide,
    anchor: Anchor,
    /// 收起态 ball 窗口**实测**的物理尺寸（宽度被 `SM_CXMINTRACK` 顶着，不能写死）
    win: Option<PhysicalSize<u32>>,
    /// 轮盘是不是展开着
    wheel_open: bool,
}

impl Default for Inner {
    fn default() -> Self {
        Self {
            rev: 0,
            form: BallForm::Orb,
            side: DockSide::Left,
            anchor: Anchor::new(0, 0),
            win: None,
            wheel_open: false,
        }
    }
}

#[derive(Default)]
pub struct BallHubState {
    inner: Mutex<Inner>,
    /// 滑入动画的世代：任何新的几何操作 +1，动画线程发现自己过期就直接退出。
    ///
    /// 为什么不用锁串行化：动画线程会调 `set_position`（Windows 上跨线程
    /// `SetWindowPos` 要等窗口线程），而同步命令跑在主线程 / WebView2 回调里，
    /// 主线程拿着锁等它就死锁。世代号只做「过期就别再动窗口」。
    anim_gen: AtomicU64,
    /// 是否有滑入动画在跑：`finish_drag` 会被前端 Promise 和看门狗各调一次，
    /// 第二次不能拿滑到一半的窗口位置重新判定落点。
    anim_running: AtomicBool,
    /// 自动贴回看门狗的世代：开始拖动 / 隐藏 / 已经贴回 都让它过期
    redock_gen: AtomicU64,
}

/// 给前端的完整状态快照（事件与快照命令用同一份结构）
#[derive(Clone, Copy, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BallSnapshot {
    pub rev: u64,
    pub form: BallForm,
    pub wheel_open: bool,
    pub anchor_x: i32,
    pub anchor_y: i32,
}

impl Default for BallSnapshot {
    fn default() -> Self {
        Self {
            rev: 0,
            form: BallForm::Orb,
            wheel_open: false,
            anchor_x: 0,
            anchor_y: 0,
        }
    }
}

// ------------------------------------------------------------------ 小工具

fn with_inner<R>(app: &AppHandle, f: impl FnOnce(&mut Inner) -> R) -> Option<R> {
    app.try_state::<BallHubState>().map(|state| {
        let mut guard = state.inner.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut guard)
    })
}

/// 让正在跑的滑入动画作废（之后它一步窗口都不许再动）。
fn cancel_anim(app: &AppHandle) {
    if let Some(state) = app.try_state::<BallHubState>() {
        state.anim_gen.fetch_add(1, Ordering::SeqCst);
        state.anim_running.store(false, Ordering::SeqCst);
    }
}

fn anim_is_running(app: &AppHandle) -> bool {
    app.try_state::<BallHubState>()
        .map(|state| state.anim_running.load(Ordering::SeqCst))
        .unwrap_or(false)
}

/// 让自动贴回看门狗作废（开始拖动 / 隐藏 / 已经贴回时调用）。
fn cancel_redock(app: &AppHandle) {
    if let Some(state) = app.try_state::<BallHubState>() {
        state.redock_gen.fetch_add(1, Ordering::SeqCst);
    }
}

fn mon_rects(app: &AppHandle) -> Vec<MonitorRect> {
    app.available_monitors()
        .map(|list| {
            list.iter()
                .map(|monitor| {
                    let origin = monitor.position();
                    let area = monitor.size();
                    MonitorRect::new(origin.x, origin.y, area.width as i32, area.height as i32)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// anchor 所在的显示器；跨屏 / 拔掉副屏时退回最近的那块
fn monitor_for(app: &AppHandle, anchor: Anchor) -> Option<MonitorRect> {
    let list = mon_rects(app);
    geom::monitor_at(anchor, &list).or_else(|| geom::nearest_monitor(anchor, &list))
}

/// 钳制 + 选边（几何的唯一「归一化」入口，顺序变了就等于换了行为）。
///
/// - `Tab`：贴住屏幕边（`clamp_anchor`）；
/// - `Orb`：球整块在屏内，**而且离屏幕边留 `UNDOCK_EDGE_INSET_LOGICAL` 的余量**
///   （`clamp_orb_inset`）—— 贴边胶囊悬停出来的球如果压着屏幕边，角标 / hover
///   描边 / 投影就被切掉了。拖动落点不走吸附判定之外的夹取，所以这里的余量不会
///   改变「拖到边上就吸」的行为（压到边的球本来就会先被判成 Tab）。
fn normalize_shape(
    form: BallForm,
    anchor: Anchor,
    mon: Option<MonitorRect>,
    scale: f64,
) -> (DockSide, Anchor) {
    match mon {
        Some(mon) => {
            let side = geom::dock_side(anchor, mon);
            let anchor = match form {
                BallForm::Tab => geom::clamp_anchor(form, side, anchor, mon, scale),
                BallForm::Orb => {
                    geom::clamp_orb_inset(anchor, mon, scale, geom::UNDOCK_EDGE_INSET_LOGICAL)
                }
            };
            (side, anchor)
        }
        // 一块屏都拿不到（极端环境）：保持原样，别把球算到屏幕外
        None => (DockSide::Left, anchor),
    }
}

pub fn snapshot(app: &AppHandle) -> BallSnapshot {
    with_inner(app, |inner| BallSnapshot {
        rev: inner.rev,
        form: inner.form,
        wheel_open: inner.wheel_open,
        anchor_x: inner.anchor.x,
        anchor_y: inner.anchor.y,
    })
    .unwrap_or_default()
}

fn emit_form(app: &AppHandle) {
    let _ = app.emit_to(BALL_LABEL, FORM_EVENT, snapshot(app));
}

pub fn is_wheel_open(app: &AppHandle) -> bool {
    with_inner(app, |inner| inner.wheel_open).unwrap_or(false)
}

pub fn is_docked(app: &AppHandle) -> bool {
    with_inner(app, |inner| inner.form == BallForm::Tab).unwrap_or(false)
}

#[repr(C)]
struct RawPoint {
    x: i32,
    y: i32,
}

#[cfg(windows)]
unsafe extern "system" {
    fn GetCursorPos(lp_point: *mut RawPoint) -> i32;
    fn GetAsyncKeyState(vkey: i32) -> i16;
}

/// 左键此刻是不是**按着**（全局物理状态，与窗口焦点无关）。
///
/// 为什么需要它：`startDragging()` 进的是 Windows 的**模态移动循环**，这段时间里主线程
/// 被占着，JS 那边的 Promise / invoke 什么时候落地完全说不准 —— 实测会出现「拖到边上
/// 松手，球停在原地不贴边，日志里连一条『拖动结束』都没有」。左键状态由系统维护，
/// 随时可查，用它判定「拖动结束」比等 Promise 可靠得多。
fn left_button_down() -> bool {
    #[cfg(windows)]
    {
        const VK_LBUTTON: i32 = 0x01;
        // SAFETY: 纯查询，无指针参数
        unsafe { (GetAsyncKeyState(VK_LBUTTON) as u16) & 0x8000 != 0 }
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// 光标在虚拟屏幕上的物理坐标（多显示器时为全局坐标，与 `outer_position` 同一坐标系）
fn cursor_screen_pos() -> Option<(i32, i32)> {
    #[cfg(windows)]
    {
        let mut pt = RawPoint { x: 0, y: 0 };
        // SAFETY: GetCursorPos 只往我们提供的结构体里写，指针有效
        if unsafe { GetCursorPos(&mut pt) } == 0 {
            return None;
        }
        Some((pt.x, pt.y))
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// 光标此刻在不在**悬浮入口那扇窗**里。
///
/// 用来分辨「用户真的把鼠标移开了」和「窗口自己动了一下造成的假 leave」：
/// 窗口改尺寸 / 位置时 Chromium 会派一次 `pointerleave`，但因为指针没动，
/// **不会再补 `pointerover`** —— 光看事件永远分不清（实测后果：轮盘一展开就被收回）。
/// 判定按**窗口矩形**：收起态那圈透明区很窄（16），而展开态整窗就是轮盘，没必要再细分。
pub fn cursor_inside_hub(app: &AppHandle) -> bool {
    let Some((cx, cy)) = cursor_screen_pos() else {
        // 拿不到光标位置就保持现状（宁可多撑一会儿，也别把轮盘闪掉）
        return true;
    };
    let Some(win) = app.get_webview_window(BALL_LABEL) else {
        return false;
    };
    if !win.is_visible().unwrap_or(false) {
        return false;
    }
    let (Ok(pos), Ok(size)) = (win.outer_position(), win.outer_size()) else {
        return false;
    };
    // 留 2px 容差：贴边时形状边沿正好压在屏幕边上，取整误差会让「明明还在球上」判成外面
    const SLOP: i32 = 2;
    cx >= pos.x - SLOP
        && cx <= pos.x + size.width as i32 + SLOP
        && cy >= pos.y - SLOP
        && cy <= pos.y + size.height as i32 + SLOP
}

// ------------------------------------------------------------------ 生命周期

/// 启动时恢复：尺寸摆正 → 还原 anchor → 按 `ball_docked` 摆形态并落位。
///
/// 这一步跑在窗口还藏着的时候（`visible: false`），所以任何 `set_size` / `set_position`
/// 都看不见 —— 这就是「首帧不会先闪一下 orb 再变 tab」的原因。
pub fn init(app: &AppHandle, settings: &AppSettings) -> Result<(), String> {
    let Some(win) = app.get_webview_window(BALL_LABEL) else {
        return Ok(());
    };
    let scale = win
        .scale_factor()
        .map_err(|e| format!("读取悬浮球缩放失败: {e}"))?;

    // 1) 尺寸：只放宽不缩窄（Windows 的 SM_CXMINTRACK 可能已经把它撑得比理想值宽）
    let measured = win
        .outer_size()
        .map_err(|e| format!("读取悬浮球尺寸失败: {e}"))?;
    let ideal = geom::ideal_window_size(scale);
    let target = PhysicalSize::new(
        measured.width.max(ideal.width),
        measured.height.max(ideal.height),
    );
    if target != measured {
        log::info!(
            "悬浮球窗口尺寸摆正：{}×{} → {}×{}（逻辑尺寸 {}×{}）",
            measured.width,
            measured.height,
            target.width,
            target.height,
            target.width as f64 / scale,
            target.height as f64 / scale
        );
        win.set_size(target)
            .map_err(|e| format!("调整悬浮球尺寸失败: {e}"))?;
    }
    // 尺寸以**实测**为准：OS 可能又插一手
    let size = win
        .outer_size()
        .map_err(|e| format!("读取悬浮球尺寸失败: {e}"))?;

    // 2) anchor：新字段优先；只有旧配置时才把「窗口左上角」摆上去再读回来
    let anchor = match (settings.ball_anchor_x, settings.ball_anchor_y) {
        (Some(x), Some(y)) => Anchor::new(x, y),
        _ => {
            if let (Some(x), Some(y)) = (settings.ball_x, settings.ball_y) {
                let _ = win.set_position(PhysicalPosition::new(x, y));
            }
            geom::anchor_of(
                win.outer_position()
                    .map_err(|e| format!("读取悬浮球位置失败: {e}"))?,
                size,
            )
        }
    };

    // 3) 形态 + 钳制 + 落位（Orb 也要离屏幕边留 6px：老版本可能存了压边的 anchor）
    let form = BallForm::from_docked(settings.ball_docked);
    let (side, anchor) = normalize_shape(form, anchor, monitor_for(app, anchor), scale);
    win.set_position(geom::window_pos(anchor, size))
        .map_err(|e| format!("调整悬浮球位置失败: {e}"))?;

    with_inner(app, |inner| {
        inner.rev += 1;
        inner.form = form;
        inner.side = side;
        inner.anchor = anchor;
        inner.win = Some(size);
        inner.wheel_open = false;
    });
    emit_form(app);
    log::info!(
        "悬浮球恢复：{}，形状 {}×{}，窗口 {}×{} @({}, {})，锚点 ({}, {})",
        form.name(),
        geom::shape_size(form, scale).0,
        geom::shape_size(form, scale).1,
        size.width,
        size.height,
        anchor.x - size.width as i32 / 2,
        anchor.y - size.height as i32 / 2,
        anchor.x,
        anchor.y
    );
    persist(app)
}

/// 切「贴边胶囊 ↔ 常规球」（胶囊上悬停 / 单击都走这里）。
///
/// - **胶囊 → 常规球**（悬停唤出）：立刻换位置，**不做动画** —— 球要出现在光标底下。
///   位置由 [`geom::clamp_orb_inset`] 决定（离屏幕边留 `UNDOCK_EDGE_INSET_LOGICAL`，
///   右上角标 / hover 描边 / 投影不被屏幕边切掉）。
/// - **常规球 → 胶囊**（自动贴回 / 显式贴边）：走 [`apply_dock`]，带 120ms 滑入动画。
///
/// 只改**位置**、不改尺寸 —— 收起态窗口尺寸恒定，所以换形态没有「先 resize 再 reposition」
/// 那种半帧错位。展开态下不切（那时窗口是按轮盘摆的）。
pub fn set_form(app: &AppHandle, docked: bool) -> Result<BallSnapshot, String> {
    let Some(win) = app.get_webview_window(BALL_LABEL) else {
        return Ok(snapshot(app));
    };
    if is_wheel_open(app) {
        log::debug!("轮盘开着，忽略形态切换（docked={docked}）");
        return Ok(snapshot(app));
    }
    if docked {
        // 贴回胶囊：先提交形态、再把窗口滑过去（也能处理「已经是胶囊、只是挪了位置」）
        return apply_dock(app, &win);
    }
    cancel_anim(app);

    let scale = win
        .scale_factor()
        .map_err(|e| format!("读取悬浮球缩放失败: {e}"))?;
    let size = win
        .outer_size()
        .map_err(|e| format!("读取悬浮球尺寸失败: {e}"))?;
    let pos = win
        .outer_position()
        .map_err(|e| format!("读取悬浮球位置失败: {e}"))?;

    let form = BallForm::Orb;
    let anchor = geom::anchor_of(pos, size);
    let (side, anchor) = normalize_shape(form, anchor, monitor_for(app, anchor), scale);
    let target = geom::window_pos(anchor, size);
    if target != pos {
        win.set_position(target)
            .map_err(|e| format!("调整悬浮球位置失败: {e}"))?;
    }

    with_inner(app, |inner| {
        inner.rev += 1;
        inner.form = form;
        inner.side = side;
        inner.anchor = anchor;
        inner.win = Some(size);
    });
    log::info!(
        "悬浮球形态 → orb：窗口 {}×{} @({}, {}) → @({}, {})，锚点 ({}, {})（离屏幕边留 {} 逻辑像素）",
        size.width,
        size.height,
        pos.x,
        pos.y,
        target.x,
        target.y,
        anchor.x,
        anchor.y,
        geom::UNDOCK_EDGE_INSET_LOGICAL
    );
    emit_form(app);
    Ok(snapshot(app))
}

/// 把悬浮球贴回屏幕边（带 120ms 滑入动画）。`finish_drag`、`set_form(true)`、
/// 自动贴回看门狗共用这一条路径。
///
/// **先提交形态、再滑窗口**：屏幕上滑动的是 16×64 的胶囊，而不是让 52×52 的球滑到
/// 一半挂在屏幕外。窗口位置只由 anchor 推导（`window_pos`），动画只负责把窗口从
/// 「松手 / 原位置」插值到目标，**anchor 本身不参与动画**。
///
/// 命令立即返回，动画在独立线程里跑 —— 前端拿到的快照已经是最终形态。
fn apply_dock(app: &AppHandle, win: &tauri::WebviewWindow) -> Result<BallSnapshot, String> {
    cancel_anim(app);
    cancel_redock(app);

    let scale = win
        .scale_factor()
        .map_err(|e| format!("读取悬浮球缩放失败: {e}"))?;
    let size = win
        .outer_size()
        .map_err(|e| format!("读取悬浮球尺寸失败: {e}"))?;
    let pos = win
        .outer_position()
        .map_err(|e| format!("读取悬浮球位置失败: {e}"))?;
    let anchor = geom::anchor_of(pos, size);

    let form = BallForm::Tab;
    let (side, anchor) = normalize_shape(form, anchor, monitor_for(app, anchor), scale);
    let target = geom::window_pos(anchor, size);

    with_inner(app, |inner| {
        inner.rev += 1;
        inner.form = form;
        inner.side = side;
        inner.anchor = anchor;
        inner.win = Some(size);
    });
    log::info!(
        "贴回贴边态 → tab：形状中心 ({}, {})，窗口 @({}, {}) → @({}, {}){}",
        anchor.x,
        anchor.y,
        pos.x,
        pos.y,
        target.x,
        target.y,
        if target != pos {
            "（120ms 滑入）"
        } else {
            ""
        }
    );
    emit_form(app);

    if target != pos {
        start_dock_animation(app, pos, target);
    }
    Ok(snapshot(app))
}

/// ease-out 三次曲线滑入：只动窗口位置，不碰状态。
///
/// 每一步都先检查 `anim_gen` 还是不是自己这一代 —— 一旦被新的几何操作
/// （切形态 / 开合轮盘 / 开始拖动 / 隐藏 / DPI 变化）作废，立刻收手，
/// 免得和主线程或原生拖动循环抢窗口。**不要改成持锁串行化**：动画线程里的
/// `set_position` 在 Windows 上要等窗口线程，主线程拿着锁等它就是死锁。
fn start_dock_animation(app: &AppHandle, from: PhysicalPosition<i32>, to: PhysicalPosition<i32>) {
    let app_for_thread = app.clone();
    let gen = match app.try_state::<BallHubState>() {
        Some(state) => {
            let gen = state.anim_gen.fetch_add(1, Ordering::SeqCst) + 1;
            state.anim_running.store(true, Ordering::SeqCst);
            gen
        }
        None => return,
    };
    std::thread::spawn(move || {
        let Some(win) = app_for_thread.get_webview_window(BALL_LABEL) else {
            return;
        };
        let steps = (DOCK_ANIM_MS / DOCK_ANIM_STEP_MS).max(1);
        for step in 1..=steps {
            std::thread::sleep(Duration::from_millis(DOCK_ANIM_STEP_MS));
            let Some(state) = app_for_thread.try_state::<BallHubState>() else {
                return;
            };
            if state.anim_gen.load(Ordering::SeqCst) != gen {
                return;
            }
            drop(state);
            let t = step as f64 / steps as f64;
            let eased = 1.0 - (1.0 - t).powi(3);
            let position = PhysicalPosition::new(
                from.x + ((to.x - from.x) as f64 * eased).round() as i32,
                from.y + ((to.y - from.y) as f64 * eased).round() as i32,
            );
            if let Err(err) = win.set_position(position) {
                log::warn!("贴边动画：移动悬浮球失败: {err}");
                break;
            }
        }
        let Some(state) = app_for_thread.try_state::<BallHubState>() else {
            return;
        };
        if state.anim_gen.load(Ordering::SeqCst) == gen {
            let _ = win.set_position(to);
            state.anim_running.store(false, Ordering::SeqCst);
            if let Err(err) = persist(&app_for_thread) {
                log::warn!("保存悬浮球位置失败: {err}");
            }
        }
    });
}

/// **拖动结束**：按落点决定形态（靠边就吸成胶囊）。**全项目唯一判定「落点吸附」的地方。**
///
/// 为什么只在这里判落点：常规球被钳进可视区之后左边缘同样贴着屏幕，位置一上报就会被
/// 误判成「想贴边」，把刚唤出来的常规态吸回胶囊（踩过）。悬停唤出之后的「自动贴回」
/// 是另一条路径（[`arm_redock`]），由**真实光标离开球**触发，不是位置上报。
///
/// 幂等：判完之后几何只会更「规范」（球贴边 → 胶囊贴边 → 再判还是贴边），所以前端和
/// [`begin_drag`] 的看门狗都调它不会打架。**左键还按着时直接跳过** —— 那说明拖动还在进行，
/// 这时候把窗口拽回边缘会被移动循环覆盖掉，松手位置也不再被重新判定。
///
/// 贴边走 [`apply_dock`]（120ms 滑入）；不贴边就是普通常规球，不动画、也不武装自动贴回
/// —— 用户是主动把它放在那儿的。
pub fn finish_drag(app: &AppHandle) -> Result<BallSnapshot, String> {
    let Some(win) = app.get_webview_window(BALL_LABEL) else {
        return Ok(snapshot(app));
    };
    if is_wheel_open(app) {
        return Ok(snapshot(app));
    }
    if left_button_down() {
        log::debug!("左键仍按着，跳过吸附判定（等拖动看门狗兜底）");
        return Ok(snapshot(app));
    }
    // 前端 Promise 与看门狗各调一次：滑入动画进行中时窗口位置是半路的，
    // 不能拿它重新判定落点（否则会二次起动画、滑到一半再从头缓动）。
    if anim_is_running(app) {
        log::debug!("贴边动画进行中，跳过重复的落点判定");
        return Ok(snapshot(app));
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
    let anchor = geom::anchor_of(pos, size);
    let form_now = with_inner(app, |inner| inner.form).unwrap_or(BallForm::Orb);

    // 吸附判定用**当前形态**的形状边：球边距 ≤ 24 就吸，胶囊边距 > 24 才松手
    // （两者之间是回差带，手抖不会来回切）
    let docked = match monitor_for(app, anchor) {
        Some(mon) => geom::should_dock(
            form_now,
            anchor,
            mon,
            scale,
            geom::DOCK_EDGE_MARGIN_LOGICAL,
        ),
        None => form_now == BallForm::Tab,
    };
    if docked {
        log::info!(
            "拖动结束 → 贴边：形状中心 ({}, {})，窗口 @({}, {})",
            anchor.x,
            anchor.y,
            pos.x,
            pos.y
        );
        return apply_dock(app, &win);
    }

    cancel_anim(app);
    // 拖动 = 主动摆放：落点判定已经在这里做完，不要再被自动贴回接走
    cancel_redock(app);
    let form = BallForm::Orb;
    let (side, anchor) = normalize_shape(form, anchor, monitor_for(app, anchor), scale);
    let target = geom::window_pos(anchor, size);
    if target != pos {
        win.set_position(target)
            .map_err(|e| format!("调整悬浮球位置失败: {e}"))?;
    }

    with_inner(app, |inner| {
        inner.rev += 1;
        inner.form = form;
        inner.side = side;
        inner.anchor = anchor;
        inner.win = Some(size);
    });
    log::info!(
        "拖动结束 → 常规球：形状中心 ({}, {})，窗口 @({}, {}) → @({}, {})",
        anchor.x,
        anchor.y,
        pos.x,
        pos.y,
        target.x,
        target.y
    );
    emit_form(app);
    Ok(snapshot(app))
}

/// 拖动开始时的**看门狗**（前端在 `startDragging()` 之前调用）。
///
/// 为什么需要它：`startDragging()` 进的是 Windows 的模态移动循环，主线程被占住，
/// 前端那条 `startDragging().then(→ finish_ball_drag)` 什么时候执行完全说不准 ——
/// 实测会出现「拖到边上松手，球停在原地不贴边」，日志里连一条「拖动结束」都没有。
/// 所以吸附判定的收尾改成在这里做：**轮询真实的左键状态**，松手后再让窗口停稳一小会儿，
/// 然后跑一次 [`finish_drag`]。
///
/// 这不推翻 [`finish_drag`] 是唯一落点判定处：这个线程只是**决定什么时候**去调它。
/// 两边都调也不会错（`finish_drag` 幂等，而且左键还按着时它自己会跳过）。
///
/// 开始拖动同时让**自动贴回看门狗**和正在跑的滑入动画作废：落点交给 `finish_drag`。
pub fn begin_drag(app: &AppHandle) {
    cancel_redock(app);
    cancel_anim(app);
    let app = app.clone();
    std::thread::spawn(move || {
        // 等松手（最多 30 秒，按住不放就放弃，别在拖动中途去吸边）
        let mut released = false;
        for _ in 0..600 {
            std::thread::sleep(std::time::Duration::from_millis(50));
            if !left_button_down() {
                released = true;
                break;
            }
        }
        if !released {
            log::warn!("拖动看门狗：左键一直没松，跳过吸附判定");
            return;
        }
        // 松手后让移动循环退出、最后一帧位置落地，再判落点
        std::thread::sleep(std::time::Duration::from_millis(120));
        match finish_drag(&app) {
            Ok(snap) => log::info!(
                "拖动看门狗：松手后判定为 {}，锚点 ({}, {})",
                snap.form.name(),
                snap.anchor_x,
                snap.anchor_y
            ),
            Err(err) => log::warn!("拖动看门狗：吸附判定失败: {err}"),
        }
    });
}

/// **自动贴回**：悬停唤出的常规球，鼠标离开球之后自己滑回原来那一侧的胶囊。
///
/// 只在命令层 `set_ball_docked(false)`（真的悬停唤出）之后武装，**不挂在位置上报上**：
/// 常规球被钳进可视区后边缘照样贴着屏幕，按位置判会「刚唤出就吸回去」（踩过）。
///
/// 几条硬要求：
/// - 用真实光标位置（`GetCursorPos`）轮询，不信前端 pointer 事件 —— 窗口自己动会派
///   假 `pointerleave`，而且不会补 `pointerover`；
/// - 只在球仍落在贴边带里时生效：悬停唤出的球一定有 6px 余量（< 24 的带宽），
///   用户拖到屏幕中间的球（`begin_drag` 已经取消本看门狗）不会被吸回来；
/// - **轮盘开着时暂停计数**（不清世代）：收起轮盘后鼠标若已经离开，仍会贴回；
/// - 离开阈值 10 逻辑像素 > 胶囊比球高出来的 6px，贴回后胶囊不会正好落在光标下
///   被前端再次唤出（来回闪）。
pub fn arm_redock(app: &AppHandle) {
    let Some(win) = app.get_webview_window(BALL_LABEL) else {
        return;
    };
    let Some((form, anchor)) = with_inner(app, |inner| (inner.form, inner.anchor)) else {
        return;
    };
    if form != BallForm::Orb || !win.is_visible().unwrap_or(false) {
        return;
    }
    let Ok(scale) = win.scale_factor() else {
        return;
    };
    let Some(mon) = monitor_for(app, anchor) else {
        return;
    };
    if !geom::should_dock(
        BallForm::Orb,
        anchor,
        mon,
        scale,
        geom::DOCK_EDGE_MARGIN_LOGICAL,
    ) {
        // 球不在贴边带里（比如用户拖到中间松手）：这不是「悬停唤出的球」，不贴
        return;
    }
    let gen = match app.try_state::<BallHubState>() {
        Some(state) => state.redock_gen.fetch_add(1, Ordering::SeqCst) + 1,
        None => return,
    };

    let app = app.clone();
    std::thread::spawn(move || {
        let started = std::time::Instant::now();
        let mut off_ticks = 0u32;
        loop {
            std::thread::sleep(Duration::from_millis(REDOCK_TICK_MS));
            match app.try_state::<BallHubState>() {
                Some(state) if state.redock_gen.load(Ordering::SeqCst) == gen => {}
                // 世代变了（拖动 / 隐藏 / 已贴回 / 重新武装）：立刻收手
                _ => return,
            }
            if started.elapsed() >= Duration::from_millis(REDOCK_GIVE_UP_MS) {
                log::info!("自动贴回：等了 5 分钟鼠标还没离开，放弃");
                return;
            }
            let Some(win) = app.get_webview_window(BALL_LABEL) else {
                return;
            };
            if !win.is_visible().unwrap_or(false) {
                return;
            }
            let Some((form, anchor, wheel_open)) =
                with_inner(&app, |inner| (inner.form, inner.anchor, inner.wheel_open))
            else {
                return;
            };
            if form != BallForm::Orb {
                return;
            }
            if wheel_open {
                // 轮盘开着：不判离开也不贴回，收起后再接着看
                off_ticks = 0;
                continue;
            }
            let Ok(scale) = win.scale_factor() else {
                return;
            };
            let Some(cursor) = cursor_screen_pos() else {
                off_ticks = 0;
                continue;
            };
            if geom::cursor_off_shape(
                cursor,
                anchor,
                BallForm::Orb,
                scale,
                REDOCK_LEAVE_GAP_LOGICAL,
            ) {
                off_ticks += 1;
            } else {
                off_ticks = 0;
            }
            if off_ticks < REDOCK_LEAVE_TICKS {
                continue;
            }
            // 兜底：这几拍之间球可能被挪出贴边带（拖动会直接取消本看门狗，这里只是保险）
            let Some(mon) = monitor_for(&app, anchor) else {
                return;
            };
            if !geom::should_dock(
                BallForm::Orb,
                anchor,
                mon,
                scale,
                geom::DOCK_EDGE_MARGIN_LOGICAL,
            ) {
                return;
            }
            log::info!(
                "自动贴回：鼠标离开悬浮球，形态 → tab，锚点 ({}, {})",
                anchor.x,
                anchor.y
            );
            if let Err(err) = set_form(&app, true) {
                log::warn!("自动贴回失败: {err}");
            }
            if let Err(err) = persist(&app) {
                log::warn!("保存悬浮球状态失败: {err}");
            }
            return;
        }
    });
}

// ------------------------------------------------------------------ 轮盘

/// 展开 / 收起 HUD 轮盘。
///
/// 轮盘和球是**同一扇窗**（`ball`）：展开时窗口从收起态的 96×96 换成 232×232
/// （200 的盘 + 四周各 16 的余量，给投影留地方）。
///
/// 为什么这次不会「跳」：
/// 1. 窗口位置**只由 anchor 推导**（纯函数），不读当前窗口几何 → 不累积漂移；
/// 2. 换尺寸与换位置是两次 OS 调用，中间必有一帧错位 —— 但**前端在调用前后把内容
///    藏起来**（`BallApp` 的 `switching`），那一帧画的是空白，看不见；
/// 3. 盘为整块进屏幕会被钳向内（最多 74px），此时盘心离开 anchor —— 中心盘就是盘心，
///    不去补偿它（补偿会让中心球贴到盘边上）。
///
/// 所以这里只做两件事：算矩形、摆窗口。**不要求在展开态下读窗口尺寸/位置** ——
/// 收起时用的永远是 `anchor` + 记住的收起态尺寸，来回多少次都不会漂。
pub fn set_wheel(app: &AppHandle, open: bool) -> Result<BallSnapshot, String> {
    let Some(win) = app.get_webview_window(BALL_LABEL) else {
        return Ok(snapshot(app));
    };
    // 轮盘要重摆窗口：正在跑的贴边滑入动画立刻作废（它只该在收起态发生）
    cancel_anim(app);
    let (anchor, collapsed) = with_inner(app, |inner| (inner.anchor, inner.win))
        .ok_or("悬浮球状态未注册")?;
    if is_wheel_open(app) == open {
        return Ok(snapshot(app));
    }
    let scale = win
        .scale_factor()
        .map_err(|e| format!("读取悬浮球缩放失败: {e}"))?;
    let Some(mon) = monitor_for(app, anchor) else {
        return Ok(snapshot(app));
    };

    if open {
        // 轮盘只在常规球上展开（贴边胶囊先悬停变球，这是产品决定的语义）
        if with_inner(app, |inner| inner.form) == Some(BallForm::Tab) {
            log::info!("贴边态不展开轮盘");
            return Ok(snapshot(app));
        }
        let layout = geom::wheel_layout(anchor, mon, scale);
        win.set_size(layout.size)
            .map_err(|e| format!("调整悬浮球尺寸失败: {e}"))?;
        // 按**实测**尺寸复核一次：跨 DPI 时 set_size 之后的真实物理尺寸可能和推算值不同，
        // 盘必须继续居中、继续整块在屏幕里
        let actual = win
            .outer_size()
            .map_err(|e| format!("读取悬浮球尺寸失败: {e}"))?;
        let layout = if actual == layout.size {
            layout
        } else {
            geom::wheel_layout_in(anchor, mon, scale, actual)
        };
        win.set_position(layout.pos)
            .map_err(|e| format!("调整悬浮球位置失败: {e}"))?;
        with_inner(app, |inner| {
            inner.rev += 1;
            inner.wheel_open = true;
        });
        log::info!(
            "轮盘展开：锚点 ({}, {})，窗口 {}×{} @({}, {})，盘心 ({}, {})（偏离锚点 {:+}）",
            anchor.x,
            anchor.y,
            layout.size.width,
            layout.size.height,
            layout.pos.x,
            layout.pos.y,
            layout.pos.x + layout.size.width as i32 / 2,
            layout.pos.y + layout.size.height as i32 / 2,
            layout.pos.x + layout.size.width as i32 / 2 - anchor.x
        );
    } else {
        // 收起：尺寸/位置全部由 anchor 反推（用记住的收起态尺寸），一比一还原
        let size = collapsed.unwrap_or_else(|| geom::ideal_window_size(scale));
        win.set_size(size)
            .map_err(|e| format!("调整悬浮球尺寸失败: {e}"))?;
        win.set_position(geom::window_pos(anchor, size))
            .map_err(|e| format!("调整悬浮球位置失败: {e}"))?;
        with_inner(app, |inner| {
            inner.rev += 1;
            inner.wheel_open = false;
        });
        log::info!(
            "轮盘收起：锚点 ({}, {})，窗口 {}×{} @({}, {})",
            anchor.x,
            anchor.y,
            size.width,
            size.height,
            anchor.x - size.width as i32 / 2,
            anchor.y - size.height as i32 / 2
        );
    }
    if let Err(err) = persist(app) {
        log::warn!("保存悬浮球状态失败: {err}");
    }
    let snap = snapshot(app);
    emit_form(app);
    Ok(snap)
}

/// 立刻收起（面板 / 截图 / 隐藏悬浮球 / 第二实例唤醒都用它）
pub fn close_wheel_now(app: &AppHandle) -> Result<(), String> {
    if !is_wheel_open(app) {
        return Ok(());
    }
    set_wheel(app, false).map(|_| ())
}

// ------------------------------------------------------------------ 其它

/// 藏起悬浮入口（含轮盘）
pub fn hide(app: &AppHandle) -> Result<(), String> {
    // 藏之前把两条看门狗都停掉：否则它们可能在窗口藏起来之后还去动几何
    cancel_redock(app);
    cancel_anim(app);
    close_wheel_now(app)?;
    if let Some(ball) = app.get_webview_window(BALL_LABEL) {
        ball.hide().map_err(|e| format!("隐藏悬浮球失败: {e}"))?;
    }
    Ok(())
}

/// 显示悬浮入口
pub fn show(app: &AppHandle) -> Result<(), String> {
    if let Some(ball) = app.get_webview_window(BALL_LABEL) {
        ball.show().map_err(|e| format!("显示悬浮球失败: {e}"))?;
    }
    Ok(())
}

/// 落盘：anchor 为真相，`ball_x/y` 只做旧版本的镜像（窗口左上角）。
pub fn persist(app: &AppHandle) -> Result<(), String> {
    let Some((anchor, form)) = with_inner(app, |inner| (inner.anchor, inner.form)) else {
        return Ok(());
    };
    let ball_pos = app
        .get_webview_window(BALL_LABEL)
        .and_then(|win| win.outer_position().ok());
    let Some(dir) = app.try_state::<DataDir>() else {
        return Ok(());
    };
    let Some(state) = app.try_state::<SettingsState>() else {
        return Ok(());
    };
    let snapshot = {
        let mut guard = state.0.lock().unwrap_or_else(|e| e.into_inner());
        guard.ball_anchor_x = Some(anchor.x);
        guard.ball_anchor_y = Some(anchor.y);
        guard.ball_docked = form == BallForm::Tab;
        if let Some(pos) = ball_pos {
            guard.ball_x = Some(pos.x);
            guard.ball_y = Some(pos.y);
        }
        guard.clone()
    };
    settings::save_settings(&dir.0, &snapshot)
}

/// DPI / 跨屏变化。
///
/// 「窗口尺寸恒定」只在同一个 DPI 下成立：`WM_DPICHANGED` 会让 tao 按建议矩形改物理尺寸。
/// 所以这里必须重读实测值、反推 anchor、重新钳制摆位 —— 否则球会按旧尺寸的推算位置偏出去。
/// 轮盘是按 anchor + DPI 算出来的，就地凑容易错，直接收掉、下次重算。
pub fn on_scale_changed(app: &AppHandle, label: &str) -> Result<(), String> {
    if label != BALL_LABEL {
        return Ok(());
    }
    let Some(win) = app.get_webview_window(BALL_LABEL) else {
        return Ok(());
    };
    // DPI 变了，正在跑的滑入动画按旧尺寸算出来的落点就不对了：先作废
    cancel_anim(app);
    let scale = win
        .scale_factor()
        .map_err(|e| format!("读取悬浮球缩放失败: {e}"))?;
    let size = win
        .outer_size()
        .map_err(|e| format!("读取悬浮球尺寸失败: {e}"))?;
    let pos = win
        .outer_position()
        .map_err(|e| format!("读取悬浮球位置失败: {e}"))?;
    let form = with_inner(app, |inner| inner.form).unwrap_or(BallForm::Orb);
    let anchor = geom::anchor_of(pos, size);
    let (side, anchor) = normalize_shape(form, anchor, monitor_for(app, anchor), scale);
    let target = geom::window_pos(anchor, size);
    if target != pos {
        win.set_position(target)
            .map_err(|e| format!("调整悬浮球位置失败: {e}"))?;
    }
    with_inner(app, |inner| {
        inner.rev += 1;
        inner.side = side;
        inner.anchor = anchor;
        inner.win = Some(size);
    });
    log::info!(
        "DPI 变化（{}）：缩放 {:.2}，窗口 {}×{} @({}, {})，锚点 ({}, {})",
        label,
        scale,
        size.width,
        size.height,
        target.x,
        target.y,
        anchor.x,
        anchor.y
    );
    emit_form(app);
    persist(app)
}

#[cfg(test)]
mod tests {
    use super::geom::{ORB_LOGICAL, TAB_H_LOGICAL};
    use super::*;

    /// 自动贴回的离开阈值必须大于「胶囊比球高出来的那半截」（6px）：
    /// 否则贴回那一刻光标正好落在胶囊上，前端悬停又把胶囊唤回常规态 → 来回闪。
    #[test]
    fn redock_leave_gap_clears_the_tab_overshoot() {
        let overshoot = (TAB_H_LOGICAL - ORB_LOGICAL) / 2.0;
        assert!(
            REDOCK_LEAVE_GAP_LOGICAL > overshoot,
            "离开阈值 {REDOCK_LEAVE_GAP_LOGICAL} 必须大于胶囊比球高出来的 {overshoot}"
        );
    }

    /// 滑入动画至少要走两步，一步就退化成瞬移了
    #[test]
    fn dock_animation_has_multiple_steps() {
        assert!(DOCK_ANIM_MS / DOCK_ANIM_STEP_MS >= 2);
    }
}
