use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::AppHandle;

/// 托盘图标 id（`app.tray_by_id(TRAY_ID)` 可拿到它）
pub const TRAY_ID: &str = "ppclip-tray";

/// 菜单项 id 全部沿用 `ball:` 前缀 —— 托盘菜单和悬浮球右键菜单是同一批动作，
/// 落进 `menu::handle_menu_event` 的同一个分支，不另开一套分发。
const ITEM_PANEL: &str = "ball:panel";
const ITEM_CAPTURE: &str = "ball:capture";
const ITEM_FULLSCREEN: &str = "ball:fullscreen";
const ITEM_SHOW_BALL: &str = "ball:show";
const ITEM_QUIT: &str = "ball:quit";

/// 双击之后 Windows 仍会补一次 `Click(Up)`（tray-icon 的 `WM_LBUTTONUP` 双击时照样发），
/// 不挡掉的话「双击托盘」= 开一次又关一次，看着像没反应。
static SUPPRESS_CLICK_UNTIL: Mutex<Option<Instant>> = Mutex::new(None);
const DOUBLE_CLICK_SUPPRESS_MS: u64 = 400;

fn suppress_click() {
    *SUPPRESS_CLICK_UNTIL.lock().unwrap_or_else(|e| e.into_inner()) =
        Some(Instant::now() + Duration::from_millis(DOUBLE_CLICK_SUPPRESS_MS));
}

fn click_suppressed() -> bool {
    let mut guard = SUPPRESS_CLICK_UNTIL.lock().unwrap_or_else(|e| e.into_inner());
    match *guard {
        Some(until) if Instant::now() < until => true,
        Some(_) => {
            *guard = None;
            false
        }
        None => false,
    }
}

/// 系统托盘：悬浮球之外的第二个入口。
///
/// 左键 = 开合面板（双击也会开面板，只开一次）；右键 = 原生菜单。
/// 菜单动作与悬浮球右键菜单完全一致（见 `menu::handle_menu_event`）；
/// 「显示悬浮球」是藏起悬浮球之后的回程，之前只能靠重启应用。
pub fn init(app: &AppHandle) -> Result<(), String> {
    let panel = MenuItemBuilder::with_id(ITEM_PANEL, "打开面板")
        .build(app)
        .map_err(|e| e.to_string())?;
    let capture = MenuItemBuilder::with_id(ITEM_CAPTURE, "区域截图")
        .build(app)
        .map_err(|e| e.to_string())?;
    let fullscreen = MenuItemBuilder::with_id(ITEM_FULLSCREEN, "全屏截图")
        .build(app)
        .map_err(|e| e.to_string())?;
    let show_ball = MenuItemBuilder::with_id(ITEM_SHOW_BALL, "显示悬浮球")
        .build(app)
        .map_err(|e| e.to_string())?;
    let quit = MenuItemBuilder::with_id(ITEM_QUIT, "退出 PPClip")
        .build(app)
        .map_err(|e| e.to_string())?;

    let menu = MenuBuilder::new(app)
        .items(&[&panel, &capture, &fullscreen])
        .separator()
        .items(&[&show_ball, &quit])
        .build()
        .map_err(|e| e.to_string())?;

    // 用应用图标（`bundle.icon` 里那张，编译期嵌进来的），不另外带一份资源
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| "没有可用的应用图标".to_string())?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip("PPClip · 剪贴板历史")
        .menu(&menu)
        // 左键用来开合面板，菜单只挂在右键上
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } => {
                if !click_suppressed() {
                    crate::toggle_panel(tray.app_handle());
                }
            }
            TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } => {
                suppress_click();
                crate::show_panel(tray.app_handle());
            }
            _ => {}
        })
        .build(app)
        .map_err(|e| format!("创建托盘图标失败: {e}"))?;

    log::info!("托盘图标已创建（左键开合面板，右键菜单）");
    Ok(())
}
