pub mod autostart;
pub mod ballhub;
pub mod capture;
pub mod clipboard;
pub mod commands;
pub mod logger;
pub mod menu;
pub mod models;
pub mod pins;
pub mod settings;
pub mod storage;
pub mod util;
pub mod viewer;

use std::path::PathBuf;
use std::sync::Mutex;

use models::AppSettings;
use tauri::{Emitter, Manager};

pub use capture::CaptureState;
pub use clipboard::{SelfWrite, WatcherHandle};

/// 面板显示 / 隐藏时广播给悬浮球（payload 是 bool）：
/// 标签上的「新条目」角标靠它决定是计数还是归零
pub const PANEL_VISIBLE_EVENT: &str = "panel:visible";

/// 面板可见性变化后通知悬浮球
fn notify_panel_visibility(app: &tauri::AppHandle) {
    let visible = app
        .get_webview_window("main")
        .and_then(|win| win.is_visible().ok())
        .unwrap_or(false);
    let _ = app.emit_to("ball", PANEL_VISIBLE_EVENT, visible);
}

/// 应用数据目录（Windows：`%APPDATA%\com.ppclip.app`）
pub struct DataDir(pub PathBuf);

/// 设置的内存副本，与 `settings.json` 保持同步
pub struct SettingsState(pub Mutex<AppSettings>);

/// 显示历史面板并聚焦（悬浮球菜单 / 第二实例唤醒都用它）
pub fn show_panel(app: &tauri::AppHandle) {
    // 面板一露面就收回枢纽态：面板是后来的置顶窗，枢纽态要是还撑着，
    // 那圈按钮只会从面板边缘探出来，而且面板是半透明玻璃、撑开的球会透出来。
    // 前端在点击路径上也会先收回（`BallApp::runSatellite`），这里是兜底。
    if let Err(err) = ballhub::set_expanded(app, false) {
        log::warn!("显示面板前收回枢纽态失败: {err}");
    }
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
    }
    notify_panel_visibility(app);
}

/// 隐藏历史面板
pub fn hide_panel(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.hide();
    }
    notify_panel_visibility(app);
}

/// 面板显示 / 隐藏切换
pub fn toggle_panel(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        if win.is_visible().unwrap_or(false) {
            let _ = win.hide();
        } else {
            // 和 show_panel 一样：面板露面就先收回枢纽态
            if let Err(err) = ballhub::set_expanded(app, false) {
                log::warn!("显示面板前收回枢纽态失败: {err}");
            }
            let _ = win.show();
            let _ = win.set_focus();
        }
    }
    notify_panel_visibility(app);
}

/// 把「会创建 / 销毁窗口」的活儿挪到独立线程执行，并等它跑完。
///
/// Windows 上 WebView2 **不允许在自己的回调里创建 widget**：`#[tauri::command]`
/// 的同步实现、原生菜单回调，都跑在 WebView2 的回调栈上，直接
/// `WebviewWindowBuilder::build()` 会永远等不到控制器创建完成的回调（死锁）。
/// Tauri 自己在 `WebviewWindowBuilder` 的文档里把这条列为已知问题（wry#583），
/// 建议「用 async 命令或独立线程」。
///
/// async 命令会被 Tauri 立刻挂到异步运行时上，IPC 回调当场返回，于是窗口创建
/// 发生在事件循环里而不是回调栈里。注意：这里之所以用 `spawn_blocking` 而不是
/// `spawn`，是因为截图路径本身是阻塞的（抓屏、写 PNG、写剪贴板）。
pub(crate) async fn run_off_main<T, F>(app: tauri::AppHandle, task: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&tauri::AppHandle) -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || task(&app))
        .await
        .map_err(|e| format!("后台任务异常结束: {e}"))?
}

