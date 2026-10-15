use std::sync::Mutex;

use clipboard_rs::common::RustImage;
use clipboard_rs::{Clipboard, ClipboardContext};
use image::RgbaImage;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder,
};

use crate::clipboard::SelfWrite;
use crate::storage::Storage;
use crate::util;
use crate::{DataDir, SettingsState};

/// 冻结图在数据目录里的相对路径
const OVERLAY_REL: &str = "cache/overlay.png";

/// 一次截图会话：冻结的整屏图像 + 截图前被藏起来的窗口
pub struct CaptureSession {
    pub image: RgbaImage,
    /// 截图前可见、结束后需要恢复显示的窗口 label
    pub restore: Vec<String>,
}

#[derive(Default)]
pub struct CaptureState(pub Mutex<Option<CaptureSession>>);

/// 本应用常驻窗口：截图前必须藏起来，否则会被拍进画面
const PERSISTENT_WINDOWS: [&str; 2] = ["ball", "main"];

/// 开始区域截图：抓冻结图 → 藏自己的窗口 → 起 overlay
pub fn start_region_capture(app: &AppHandle) -> Result<(), String> {
    // 幂等：上一次没结束的话先收干净，避免叠加多个 overlay
    cancel_capture_impl(app);

    let monitor = locate_monitor(app)?;
    let width = monitor.width().map_err(|e| e.to_string())?;
    let height = monitor.height().map_err(|e| e.to_string())?;
    let monitor_x = monitor.x().map_err(|e| e.to_string())?;
    let monitor_y = monitor.y().map_err(|e| e.to_string())?;

    // 先抓图：此刻 overlay 还不存在，不会把自己拍进去
    let image = monitor
        .capture_image()
        .map_err(|e| format!("抓取屏幕失败: {e}"))?;
    if image.width() == 0 || image.height() == 0 {
        return Err("抓到的屏幕图像为空".into());
    }

    let data_dir = app.state::<DataDir>().0.clone();
    let overlay_path = data_dir.join(OVERLAY_REL);
    if let Some(parent) = overlay_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建缓存目录失败: {e}"))?;
    }
    image
        .save_with_format(&overlay_path, image::ImageFormat::Png)
        .map_err(|e| format!("写入冻结图失败: {e}"))?;

    let restore = hide_own_windows(app);

    let session = CaptureSession { image, restore };
    {
        let state = app.state::<CaptureState>();
        let mut guard = state.0.lock().unwrap_or_else(|e| e.into_inner());
        *guard = Some(session);
    }

    // overlay 覆盖该显示器的「物理」范围：用 physical 定位/设尺寸，
    // 不受缩放比影响（builder 的 inner_size/position 是逻辑单位，不能用）
    if let Some(existing) = app.get_webview_window("overlay") {
        let _ = existing.close();
    }

    let overlay = WebviewWindowBuilder::new(app, "overlay", WebviewUrl::App("index.html".into()))
        .title("PPClip 截图")
        .inner_size(320.0, 240.0)
        .decorations(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .shadow(false)
        .transparent(false)
        .visible(false)
        .build()
        .map_err(|e| format!("创建截图窗口失败: {e}"))?;

    let _ = overlay.set_position(PhysicalPosition::new(monitor_x, monitor_y));
    let _ = overlay.set_size(PhysicalSize::new(width.max(1), height.max(1)));

    log::info!("区域截图：显示器 {width}×{height} @ ({monitor_x},{monitor_y})");
    Ok(())
}

/// overlay 前端拿到冻结图并完成首帧渲染后调用：这时才显示窗口，避免白屏闪一下
#[tauri::command]
pub fn overlay_ready(app: AppHandle) {
    if let Some(overlay) = app.get_webview_window("overlay") {
        let _ = overlay.show();
        let _ = overlay.set_focus();
    }
}

/// 冻结图绝对路径（前端用 convertFileSrc 加载）
#[tauri::command]
pub fn get_overlay_image(app: AppHandle) -> Result<String, String> {
    let data_dir = app.state::<DataDir>().0.clone();
    let state = app.state::<CaptureState>();
    let guard = state.0.lock().unwrap_or_else(|e| e.into_inner());
    guard
        .as_ref()
        .map(|_| {
            data_dir
                .join(OVERLAY_REL)
                .to_string_lossy()
                .to_string()
        })
        .ok_or_else(|| "没有进行中的截图".to_string())
}

