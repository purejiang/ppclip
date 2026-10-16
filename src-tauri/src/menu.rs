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
            "capture" => spawn_detached(app, "区域截图", |app| {
                crate::capture::start_region_capture(app)
            }),
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
                    "view" => {
                        spawn_detached(app, "查看完整内容", move |app| {
                            commands::open_viewer_impl(app, clip_id)
                        });
                        Ok(())
                    }
                    "copy" => commands::copy_clip_impl(app, clip_id),
                    "pin" => {
                        spawn_detached(app, "贴到桌面", move |app| {
                            commands::create_pin_impl(app, clip_id).map(|_| ())
                        });
                        Ok(())
                    }
                    "delete" => {
                        spawn_detached(app, "删除条目", move |app| {
                            commands::delete_clip_impl(app, clip_id)
                        });
                        Ok(())
                    }
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
                    "close" => {
                        spawn_detached(app, "关闭贴图", move |app| {
                            commands::delete_pin_impl(app, pin_id)
                        });
                        Ok(())
                    }
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

/// 把「会创建 / 销毁 webview」的菜单动作扔到独立线程。
///
/// 菜单回调是在 `TrackPopupMenu` 的模态循环里执行的，而 `popup()` 本身是从
/// WebView2 的 IPC 回调里调用的 —— 也就是说菜单回调仍处在 WebView2 的回调栈上，
/// 在那里建 / 关 webview 会死锁（wry#583，详见 `crate::run_off_main`）。
/// 菜单没有回执通道，出错只能记日志。
fn spawn_detached(
    app: &AppHandle,
    what: &'static str,
    task: impl FnOnce(&AppHandle) -> Result<(), String> + Send + 'static,
) {
    let app = app.clone();
    std::thread::spawn(move || {
        if let Err(err) = task(&app) {
            log::warn!("{what}失败: {err}");
        }
    });
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
    let view = MenuItemBuilder::with_id(format!("clip:{clip_id}:view"), "查看完整内容")
        .build(app)
        .map_err(|e| e.to_string())?;
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
        .items(&[&view, &copy, &pin, &delete])
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
