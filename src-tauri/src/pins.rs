use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};

use crate::models::{Clip, ClipKind, Pin};
use crate::storage::Storage;

/// 贴图窗口的尺寸约束
const PIN_MAX_W: i32 = 480;
const PIN_MAX_H: i32 = 640;
const PIN_MIN_W: i32 = 60;
/// 文本贴图的卡片尺寸
const TEXT_PIN_W: i32 = 320;
const TEXT_PIN_H: i32 = 140;

pub fn pin_label(id: i64) -> String {
    format!("pin-{id}")
}

/// 按内容算初始几何：图片按原始像素（受上限约束等比缩放），文本给固定卡片
pub fn default_geometry(app: &AppHandle, clip: &Clip) -> (i32, i32, i32, i32, f64) {
    let (mut w, mut h, zoom) = match clip.kind {
        ClipKind::Image => {
            let natural_w = clip.width.unwrap_or(TEXT_PIN_W as i64).max(1) as f64;
            let natural_h = clip.height.unwrap_or(TEXT_PIN_H as i64).max(1) as f64;
            let scale = (PIN_MAX_W as f64 / natural_w)
                .min(PIN_MAX_H as f64 / natural_h)
                .min(1.0);
            (
                (natural_w * scale).round() as i32,
                (natural_h * scale).round() as i32,
                scale,
            )
        }
        ClipKind::Text => (TEXT_PIN_W, TEXT_PIN_H, 1.0),
    };
    w = w.max(PIN_MIN_W);
    h = h.max(24);

    // 落点跟着光标，往右下偏一点，别正好压住用户正在看的位置
    let (x, y) = match app.cursor_position() {
        Ok(pos) => (pos.x.round() as i32 + 24, pos.y.round() as i32 + 24),
        Err(_) => (240, 160),
    };
    (x, y, w, h, zoom)
}

/// 建贴图窗口（已存在则直接返回）
pub fn build_pin_window(app: &AppHandle, pin: &Pin) -> Result<(), String> {
    let label = pin_label(pin.id);
    if app.get_webview_window(&label).is_some() {
        return Ok(());
    }

    let window = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("index.html".into()))
        .title("PPClip 贴图")
        .inner_size(200.0, 200.0)
        .decorations(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .shadow(false)
        .transparent(true)
        .visible(false)
        .build()
        .map_err(|e| format!("创建贴图窗口失败: {e}"))?;

    // 几何一律用 physical 设置，避免缩放比换算
    let _ = window.set_position(PhysicalPosition::new(pin.x, pin.y));
    let _ = window.set_size(PhysicalSize::new(pin.w.max(1) as u32, pin.h.max(1) as u32));
    let _ = window.show();
    Ok(())
}

pub fn close_pin_window(app: &AppHandle, pin_id: i64) {
    if let Some(win) = app.get_webview_window(&pin_label(pin_id)) {
        let _ = win.close();
    }
}

/// 启动时恢复上次退出前钉在桌面上的贴图
pub fn restore_pins(app: &AppHandle) {
    let Some(storage) = app.try_state::<Storage>() else {
        return;
    };

    match storage.list_pins() {
        Ok(pins) => {
            if pins.is_empty() {
                return;
            }
            let mut restored = 0;
            for pin in &pins {
                match build_pin_window(app, pin) {
                    Ok(()) => restored += 1,
                    Err(err) => log::warn!("恢复贴图 {} 失败: {err}", pin.id),
                }
            }
            log::info!("恢复了 {restored} 个桌面贴图");
        }
        Err(err) => log::warn!("读取贴图失败: {err}"),
    }
}

/// 缩放贴图：direction > 0 放大，< 0 缩小。保持长宽比；文本贴图不缩放。
pub fn zoom_pin(app: &AppHandle, pin_id: i64, direction: i32) -> Result<Pin, String> {
    let storage = app
        .try_state::<Storage>()
        .ok_or_else(|| "存储尚未初始化".to_string())?;

    let pin = storage.get_pin(pin_id)?;
    let clip = storage.get(pin.clip_id)?;
    if clip.kind != ClipKind::Image {
        return Ok(pin);
    }

    let factor = if direction >= 0 { 1.15 } else { 1.0 / 1.15 };
    let natural_w = clip.width.unwrap_or(pin.w as i64).max(1) as f64;
    let natural_h = clip.height.unwrap_or(pin.h as i64).max(1) as f64;

    let new_w = (pin.w as f64 * factor).clamp(PIN_MIN_W as f64, natural_w * 4.0);
    let new_h = (new_w * natural_h / natural_w).max(24.0);
    let new_w = new_w.round() as i32;
    let new_h = new_h.round() as i32;

    storage.update_pin_geometry(pin.id, pin.x, pin.y, new_w, new_h, new_w as f64 / natural_w)?;
    if let Some(win) = app.get_webview_window(&pin_label(pin.id)) {
        let _ = win.set_size(PhysicalSize::new(new_w as u32, new_h as u32));
    }
    storage.get_pin(pin_id)
}