/// 目录能不能真写：建目录 + 写一个探测文件（只有真写成功才算数）
fn is_writable(dir: &std::path::Path) -> bool {
    if std::fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(".ppclip-write-probe");
    let ok = std::fs::write(&probe, b"ppclip").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

/// 在 Tauri 建窗**之前**把数据目录定下来。
///
/// 窗口（以及 WebView2 自己的用户数据目录）是在 Tauri 的 setup 阶段、先于我们注册的
/// 闭包创建出来的，所以这件事不能等到闭包里再做 —— 否则受限环境会直接
/// `Failed to setup app: 拒绝访问`。
///
/// 正常走 `%APPDATA%\<identifier>`（与 tauri.conf.json 里 asset protocol 的
/// `$APPDATA/**` 对得上）；写不了（沙箱 / 受限企业环境）时依次回退到
/// `%LOCALAPPDATA%\<identifier>`、`<exe 同级>/ppclip-data`。
/// 返回 `(目录, 是否走了覆盖或回退)`。
fn resolve_data_dir_early(identifier: &str) -> (PathBuf, bool) {
    if let Ok(custom) = std::env::var("PPCLIP_DATA_DIR") {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            return (PathBuf::from(trimmed), true);
        }
    }

    let standard = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .map(|base| base.join(identifier));
    if let Some(dir) = &standard {
        if is_writable(dir) {
            return (dir.clone(), false);
        }
    }

    if let Some(base) = std::env::var_os("LOCALAPPDATA") {
        let dir = PathBuf::from(base).join(identifier);
        if is_writable(&dir) {
            return (dir, true);
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let dir = parent.join("ppclip-data");
            if is_writable(&dir) {
                return (dir, true);
            }
        }
    }

    // 都写不了：仍旧用标准位置，让 setup 里的报错把原因讲清楚
    (standard.unwrap_or_else(|| PathBuf::from(".")), false)
}

