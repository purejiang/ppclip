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

use std::path::PathBuf;
use std::sync::Mutex;

use models::AppSettings;
use tauri::Manager;

pub use capture::CaptureState;
pub use clipboard::{SelfWrite, WatcherHandle};

/// 应用数据目录（Windows：`%APPDATA%\com.ppclip.app`）
pub struct DataDir(pub PathBuf);

/// 设置的内存副本，与 `settings.json` 保持同步
pub struct SettingsState(pub Mutex<AppSettings>);

/// 显示历史面板并聚焦（悬浮球菜单 / 第二实例唤醒都用它）
pub fn show_panel(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
    }
}

/// 隐藏历史面板
pub fn hide_panel(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.hide();
    }
}

/// 面板显示 / 隐藏切换
pub fn toggle_panel(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        if win.is_visible().unwrap_or(false) {
            let _ = win.hide();
        } else {
            let _ = win.show();
            let _ = win.set_focus();
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            log::info!("检测到第二个实例，唤醒已有悬浮球与面板");
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
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;

            // 日志：控制台 + <data_dir>/logs/ppclip.log
            app.handle().plugin(logger::init_plugin(&data_dir))?;

            log::info!(
                "PPClip v{} 启动，数据目录 {}",
                app.package_info().version,
                data_dir.display()
            );

            // 状态必须在监听线程启动前挂好：监听线程会 try_state 这些状态
            let storage = storage::Storage::new(&data_dir)?;
            app.manage(storage);

            let settings = settings::load_settings(&data_dir);
            app.manage(SettingsState(Mutex::new(settings.clone())));
            app.manage(SelfWrite::new());
            app.manage(CaptureState::default());
            app.manage(DataDir(data_dir));

            // 恢复悬浮球上次位置
            if let Some(ball) = app.get_webview_window("ball") {
                if let (Some(x), Some(y)) = (settings.ball_x, settings.ball_y) {
                    let _ = ball.set_position(tauri::PhysicalPosition::new(x, y));
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
