use tauri::menu::{ContextMenu, MenuBuilder, MenuItemBuilder};
use tauri::{AppHandle, Manager};

use crate::commands;

/// 菜单项 id 统一用 `<作用域>:<动作>` 或 `<作用域>:<行 id>:<动作>`：
/// 全部走原生菜单，因为悬浮球窗口只有 56×56、贴图窗口也很小，
/// HTML 菜单会被窗口边界裁掉，原生菜单可以画在窗口之外。
pub fn handle_menu_event(app: &AppHandle, id: &str) {
    let parts: Vec<&str> = id.split(':').collect();

    match parts.as_slice() {
        ["ball", action] => match *action {
            "capture" => {
                if let Err(err) = crate::capture::start_region_capture(app) {
                    log::warn!("区域截图启动失败: {err}");
                }
            }
            "panel" => crate::show_panel(app),
            "hide" => {
                if let Some(ball) = app.get_webview_window("ball") {
                    let _ = ball.hide();
                }
            }
            "quit" => {
                log::info!("菜单触发退出");
                app.exit(0);
            }
            other => log::warn!("未处理的悬浮球菜单项: {other}"),
        },
        ["clip", raw_id, action] => match raw_id.parse::<i64>() {
            Ok(clip_id) => {
                let result = match *action {
                    "copy" => commands::copy_clip_impl(app, clip_id),
                    "pin" => commands::create_pin_impl(app, clip_id).map(|_| ()),
                    "delete" => commands::delete_clip_impl(app, clip_id),
                    other => {
                        log::warn!("未处理的条目菜单项: {other}");
                        Ok(())
                    }
                };
                if let Err(err) = result {
                    log::warn!("条目菜单操作失败: {err}");
                }
            }
            Err(_) => log::warn!("条目菜单 id 解析失败: {id}"),
        },
        ["pin", raw_id, action] => match raw_id.parse::<i64>() {
            Ok(pin_id) => {
                let result = match *action {
                    "copy" => commands::copy_pin_impl(app, pin_id),
                    "close" => commands::delete_pin_impl(app, pin_id),
                    "zoom-in" => commands::zoom_pin_impl(app, pin_id, 1).map(|_| ()),
                    "zoom-out" => commands::zoom_pin_impl(app, pin_id, -1).map(|_| ()),
                    other => {
                        log::warn!("未处理的贴图菜单项: {other}");
                        Ok(())
                    }
                };
                if let Err(err) = result {
                    log::warn!("贴图菜单操作失败: {err}");
                }
            }
            Err(_) => log::warn!("贴图菜单 id 解析失败: {id}"),
        },
        _ => log::warn!("未处理的菜单项: {id}"),
    }
}

/// 悬浮球右键菜单
pub fn show_ball_menu(app: &AppHandle) -> Result<(), String> {
    let capture = MenuItemBuilder::with_id("ball:capture", "区域截图")
        .build(app)
        .map_err(|e| e.to_string())?;
    let panel = MenuItemBuilder::with_id("ball:panel", "打开面板")
        .build(app)
        .map_err(|e| e.to_string())?;
    let hide = MenuItemBuilder::with_id("ball:hide", "隐藏悬浮球")
        .build(app)
        .map_err(|e| e.to_string())?;
    let quit = MenuItemBuilder::with_id("ball:quit", "退出 PPClip")
        .build(app)
        .map_err(|e| e.to_string())?;

    let menu = MenuBuilder::new(app)
        .items(&[&capture, &panel, &hide, &quit])
        .build()
        .map_err(|e| e.to_string())?;

    popup(app, "ball", menu)
}

/// 历史条目右键菜单
pub fn show_clip_menu(app: &AppHandle, clip_id: i64) -> Result<(), String> {
    let copy = MenuItemBuilder::with_id(format!("clip:{clip_id}:copy"), "复制")
        .build(app)
        .map_err(|e| e.to_string())?;
    let pin = MenuItemBuilder::with_id(format!("clip:{clip_id}:pin"), "贴到桌面")
        .build(app)
        .map_err(|e| e.to_string())?;
    let delete = MenuItemBuilder::with_id(format!("clip:{clip_id}:delete"), "删除")
        .build(app)
        .map_err(|e| e.to_string())?;

    let menu = MenuBuilder::new(app)
        .items(&[&copy, &pin, &delete])
        .build()
        .map_err(|e| e.to_string())?;

    popup(app, "main", menu)
}

/// 贴图右键菜单
pub fn show_pin_menu(app: &AppHandle, pin_id: i64) -> Result<(), String> {
    let copy = MenuItemBuilder::with_id(format!("pin:{pin_id}:copy"), "复制")
        .build(app)
        .map_err(|e| e.to_string())?;
    let zoom_in = MenuItemBuilder::with_id(format!("pin:{pin_id}:zoom-in"), "放大")
        .build(app)
        .map_err(|e| e.to_string())?;
    let zoom_out = MenuItemBuilder::with_id(format!("pin:{pin_id}:zoom-out"), "缩小")
        .build(app)
        .map_err(|e| e.to_string())?;
    let close = MenuItemBuilder::with_id(format!("pin:{pin_id}:close"), "关闭贴图")
        .build(app)
        .map_err(|e| e.to_string())?;

    let menu = MenuBuilder::new(app)
        .items(&[&copy, &zoom_in, &zoom_out, &close])
        .build()
        .map_err(|e| e.to_string())?;

    popup(app, &crate::pins::pin_label(pin_id), menu)
}

fn popup(
    app: &AppHandle,
    window_label: &str,
    menu: tauri::menu::Menu<tauri::Wry>,
) -> Result<(), String> {
    // ContextMenu::popup 要的是 Window，而 Manager::get_window 在 tauri 2.12 里
    // 被 unstable feature 挡住了；用 WebviewWindow → Webview → Window 这条公开路径拿。
    let webview_window = app
        .get_webview_window(window_label)
        .ok_or_else(|| format!("窗口 {window_label} 不存在"))?;
    let window = webview_window.as_ref().window();

    menu.popup(window).map_err(|e| e.to_string())
}