/// 解析数据目录。
///
/// 默认用 Tauri 的 `app_data_dir()`（Windows：`%APPDATA%\<identifier>`）；
/// `resolve_data_dir_early` 决定走覆盖/回退时会把 `PPCLIP_DATA_DIR` 设好，
/// 这里优先读它
fn resolve_data_dir(app: &tauri::AppHandle) -> std::io::Result<PathBuf> {
    if let Ok(custom) = std::env::var("PPCLIP_DATA_DIR") {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }

    app.path()
        .app_data_dir()
        .map_err(|e| std::io::Error::other(e.to_string()))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let context: tauri::Context<tauri::Wry> = tauri::generate_context!();

    // 必须早于 Builder：窗口与 WebView2 的目录要在这时就定好
    let (early_dir, custom) = resolve_data_dir_early(&context.config().identifier);
    if custom {
        std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", early_dir.join("webview2"));
        std::env::set_var("PPCLIP_DATA_DIR", &early_dir);
    }

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            log::info!("检测到第二个实例，唤醒已有悬浮球与面板");
            let _ = ballhub::set_expanded(app, false);
            if let Some(ball) = app.get_webview_window("ball") {
                let _ = ball.show();
            }
            show_panel(app);
        }))
        .on_menu_event(|app, event| menu::handle_menu_event(app, event.id().as_ref()))
        .on_window_event(|window, event| {
            // 面板和悬浮球只有隐藏、没有关闭：没有托盘的情况下误关就没有入口了
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let label = window.label();
                if label == "main" || label == "ball" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(|app| {
            let data_dir = resolve_data_dir(app.handle())?;
            std::fs::create_dir_all(&data_dir)
                .map_err(|e| format!("创建数据目录 {} 失败: {e}", data_dir.display()))?;

            // 日志：控制台 + <data_dir>/logs/ppclip.log
            app.handle()
                .plugin(logger::init_plugin(&data_dir))
                .map_err(|e| format!("初始化日志失败: {e}"))?;

            log::info!(
                "PPClip v{} 启动，数据目录 {}",
                app.package_info().version,
                data_dir.display()
            );

            // 走了回退/覆盖就说明标准位置写不了，明确记一笔方便排查
            if let Ok(standard) = app.path().app_data_dir() {
                if standard != data_dir {
                    log::warn!(
                        "标准数据目录 {} 不可写，已改用 {}",
                        standard.display(),
                        data_dir.display()
                    );
                }
            }

            // 状态必须在监听线程启动前挂好：监听线程会 try_state 这些状态
            let storage = storage::Storage::new(&data_dir)?;
            app.manage(storage);

            let settings = settings::load_settings(&data_dir);
            app.manage(SettingsState(Mutex::new(settings.clone())));
            app.manage(SelfWrite::new());
            app.manage(CaptureState::default());
            app.manage(viewer::ViewerState::default());
            app.manage(ballhub::BallHubState::default());
            app.manage(DataDir(data_dir));

            // 恢复悬浮球：先摆到上次的位置，再按上次记录的形态摆正几何
            // （贴边胶囊 = 吸到最近的屏幕边缘并让它整条留在屏幕内；常规球 = 形状钳进可视区）。
            // 形态不靠坐标反推 —— 常规球被钳到屏幕边时左边缘同样贴着 0，分不出来
            if let Some(ball) = app.get_webview_window("ball") {
                if let (Some(x), Some(y)) = (settings.ball_x, settings.ball_y) {
                    let _ = ball.set_position(tauri::PhysicalPosition::new(x, y));
                }
                if let Err(err) = ballhub::set_docked(app.handle(), settings.ball_docked) {
                    log::warn!("启动时摆正悬浮球失败: {err}");
                }
            }
            if let Some(panel) = app.get_webview_window("main") {
                if let (Some(x), Some(y)) = (settings.panel_x, settings.panel_y) {
                    let _ = panel.set_position(tauri::PhysicalPosition::new(x, y));
                }
            }

            // 剪贴板监听
            match clipboard::start_watcher(app.handle().clone()) {
                Ok(shutdown) => {
                    app.manage(WatcherHandle(Mutex::new(Some(shutdown))));
                }
                Err(err) => log::error!("剪贴板监听启动失败: {err}"),
            }

            // 恢复上次钉在桌面上的贴图
            pins::restore_pins(app.handle());

            // 便于脚本化与冒烟测试：启动后完成一次全屏截图并退出
            // （截图结果照常写剪贴板 + 进历史）
            if std::env::args().any(|arg| arg == "--capture-fullscreen") {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(1500));
                    let code = match capture::capture_fullscreen(&handle) {
                        Ok(()) => {
                            log::info!("--capture-fullscreen 完成");
                            0
                        }
                        Err(err) => {
                            log::error!("--capture-fullscreen 失败: {err}");
                            1
                        }
                    };
                    handle.exit(code);
                });
            }

            // 区域截图冒烟测试：只验证「冷冻图 → 藏窗口 → 建 overlay → 前端把 overlay 显示出来」
            // 这条链路（选区要人来拖），跑完就退出。这条链路曾经因为 wry#583 死锁，
            // 把悬浮球和面板永久藏在后台 —— 见 `run_off_main`。
            if std::env::args().any(|arg| arg == "--capture-region") {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(1000));
                    let code = match capture::start_region_capture(&handle) {
                        Ok(()) => {
                            let mut shown = false;
                            for _ in 0..50 {
                                std::thread::sleep(std::time::Duration::from_millis(100));
                                if capture::overlay_is_visible(&handle) {
                                    shown = true;
                                    break;
                                }
                            }
                            if shown {
                                log::info!("--capture-region 完成：overlay 已显示");
                                0
                            } else {
                                log::error!("--capture-region 失败：overlay 没有显示出来");
                                1
                            }
                        }
                        Err(err) => {
                            log::error!("--capture-region 失败: {err}");
                            1
                        }
                    };
                    handle.exit(code);
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::update_settings,
            commands::get_data_dir,
            commands::open_data_dir,
            commands::toggle_panel,
            commands::show_panel,
            commands::hide_panel,
            commands::hide_ball,
            commands::show_ball,
            commands::ball_visible,
            commands::is_autostart_enabled,
            commands::set_autostart,
            commands::set_ball_expanded,
            commands::ball_wheel_shift,
            commands::set_ball_docked,
            commands::finish_ball_drag,
            commands::save_ball_position,
            commands::save_panel_position,
            commands::show_ball_menu,
            commands::quit_app,
            commands::list_clips,
            commands::clip_stats,
            commands::get_clip,
            commands::copy_clip,
            commands::delete_clip,
            commands::clear_clips,
            commands::show_clip_menu,
            commands::open_viewer,
            commands::close_viewer,
            commands::viewer_clip,
            commands::create_pin,
            commands::list_pins,
            commands::get_pin,
            commands::delete_pin,
            commands::update_pin_geometry,
            commands::zoom_pin,
            commands::show_pin_menu,
            commands::start_region_capture,
            commands::capture_fullscreen,
            capture::overlay_ready,
            capture::get_overlay_image,
            capture::finish_region_selection,
            capture::cancel_capture,
        ])
        .build(tauri::generate_context!())
        .expect("PPClip 构建失败");

    app.run(|handle, event| {
        if let tauri::RunEvent::Exit = event {
            clipboard::stop_watcher(handle);
            log::info!("PPClip 已退出");
        }
    });
}