/// 完成区域截图。x/y/w/h 是相对显示器左上角的**物理像素**
#[tauri::command]
pub fn finish_region_selection(
    app: AppHandle,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) -> Result<(), String> {
    let session = take_session(&app).ok_or_else(|| "没有进行中的截图".to_string())?;

    let image_w = session.image.width() as i32;
    let image_h = session.image.height() as i32;
    let (cx, cy, cw, ch) = clamp_rect(x, y, w, h, image_w, image_h)?;

    let cropped =
        image::imageops::crop_imm(&session.image, cx as u32, cy as u32, cw as u32, ch as u32)
            .to_image();

    let result = commit_capture(&app, &cropped);
    cleanup_overlay(&app, &session);
    result.map(|_| ())
}

/// 全屏截图：直接抓光标所在显示器
pub fn capture_fullscreen(app: &AppHandle) -> Result<(), String> {
    let monitor = locate_monitor(app)?;
    let image = monitor
        .capture_image()
        .map_err(|e| format!("抓取屏幕失败: {e}"))?;
    commit_capture(app, &image).map(|_| ())
}

#[tauri::command]
pub fn cancel_capture(app: AppHandle) -> Result<(), String> {
    cancel_capture_impl(&app);
    Ok(())
}

/// 取消截图：关 overlay、恢复窗口、删冻结图。幂等。
pub fn cancel_capture_impl(app: &AppHandle) {
    if let Some(session) = take_session(app) {
        cleanup_overlay(app, &session);
        log::debug!("截图已取消");
    }
}

/// 裁剪矩形钳制。返回 Err 表示这个选区不值得截（尺寸为 0 或完全在屏外）。
///
/// 纯函数，方便单测覆盖越界 / 负坐标 / 零尺寸。
fn clamp_rect(
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    image_w: i32,
    image_h: i32,
) -> Result<(i32, i32, i32, i32), String> {
    if image_w <= 0 || image_h <= 0 {
        return Err("屏幕图像尺寸异常".into());
    }

    // 支持反向拖拽（从右下往左上拉），先归一化再钳制
    let left = x.min(x + w);
    let top = y.min(y + h);
    let right = x.max(x + w);
    let bottom = y.max(y + h);

    let left = left.clamp(0, image_w);
    let top = top.clamp(0, image_h);
    let right = right.clamp(0, image_w);
    let bottom = bottom.clamp(0, image_h);

    let cw = right - left;
    let ch = bottom - top;
    if cw <= 0 || ch <= 0 {
        return Err("选区尺寸为 0 或在屏幕之外".into());
    }
    Ok((left, top, cw, ch))
}

/// 写剪贴板 + 入库 + 淘汰 + 通知前端
fn commit_capture(
    app: &AppHandle,
    image: &RgbaImage,
) -> Result<crate::models::UpsertOutcome, String> {
    let (width, height) = (image.width(), image.height());
    let dynamic = image::DynamicImage::ImageRgba8(image.clone());
    let png = util::encode_png(&dynamic)?;
    let thumb = util::thumbnail_png(&dynamic)?;

    // 标记自身写入，避免监听线程把刚截的图又当成一次「复制」重复入库
    let hash = util::sha256_hex(&png);
    app.state::<SelfWrite>().mark_written(hash);

    let ctx = ClipboardContext::new().map_err(|e| format!("访问剪贴板失败: {e}"))?;
    let rust_image =
        clipboard_rs::RustImageData::from_bytes(&png).map_err(|e| format!("构造图片失败: {e}"))?;
    ctx.set_image(rust_image)
        .map_err(|e| format!("写入剪贴板失败: {e}"))?;

    let outcome = {
        let storage = app.state::<Storage>();
        storage.upsert_image(&png, &thumb, width, height, "screenshot")?
    };

    let (max_items, max_image_mb) = {
        let settings = app.state::<SettingsState>();
        let guard = settings.0.lock().unwrap_or_else(|e| e.into_inner());
        (guard.max_items, guard.max_image_mb)
    };
    if let Err(err) = app
        .state::<Storage>()
        .evict(max_items, max_image_mb)
    {
        log::warn!("淘汰失败: {err}");
    }

    log::info!("截图已复制到剪贴板并入库：{width}×{height}");
    let _ = app.emit("clips:changed", &outcome);
    Ok(outcome)
}

