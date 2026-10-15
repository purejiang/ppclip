use clipboard_rs::common::RustImage;
use clipboard_rs::{Clipboard, ClipboardContext};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::models::{AppSettings, Clip, ClipKind, ClipStats, Pin, UpsertOutcome};
use crate::storage::Storage;
use crate::{capture, menu, pins, util, DataDir, SelfWrite, SettingsState};

// ------------------------------------------------------------------ 设置

#[tauri::command]
pub fn get_settings(state: State<'_, SettingsState>) -> AppSettings {
    state.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

#[tauri::command]
pub fn update_settings(
    app: AppHandle,
    state: State<'_, SettingsState>,
    settings: AppSettings,
) -> Result<(), String> {
    let data_dir = app.state::<DataDir>().0.clone();
    *state.0.lock().unwrap_or_else(|e| e.into_inner()) = settings.clone();
    crate::settings::save_settings(&data_dir, &settings)?;

    // 上限调小后立刻生效，不然要等下次复制才淘汰
    if let Some(storage) = app.try_state::<Storage>() {
        if let Err(err) = storage.evict(settings.max_items, settings.max_image_mb) {
            log::warn!("按新设置淘汰失败: {err}");
        }
    }
    let _ = app.emit("clips:changed", ());
    Ok(())
}

#[tauri::command]
pub fn get_data_dir(app: AppHandle) -> String {
    app.state::<DataDir>().0.to_string_lossy().to_string()
}

#[tauri::command]
pub fn open_data_dir(app: AppHandle) -> Result<(), String> {
    let dir = app.state::<DataDir>().0.clone();
    std::process::Command::new("explorer")
        .arg(&dir)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("打开数据目录失败: {e}"))
}

// ------------------------------------------------------- 窗口 / 悬浮球

#[tauri::command]
pub fn toggle_panel(app: AppHandle) {
    crate::toggle_panel(&app);
}

#[tauri::command]
pub fn show_panel(app: AppHandle) {
    crate::show_panel(&app);
}

#[tauri::command]
pub fn hide_panel(app: AppHandle) {
    crate::hide_panel(&app);
}

/// 隐藏悬浮球。恢复入口：再次启动 PPClip（单实例回调会把它显示回来）。
#[tauri::command]
pub fn hide_ball(app: AppHandle) {
    if let Some(ball) = app.get_webview_window("ball") {
        let _ = ball.hide();
    }
}

#[tauri::command]
pub fn show_ball(app: AppHandle) {
    if let Some(ball) = app.get_webview_window("ball") {
        let _ = ball.show();
    }
}

#[tauri::command]
pub fn save_ball_position(
    app: AppHandle,
    state: State<'_, SettingsState>,
    x: i32,
    y: i32,
) -> Result<(), String> {
    save_position(&app, state, |s| {
        s.ball_x = Some(x);
        s.ball_y = Some(y);
    })
}

#[tauri::command]
pub fn save_panel_position(
    app: AppHandle,
    state: State<'_, SettingsState>,
    x: i32,
    y: i32,
) -> Result<(), String> {
    save_position(&app, state, |s| {
        s.panel_x = Some(x);
        s.panel_y = Some(y);
    })
}

fn save_position(
    app: &AppHandle,
    state: State<'_, SettingsState>,
    apply: impl FnOnce(&mut AppSettings),
) -> Result<(), String> {
    let data_dir = app.state::<DataDir>().0.clone();
    let snapshot = {
        let mut guard = state.0.lock().unwrap_or_else(|e| e.into_inner());
        apply(&mut guard);
        guard.clone()
    };
    crate::settings::save_settings(&data_dir, &snapshot)
}

#[tauri::command]
pub fn show_ball_menu(app: AppHandle) -> Result<(), String> {
    menu::show_ball_menu(&app)
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    log::info!("用户请求退出");
    app.exit(0);
}

// ------------------------------------------------------------ 剪贴板历史

