use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};

use crate::models::{Clip, ClipKind};
use crate::storage::Storage;

/// 查看窗口的 label：整个应用只开一个，内容跟着当前条目换
pub const VIEWER_LABEL: &str = "viewer";

/// 通知前端「换条目了」（前端收到后去问后端要 id，事件本身不传数据）
pub const VIEWER_SHOW_EVENT: &str = "viewer:show";

/// 查看窗口当前显示的条目 id
#[derive(Default)]
pub struct ViewerState(pub Mutex<Option<i64>>);

/// 查看窗口的尺寸约束
const VIEW_MAX_W: u32 = 1280;
const VIEW_MAX_H: u32 = 960;
const VIEW_MIN_W: u32 = 420;
const VIEW_MIN_H: u32 = 320;
/// 文本查看器的默认尺寸（图片按原始像素，受上面的上限约束）
const TEXT_VIEW_W: u32 = 760;
const TEXT_VIEW_H: u32 = 620;

/// 打开某条历史的完整内容。
///
/// 面板里只有 120 字预览与 320px 缩略图，要看全文 / 原图就靠这个窗口。
/// 只开一个窗口：已经开着就换内容 + 聚焦（面板保持可见，方便一条条翻）。
/// 注意调用方必须在「非 WebView2 回调」的线程上（建窗，见 `crate::run_off_main`）。
pub fn open_viewer(app: &AppHandle, clip_id: i64) -> Result<(), String> {
    // 先确认条目存在，免得开出个空窗口
    let clip = app.state::<Storage>().get(clip_id)?;

    if let Some(state) = app.try_state::<ViewerState>() {
        let mut guard = state.0.lock().unwrap_or_else(|e| e.into_inner());
        *guard = Some(clip_id);
    }

    if let Some(existing) = app.get_webview_window(VIEWER_LABEL) {
        let _ = existing.show();
        let _ = existing.set_focus();
        let _ = app.emit_to(VIEWER_LABEL, VIEWER_SHOW_EVENT, clip_id);
        return Ok(());
    }

    let (width, height) = view_size(&clip);
    let window = WebviewWindowBuilder::new(app, VIEWER_LABEL, WebviewUrl::App("index.html".into()))
        .title("PPClip 查看")
        .inner_size(width as f64, height as f64)
        .decorations(false)
        .resizable(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .shadow(true)
        .transparent(false)
        .visible(false)
        .build()
        .map_err(|e| format!("创建查看窗口失败: {e}"))?;

    place_beside_panel(app, &window);
    let _ = window.show();
    let _ = window.set_focus();
    log::info!("打开查看窗口（条目 {clip_id}，{width}×{height}）");
    Ok(())
}

/// 把查看窗口摆在面板旁边：右边放不下就放左边，面板不在就居中。
///
/// 两边都摆不下（面板占满屏幕宽度之类）时保持原位，反正用户能拖。
fn place_beside_panel(app: &AppHandle, window: &tauri::WebviewWindow) {
    let Some(panel) = app.get_webview_window("main") else {
        return;
    };
    if !panel.is_visible().unwrap_or(false) {
        let _ = window.center();
        return;
    }

    let (Ok(size), Ok(pos), Ok(panel_size)) = (
        window.outer_size(),
        panel.outer_position(),
        panel.outer_size(),
    ) else {
        return;
    };

    let (mx, my, mw, mh) = match panel.current_monitor().ok().flatten() {
        Some(monitor) => (
            monitor.position().x,
            monitor.position().y,
            monitor.size().width as i32,
            monitor.size().height as i32,
        ),
        None => (0, 0, 1920, 1080),
    };

    let (width, height) = (size.width as i32, size.height as i32);
    const GAP: i32 = 12;
    let right = pos.x + panel_size.width as i32 + GAP;
    let left = pos.x - GAP - width;
    let x = if right + width <= mx + mw {
        right
    } else if left >= mx {
        left
    } else {
        pos.x
    };
    let y = pos.y.clamp(my, (my + mh - height).max(my));
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

/// 按内容给个初始尺寸：图片尽量按原始像素，文本给固定卡片
fn view_size(clip: &Clip) -> (u32, u32) {
    match clip.kind {
        ClipKind::Image => {
            let width = clip.width.unwrap_or(TEXT_VIEW_W as i64).max(1) as u32;
            let height = clip.height.unwrap_or(TEXT_VIEW_H as i64).max(1) as u32;
            (
                width.clamp(VIEW_MIN_W, VIEW_MAX_W),
                height.clamp(VIEW_MIN_H, VIEW_MAX_H),
            )
        }
        ClipKind::Text => (TEXT_VIEW_W, TEXT_VIEW_H),
    }
}

/// 查看窗口当前显示的条目 id（`viewer_clip` 命令与删除逻辑都用它）
pub fn shown_clip(app: &AppHandle) -> Option<i64> {
    let state = app.try_state::<ViewerState>()?;
    let guard = state.0.lock().unwrap_or_else(|e| e.into_inner());
    *guard
}

/// 关闭查看窗口（条目被删 / 清空 / 用户按 Esc 时调用）
pub fn close_viewer(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(VIEWER_LABEL) {
        let _ = window.close();
    }
    if let Some(state) = app.try_state::<ViewerState>() {
        let mut guard = state.0.lock().unwrap_or_else(|e| e.into_inner());
        *guard = None;
    }
}
