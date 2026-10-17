use clipboard_rs::common::RustImage;
use clipboard_rs::{Clipboard, ClipboardContext};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::models::{AppSettings, Clip, ClipKind, ClipStats, Pin, UpsertOutcome};
use crate::storage::Storage;
use crate::{capture, menu, pins, util, viewer, DataDir, SelfWrite, SettingsState};

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
    // 悬浮球（不透明度）等窗口也要跟着变，不能等下次启动
    let _ = app.emit(SETTINGS_CHANGED_EVENT, ());
    Ok(())
}

/// 设置变化时广播（payload 为空，收到的一方自己去 `get_settings` 问）
pub const SETTINGS_CHANGED_EVENT: &str = "settings:changed";

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

/// 隐藏悬浮入口。恢复入口：再次启动 PPClip（单实例回调会把它显示回来）。
#[tauri::command]
pub fn hide_ball(app: AppHandle) {
    // 先收回枢纽态：窗口藏起来后前端收不到 mouseleave，会带着 260×260 的
    // 大窗口再出现（那时鼠标在别处，看着就像个卡住的大方块）
    if let Err(err) = crate::ballhub::set_expanded(&app, false) {
        log::warn!("隐藏悬浮球前收回枢纽态失败: {err}");
    }
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

/// 悬浮球当前是不是露着的（设置页的开关要如实反映状态）
#[tauri::command]
pub fn ball_visible(app: AppHandle) -> bool {
    app.get_webview_window("ball")
        .and_then(|ball| ball.is_visible().ok())
        .unwrap_or(false)
}

/// 开机自启开关：注册表是唯一真相，这里每次现场读
#[tauri::command]
pub fn is_autostart_enabled() -> bool {
    crate::autostart::is_enabled()
}

#[tauri::command]
pub fn set_autostart(enabled: bool) -> Result<(), String> {
    match crate::autostart::set(enabled) {
        Ok(()) => Ok(()),
        // 失败一定要落日志：前端的提示条会自己消失，注册表没写上是很难事后发现的
        Err(err) => {
            log::error!("切换开机自启失败（目标状态 {enabled}）: {err}");
            Err(err)
        }
    }
}

/// 鼠标悬停时把悬浮球撑成 HUD 轮盘（中心 + 四个卫星按钮），移开还原。
/// 几何换算见 `crate::ballhub`：形状中心不动是硬要求。
#[tauri::command]
pub fn set_ball_expanded(app: AppHandle, expanded: bool) -> Result<[i32; 2], String> {
    crate::ballhub::set_expanded(&app, expanded)?;
    // 一次 IPC 就把补偿量带回去：多查一次命令会让前端多一个 await，
    // 期间 hovered 可能已经变了，窗口撑开而 CSS 没跟上
    let (x, y) = crate::ballhub::wheel_shift(&app);
    if expanded && (x != 0 || y != 0) {
        log::info!("展开位移补偿：球要反向偏移 ({x}, {y}) 物理像素");
    }
    Ok([x, y])
}

/// 切「贴边胶囊 ↔ 常规球」。胶囊上 hover / 单击都会走到这里（稿子里的「唤出常规态」），
/// 形态显式落盘，下次启动就照它还原。
#[tauri::command]
pub fn set_ball_docked(
    app: AppHandle,
    state: State<'_, SettingsState>,
    docked: bool,
) -> Result<(), String> {
    let effective = crate::ballhub::set_docked(&app, docked)?;
    persist_ball_form(&app, state, effective)
}

/// **拖动结束**时调用：按落点决定形态（靠边吸成胶囊），并把最终几何写回设置。
///
/// 位置上报（`save_ball_position`）只记坐标、不判形态：常规球被钳进可视区后左边缘
/// 同样贴着屏幕，拿坐标反推形态会把刚唤出来的常规态吸回胶囊。
#[tauri::command]
pub fn finish_ball_drag(
    app: AppHandle,
    state: State<'_, SettingsState>,
) -> Result<(), String> {
    let docked = crate::ballhub::dock(&app)?;
    persist_ball_form(&app, state, docked)
}

/// 只记悬浮球坐标（拖动过程中前端会节流上报），不动形态
#[tauri::command]
pub fn save_ball_position(
    app: AppHandle,
    state: State<'_, SettingsState>,
    x: i32,
    y: i32,
) -> Result<(), String> {
    // 展开态下窗口是按形状中心重新定位过的，此时的左上角不是「悬浮球的位置」；
    // 存下去下次启动就会从偏移过的位置冒出来（程序自己挪窗也会触发 onMoved）
    if crate::ballhub::is_expanded(&app) {
        log::debug!("忽略展开态下的悬浮球位置上报: ({x}, {y})");
        return Ok(());
    }
    // 吸附发生在 `finish_ball_drag` 里（拖动结束那一刻），这里只照当前真实位置落盘：
    // 形状切来切去也会触发 onMoved，顺手把新位置存住
    let pos = app
        .get_webview_window("ball")
        .and_then(|ball| ball.outer_position().ok())
        .unwrap_or(tauri::PhysicalPosition::new(x, y));

    save_position(&app, state, |s| {
        s.ball_x = Some(pos.x);
        s.ball_y = Some(pos.y);
    })
}

/// 形态 + 当前左上角一起落盘（形态一变位置也跟着变，两个字段要一起写）
fn persist_ball_form(
    app: &AppHandle,
    state: State<'_, SettingsState>,
    docked: bool,
) -> Result<(), String> {
    let pos = app
        .get_webview_window("ball")
        .and_then(|ball| ball.outer_position().ok());
    save_position(app, state, |s| {
        s.ball_docked = docked;
        if let Some(pos) = pos {
            s.ball_x = Some(pos.x);
            s.ball_y = Some(pos.y);
        }
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

/// 删除条目。走后台线程：条目被删时可能连带关掉贴图窗口，
/// 而贴图窗口自己的「✕」也会调到这条路径（在自己回调栈里销毁自己同样会卡）。
#[tauri::command]
pub async fn delete_clip(app: AppHandle, id: i64) -> Result<(), String> {
    crate::run_off_main(app, move |app| delete_clip_impl(app, id)).await
}

pub(crate) fn delete_clip_impl(app: &AppHandle, id: i64) -> Result<(), String> {
    // 条目被删时贴图会级联删除，对应的窗口也要跟着关掉
    let pin_ids = app.state::<Storage>().delete(id)?;
    for pin_id in pin_ids {
        pins::close_pin_window(app, pin_id);
    }
    // 正在看的就是这条 → 窗口也关掉
    if viewer::shown_clip(app) == Some(id) {
        viewer::close_viewer(app);
    }
    let _ = app.emit("pins:changed", ());
    let _ = app.emit("clips:changed", ());
    Ok(())
}

#[tauri::command]
pub async fn clear_clips(app: AppHandle, kind: Option<String>) -> Result<usize, String> {
    crate::run_off_main(app, move |app| clear_clips_impl(app, kind.as_deref())).await
}

fn clear_clips_impl(app: &AppHandle, kind: Option<&str>) -> Result<usize, String> {
    let kind = kind.and_then(ClipKind::parse);
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
                for pin_id in known_pin_windows(app) {
                    if !alive.contains(&pin_id) {
                        pins::close_pin_window(app, pin_id);
                    }
                }
            }
            Err(err) => log::warn!("读取贴图失败: {err}"),
        }

        // 查看窗口同理：正在看的那条没了就别留着空窗口
        if let Some(shown) = viewer::shown_clip(app) {
            if storage.get(shown).is_err() {
                viewer::close_viewer(app);
            }
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

// ------------------------------------------------------------ 查看完整内容

/// 打开某条历史的完整内容（面板里只有 120 字预览 / 320px 缩略图）。
///
/// 与截图 / 贴图同理：这个命令要建窗口，必须离开 WebView2 回调栈（wry#583）。
#[tauri::command]
pub async fn open_viewer(app: AppHandle, id: i64) -> Result<(), String> {
    crate::run_off_main(app, move |app| open_viewer_impl(app, id)).await
}

/// 关闭查看窗口（Esc / 关闭按钮）。它会销毁自己的 webview，也要离开回调栈。
#[tauri::command]
pub async fn close_viewer(app: AppHandle) -> Result<(), String> {
    crate::run_off_main(app, |app| {
        viewer::close_viewer(app);
        Ok(())
    })
    .await
}

/// 查看窗口当前显示的条目 id（前端挂载后与收到 `viewer:show` 时都来问它）
#[tauri::command]
pub fn viewer_clip(app: AppHandle) -> Option<i64> {
    viewer::shown_clip(&app)
}

pub(crate) fn open_viewer_impl(app: &AppHandle, clip_id: i64) -> Result<(), String> {
    // 面板不收起：查看窗口就停在它旁边，方便一条条往下翻
    viewer::open_viewer(app, clip_id)
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

/// 贴到桌面。
///
/// 与 `start_region_capture` 同理：这个命令要建贴图窗口，必须离开 WebView2 回调栈。
#[tauri::command]
pub async fn create_pin(app: AppHandle, clip_id: i64) -> Result<Pin, String> {
    crate::run_off_main(app, move |app| create_pin_impl(app, clip_id)).await
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

/// 关闭贴图：要从贴图窗口自己的「✕」调用，所以同样离开回调栈。
#[tauri::command]
pub async fn delete_pin(app: AppHandle, id: i64) -> Result<(), String> {
    crate::run_off_main(app, move |app| delete_pin_impl(app, id)).await
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

/// 区域截图。
///
/// 必须 `async`：这个命令会建 overlay 窗口，而同步命令跑在 WebView2 的回调栈上，
/// 在那里建 webview 会在 Windows 上死锁（wry#583）—— 症状是 overlay 只建出个空壳、
/// 命令永不返回，而悬浮球与面板已经被藏起来，整个应用「消失」且再也点不到。
/// `run_off_main` 把它挪到后台线程，IPC 回调当场返回。
#[tauri::command]
pub async fn start_region_capture(app: AppHandle) -> Result<(), String> {
    crate::run_off_main(app, |app| capture::start_region_capture(app)).await
}

#[tauri::command]
pub async fn capture_fullscreen(app: AppHandle) -> Result<(), String> {
    crate::run_off_main(app, |app| capture::capture_fullscreen(app)).await
}
