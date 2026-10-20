use clipboard_rs::common::RustImage;
use clipboard_rs::{Clipboard, ClipboardContext};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::models::{
    AppSettings, Clip, ClipKind, ClipStats, FileDetail, FileHashes, Pin, UpsertOutcome,
};
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
    let mut next = settings.clone();
    {
        // 悬浮球 / 面板的几何是**窗口自己**在拖动、吸附、换形态时写的真相，
        // 设置页手里那份是打开设置那一刻的旧快照。整份回写会把期间拖出来的位置覆盖掉，
        // 所以这几项一律以内存里的当前值为准。
        let current = state.0.lock().unwrap_or_else(|e| e.into_inner());
        next.ball_x = current.ball_x;
        next.ball_y = current.ball_y;
        next.ball_anchor_x = current.ball_anchor_x;
        next.ball_anchor_y = current.ball_anchor_y;
        next.ball_docked = current.ball_docked;
        next.panel_x = current.panel_x;
        next.panel_y = current.panel_y;
        // 置顶同理：它只由面板右上角那个按钮改（`set_panel_pinned`），
        // 设置页手里那份是打开那一刻的旧快照，整份回写会把期间切过的状态抹掉
        next.panel_pinned = current.panel_pinned;
    }
    // 两个不透明度都钳一下：前端滑杆已经限了范围，但设置文件是用户可手改的明文
    next.ball_opacity = next.ball_opacity.clamp(20, 100);
    next.panel_opacity = next.panel_opacity.clamp(20, 100);
    *state.0.lock().unwrap_or_else(|e| e.into_inner()) = next.clone();
    crate::settings::save_settings(&data_dir, &next)?;

    // 上限调小后立刻生效，不然要等下次复制才淘汰
    if let Some(storage) = app.try_state::<Storage>() {
        if let Err(err) = storage.evict(next.max_items, next.max_image_mb) {
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

// ---------------------------------------------------------------- 文件详情

/// 文件条目的**现场**元信息：大小 / 修改时间 / 是否还在 / 类型说明。
///
/// 不做成入库字段 —— 这些是「打开详情那一刻」的事实，用户随时可能改、删、挪文件。
/// 存快照只会显示过期信息，还会和「复制时剔掉已不存在的路径」那一套打架。
/// 一次最多 `clipboard::MAX_FILES`(50) 条，同步 `fs::metadata` 足够快。
#[tauri::command]
pub fn file_details(paths: Vec<String>) -> Vec<FileDetail> {
    paths.iter().map(|path| util::file_detail(path)).collect()
}

/// 在资源管理器里定位某个文件（选中它）。文件已被删除时资源管理器会打开它原来的目录。
#[tauri::command]
pub fn reveal_path(path: String) -> Result<(), String> {
    std::process::Command::new("explorer")
        .arg(format!("/select,{path}"))
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("打开文件所在目录失败: {e}"))
}

/// 复制任意文本（详情卡里的「复制文件名 / 复制路径 / 复制校验码」）。
///
/// 必须记 `SelfWrite`：不然复制出来的路径会被剪贴板监听当成一条新历史录进去 ——
/// 用户只是在看详情，不该凭空多出一条路径文本。
#[tauri::command]
pub fn copy_text(app: AppHandle, text: String) -> Result<(), String> {
    let ctx = ClipboardContext::new().map_err(|e| format!("访问剪贴板失败: {e}"))?;
    app.state::<SelfWrite>()
        .mark_written(util::sha256_hex(text.as_bytes()));
    ctx.set_text(text).map_err(|e| format!("写入剪贴板失败: {e}"))
}

/// 计算文件的 **SHA-256 + MD5**，给详情卡的校验和用。
///
/// **按需触发**（用户点「计算校验和」），不随详情一起算：几百 MB 的镜像算一遍要好几秒，
/// 打开详情就卡住不合适。一次读盘同时算两个算法（`util::file_hashes`）——
/// SHA-256 做真正的完整性校验，MD5 为了和发布页 / 第三方 MD5 工具对得上。
/// `async` + `run_off_main`：哈希是纯阻塞 IO，不能占着主线程。
#[tauri::command]
pub async fn file_hashes(app: AppHandle, path: String) -> Result<FileHashes, String> {
    crate::run_off_main(app, move |_| {
        util::file_hashes(std::path::Path::new(&path))
    })
    .await
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

/// 面板「桌面置顶」开关（面板右上角的图钉）。
///
/// 两件事必须一起变，缺一个都会出现半吊子状态：
/// ① 窗口的 `always_on_top` —— 关着的时候面板是普通窗口，会被别的程序盖住；
/// ② 落盘的 `panel_pinned` —— 失焦自动收起（`lib.rs::WindowEvent::Focused`）
///    和下次启动要不要直接把面板摆出来，都读它。
#[tauri::command]
pub fn set_panel_pinned(
    app: AppHandle,
    state: State<'_, SettingsState>,
    pinned: bool,
) -> Result<(), String> {
    let data_dir = app.state::<DataDir>().0.clone();
    let snapshot = {
        let mut guard = state.0.lock().unwrap_or_else(|e| e.into_inner());
        guard.panel_pinned = pinned;
        guard.clone()
    };
    crate::settings::save_settings(&data_dir, &snapshot)?;
    crate::apply_panel_pinned(&app, pinned);
    let _ = app.emit(SETTINGS_CHANGED_EVENT, ());
    log::info!("面板桌面置顶 → {}", if pinned { "开" } else { "关" });
    Ok(())
}

/// 隐藏悬浮入口（连同轮盘）。恢复入口：再次启动 PPClip（单实例回调会把它显示回来）。
#[tauri::command]
pub fn hide_ball(app: AppHandle) {
    // 先收回轮盘：窗口藏起来后前端收不到 mouseleave，会带着那扇吃点击的置顶窗再出现
    if let Err(err) = crate::ballhub::hide(&app) {
        log::warn!("隐藏悬浮球失败: {err}");
    }
}

#[tauri::command]
pub fn show_ball(app: AppHandle) {
    if let Err(err) = crate::ballhub::show(&app) {
        log::warn!("显示悬浮球失败: {err}");
    }
}

/// 悬浮球当前是不是露着的（设置页的开关要如实反映状态）
///
/// 只看 `ball` 那扇窗：`wheel` 常显但停在屏幕外（见 `ballhub` 的 `park_wheel`），
/// 它的可见性没有意义。
#[tauri::command]
pub fn ball_visible(app: AppHandle) -> bool {
    app.get_webview_window("ball")
        .and_then(|win| win.is_visible().ok())
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

/// 诊断：把前端遇到的问题写进 Rust 日志（悬浮球这种没有控制台的窗口用它）
#[tauri::command]
pub fn frontend_log(scope: String, message: String) {
    log::warn!("[前端 {scope}] {message}");
}

/// 前端挂载时先拉一次快照（**先 listen 再拉**，靠 `rev` 保证不会被旧快照盖掉）。
#[tauri::command]
pub fn ball_snapshot(app: AppHandle) -> crate::ballhub::BallSnapshot {
    crate::ballhub::snapshot(&app)
}

/// 展开 / 收起 HUD 轮盘。
///
/// 一次调用就把窗口几何摆好，返回值是**摆好之后**的快照：前端拿它直接切 CSS，
/// 不用再查一次（多一次 IPC 就多一段「窗口已经变了、CSS 还没跟上」的空窗）。
#[tauri::command]
pub fn set_ball_wheel(app: AppHandle, open: bool) -> Result<crate::ballhub::BallSnapshot, String> {
    crate::ballhub::set_wheel(&app, open)
}

/// 光标此刻是否真的在悬浮入口里（ball ∪ wheel）。
///
/// 用来区分「用户真的把鼠标移开了」和「窗口自己动了一下造成的假 leave」：
/// 窗口改尺寸 / 位置时 Chromium 会派一次 `pointerleave`，但因为指针没动，
/// **不会再补 `pointerover`** —— 前端光靠事件永远分不清（实测后果：轮盘展开即被收回，
/// 鼠标还在球上又展开，一秒一轮地闪）。
#[tauri::command]
pub fn hub_cursor_inside(app: AppHandle) -> bool {
    crate::ballhub::cursor_inside_hub(&app)
}

/// 切「贴边胶囊 ↔ 常规球」。胶囊上 hover / 单击都会走到这里（稿子里的「唤出常规态」），
/// 形态显式落盘，下次启动就照它还原（窗口尺寸已经恒定，靠尺寸反推不出来了）。
///
/// 悬停唤出之后顺手武装**自动贴回**看门狗：鼠标离开球、球还在贴边带里就滑回胶囊。
/// 只在这一条真实交互路径上武装（内部 / 冒烟直接调 `ballhub::set_form` 不武装）。
#[tauri::command]
pub fn set_ball_docked(app: AppHandle, docked: bool) -> Result<crate::ballhub::BallSnapshot, String> {
    let snap = crate::ballhub::set_form(&app, docked)?;
    if let Err(err) = crate::ballhub::persist(&app) {
        log::warn!("保存悬浮球形态失败: {err}");
    }
    if !docked {
        crate::ballhub::arm_redock(&app);
    }
    Ok(snap)
}

/// **拖动开始**时调用（原生拖动之前）：起一个看门狗线程，按真实的左键状态判定松手，
/// 松手后自己做吸附判定。不能只靠前端 `startDragging().then(...)` —— 模态移动循环里
/// 那条 Promise 什么时候落地说不准（实测会漏掉、拖到边上不贴边）。
#[tauri::command]
pub fn begin_ball_drag(app: AppHandle) {
    crate::ballhub::begin_drag(&app);
}

/// **拖动结束**时调用：按落点决定形态（靠边就吸成胶囊），并把最终几何写回设置。
///
/// 这是全项目**唯一**判定吸附的地方 —— 挂在位置上报上会把刚唤出来的常规态吸回去
/// （常规球被钳进可视区后左边缘同样贴着屏幕）。
#[tauri::command]
pub fn finish_ball_drag(app: AppHandle) -> Result<crate::ballhub::BallSnapshot, String> {
    let snap = crate::ballhub::finish_drag(&app)?;
    if let Err(err) = crate::ballhub::persist(&app) {
        log::warn!("保存悬浮球位置失败: {err}");
    }
    Ok(snap)
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
        ClipKind::File => {
            // 已经不在磁盘上的路径要剔掉：资源管理器粘贴到一个不存在的路径会整个失败，
            // 用户看到的是「复制了但粘不出来」。剔完还剩就照常写。
            let files: Vec<String> = clip
                .files
                .clone()
                .unwrap_or_default()
                .into_iter()
                .filter(|path| std::path::Path::new(path).exists())
                .collect();

            if files.is_empty() {
                return Err("这些文件都已经不在原来的位置了".to_string());
            }
            if files.len() < clip.files.as_ref().map(Vec::len).unwrap_or(0) {
                log::info!("{} 个文件已不存在，已跳过", files.len());
            }

            // 抑制用的指纹必须与监听线程算的一致：同一个归一化 + 同一个序列化
            let normalized = util::normalize_files(&files);
            app.state::<SelfWrite>()
                .mark_written(util::files_key(&normalized));
            ctx.set_files(normalized)
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
    // 文件条目没有可画的像素：`text` 里是路径的 JSON，贴到桌面只会是一屏方括号引号
    if clip.kind == ClipKind::File {
        return Err("文件条目不支持贴到桌面".to_string());
    }
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