#[tauri::command]
pub fn list_clips(
    storage: State<'_, Storage>,
    query: Option<String>,
    kind: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<Clip>, String> {
    let kind = kind.as_deref().and_then(ClipKind::parse);
    storage.list(
        query.as_deref(),
        kind,
        limit.unwrap_or(200),
        offset.unwrap_or(0),
    )
}

#[tauri::command]
pub fn clip_stats(storage: State<'_, Storage>) -> Result<ClipStats, String> {
    storage.stats()
}

#[tauri::command]
pub fn get_clip(storage: State<'_, Storage>, id: i64) -> Result<Clip, String> {
    storage.get(id)
}

#[tauri::command]
pub fn copy_clip(app: AppHandle, id: i64) -> Result<(), String> {
    copy_clip_impl(&app, id)
}

/// 复制一条历史到剪贴板（菜单与命令共用）
pub(crate) fn copy_clip_impl(app: &AppHandle, id: i64) -> Result<(), String> {
    let clip = app.state::<Storage>().get(id)?;
    write_to_clipboard(app, &clip)?;

    let storage = app.state::<Storage>();
    storage.touch(id)?;
    drop(storage);

    let _ = app.emit("clips:changed", UpsertOutcome { id, created: false });
    // 复制后收起面板：下一步多半是去目标窗口粘贴
    crate::hide_panel(app);
    Ok(())
}

/// 复制某个贴图背后的内容
pub(crate) fn copy_pin_impl(app: &AppHandle, pin_id: i64) -> Result<(), String> {
    let clip_id = app.state::<Storage>().get_pin(pin_id)?.clip_id;
    copy_clip_impl(app, clip_id)
}

#[tauri::command]
pub fn delete_clip(app: AppHandle, id: i64) -> Result<(), String> {
    delete_clip_impl(&app, id)
}

pub(crate) fn delete_clip_impl(app: &AppHandle, id: i64) -> Result<(), String> {
    // 条目被删时贴图会级联删除，对应的窗口也要跟着关掉
    let pin_ids = app.state::<Storage>().delete(id)?;
    for pin_id in pin_ids {
        pins::close_pin_window(app, pin_id);
    }
    let _ = app.emit("pins:changed", ());
    let _ = app.emit("clips:changed", ());
    Ok(())
}

#[tauri::command]
pub fn clear_clips(app: AppHandle, kind: Option<String>) -> Result<usize, String> {
    let kind = kind.as_deref().and_then(ClipKind::parse);
    let removed = {
        let storage = app.state::<Storage>();
        storage.clear(kind)?
    };

    // 贴图行被级联删了，窗口也要关
    if let Some(storage) = app.try_state::<Storage>() {
        match storage.list_pins() {
            Ok(pins_left) => {
                let alive: std::collections::HashSet<i64> =
                    pins_left.iter().map(|p| p.id).collect();
                for pin_id in known_pin_windows(&app) {
                    if !alive.contains(&pin_id) {
                        pins::close_pin_window(&app, pin_id);
                    }
                }
            }
            Err(err) => log::warn!("读取贴图失败: {err}"),
        }
    }

    let _ = app.emit("pins:changed", ());
    let _ = app.emit("clips:changed", ());
    Ok(removed)
}

#[tauri::command]
pub fn show_clip_menu(app: AppHandle, id: i64) -> Result<(), String> {
    menu::show_clip_menu(&app, id)
}

fn known_pin_windows(app: &AppHandle) -> Vec<i64> {
    app.webview_windows()
        .keys()
        .filter_map(|label| label.strip_prefix("pin-"))
        .filter_map(|raw| raw.parse::<i64>().ok())
        .collect()
}

fn write_to_clipboard(app: &AppHandle, clip: &Clip) -> Result<(), String> {
    let ctx = ClipboardContext::new().map_err(|e| format!("访问剪贴板失败: {e}"))?;

    match clip.kind {
        ClipKind::Text => {
            let text = clip.text.clone().unwrap_or_default();
            app.state::<SelfWrite>()
                .mark_written(util::sha256_hex(text.as_bytes()));
            ctx.set_text(text)
                .map_err(|e| format!("写入剪贴板失败: {e}"))
        }
        ClipKind::Image => {
            let data_dir = app.state::<DataDir>().0.clone();
            let rel = clip
                .image_rel
                .clone()
                .ok_or_else(|| "图片条目缺少文件".to_string())?;
            let bytes = std::fs::read(data_dir.join(&rel))
                .map_err(|e| format!("读取图片失败: {e}"))?;

            app.state::<SelfWrite>()
                .mark_written(util::sha256_hex(&bytes));
            let image = clipboard_rs::RustImageData::from_bytes(&bytes)
                .map_err(|e| format!("解析图片失败: {e}"))?;
            ctx.set_image(image)
                .map_err(|e| format!("写入剪贴板失败: {e}"))
        }
    }
}

// ---------------------------------------------------------------- 贴图

#[tauri::command]
pub fn create_pin(app: AppHandle, clip_id: i64) -> Result<Pin, String> {
    create_pin_impl(&app, clip_id)
}

pub(crate) fn create_pin_impl(app: &AppHandle, clip_id: i64) -> Result<Pin, String> {
    let clip = app.state::<Storage>().get(clip_id)?;
    let (x, y, w, h, zoom) = pins::default_geometry(app, &clip);

    let pin = {
        let storage = app.state::<Storage>();
        let id = storage.create_pin(clip_id, x, y, w, h, zoom)?;
        storage.get_pin(id)?
    };

    pins::build_pin_window(app, &pin)?;
    let _ = app.emit("pins:changed", ());
    log::info!("已贴到桌面：{}", pin_label_for_log(&pin));
    Ok(pin)
}

fn pin_label_for_log(pin: &Pin) -> String {
    format!("pin-{} (clip {})", pin.id, pin.clip_id)
}

#[tauri::command]
pub fn list_pins(storage: State<'_, Storage>) -> Result<Vec<Pin>, String> {
    storage.list_pins()
}

#[tauri::command]
pub fn get_pin(storage: State<'_, Storage>, id: i64) -> Result<Pin, String> {
    storage.get_pin(id)
}

#[tauri::command]
pub fn delete_pin(app: AppHandle, id: i64) -> Result<(), String> {
    delete_pin_impl(&app, id)
}

pub(crate) fn delete_pin_impl(app: &AppHandle, id: i64) -> Result<(), String> {
    {
        let storage = app.state::<Storage>();
        storage.delete_pin(id)?;
    }
    pins::close_pin_window(app, id);
    let _ = app.emit("pins:changed", ());
    Ok(())
}

#[tauri::command]
pub fn update_pin_geometry(
    app: AppHandle,
    id: i64,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    zoom: Option<f64>,
) -> Result<(), String> {
    let storage = app.state::<Storage>();
    let current = storage.get_pin(id)?;
    storage.update_pin_geometry(id, x, y, w, h, zoom.unwrap_or(current.zoom))
}

#[tauri::command]
pub fn zoom_pin(app: AppHandle, id: i64, direction: i32) -> Result<Pin, String> {
    zoom_pin_impl(&app, id, direction)
}

pub(crate) fn zoom_pin_impl(app: &AppHandle, id: i64, direction: i32) -> Result<Pin, String> {
    pins::zoom_pin(app, id, direction)
}

#[tauri::command]
pub fn show_pin_menu(app: AppHandle, id: i64) -> Result<(), String> {
    menu::show_pin_menu(&app, id)
}

// ---------------------------------------------------------- 截图（转发）

#[tauri::command]
pub fn start_region_capture(app: AppHandle) -> Result<(), String> {
    capture::start_region_capture(&app)
}

#[tauri::command]
pub fn capture_fullscreen(app: AppHandle) -> Result<(), String> {
    capture::capture_fullscreen(&app)
}