fn take_session(app: &AppHandle) -> Option<CaptureSession> {
    let state = app.state::<CaptureState>();
    let mut guard = state.0.lock().unwrap_or_else(|e| e.into_inner());
    guard.take()
}

fn cleanup_overlay(app: &AppHandle, session: &CaptureSession) {
    if let Some(overlay) = app.get_webview_window("overlay") {
        let _ = overlay.close();
    }
    restore_windows(app, &session.restore);

    let data_dir = app.state::<DataDir>().0.clone();
    let overlay_path = data_dir.join(OVERLAY_REL);
    if let Err(err) = std::fs::remove_file(&overlay_path) {
        if err.kind() != std::io::ErrorKind::NotFound {
            log::warn!("清理冻结图失败: {err}");
        }
    }
}

/// 藏起本应用所有窗口，返回「原本可见」的 label 列表
fn hide_own_windows(app: &AppHandle) -> Vec<String> {
    let mut labels: Vec<String> = PERSISTENT_WINDOWS.iter().map(|s| s.to_string()).collect();
    for label in app.webview_windows().keys() {
        if label.starts_with("pin-") {
            labels.push(label.clone());
        }
    }

    let mut hidden = Vec::new();
    for label in labels {
        if label == "overlay" {
            continue;
        }
        if let Some(win) = app.get_webview_window(&label) {
            if win.is_visible().unwrap_or(false) {
                let _ = win.hide();
                hidden.push(label);
            }
        }
    }
    hidden
}

fn restore_windows(app: &AppHandle, labels: &[String]) {
    for label in labels {
        if let Some(win) = app.get_webview_window(label) {
            let _ = win.show();
        }
    }
}

/// 定位截图目标显示器：优先光标所在的那块
fn locate_monitor(app: &AppHandle) -> Result<xcap::Monitor, String> {
    if let Ok(pos) = app.cursor_position() {
        match xcap::Monitor::from_point(pos.x.round() as i32, pos.y.round() as i32) {
            Ok(monitor) => return Ok(monitor),
            Err(err) => log::warn!("按光标定位显示器失败（{err}），回退到第一块显示器"),
        }
    }

    xcap::Monitor::all()
        .map_err(|e| format!("枚举显示器失败: {e}"))?
        .into_iter()
        .next()
        .ok_or_else(|| "没有检测到可用显示器".to_string())
}

#[cfg(test)]
mod tests {
    use super::clamp_rect;

    #[test]
    fn clamp_keeps_inside_image() {
        assert_eq!(clamp_rect(10, 20, 100, 50, 1920, 1080).unwrap(), (10, 20, 100, 50));
    }

    #[test]
    fn clamp_trims_out_of_bounds() {
        // 选区右下越界 → 裁到图像边界
        assert_eq!(
            clamp_rect(1900, 1000, 200, 200, 1920, 1080).unwrap(),
            (1900, 1000, 20, 80)
        );
        // 负坐标 → 归零
        assert_eq!(
            clamp_rect(-50, -50, 100, 100, 1920, 1080).unwrap(),
            (0, 0, 50, 50)
        );
    }

    #[test]
    fn clamp_normalizes_reverse_drag() {
        // 从右下往左上拖：宽高为负
        assert_eq!(
            clamp_rect(300, 400, -200, -300, 1920, 1080).unwrap(),
            (100, 100, 200, 300)
        );
    }

    #[test]
    fn clamp_rejects_degenerate_selection() {
        assert!(clamp_rect(10, 10, 0, 50, 1920, 1080).is_err());
        assert!(clamp_rect(10, 10, 50, 0, 1920, 1080).is_err());
        // 完全在屏外
        assert!(clamp_rect(5000, 5000, 100, 100, 1920, 1080).is_err());
        assert!(clamp_rect(-500, -500, 100, 100, 1920, 1080).is_err());
        // 图像尺寸异常
        assert!(clamp_rect(0, 0, 10, 10, 0, 0).is_err());
    }

    #[test]
    fn clamp_handles_single_pixel_selection() {
        assert_eq!(clamp_rect(5, 5, 1, 1, 1920, 1080).unwrap(), (5, 5, 1, 1));
    }
}
