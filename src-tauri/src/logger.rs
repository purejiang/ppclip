use std::path::Path;

use tauri_plugin_log::{Target, TargetKind};

/// 日志同时输出到控制台与 `<data_dir>/logs/ppclip.log`。
///
/// 发布版也保留文件日志：常驻类工具出问题时，用户日志是唯一的排查线索。
pub fn init_plugin(data_dir: &Path) -> tauri::plugin::TauriPlugin<tauri::Wry> {
    let log_dir = data_dir.join("logs");

    tauri_plugin_log::Builder::default()
        .level(log::LevelFilter::Info)
        .targets([
            Target::new(TargetKind::Stdout),
            Target::new(TargetKind::Folder {
                path: log_dir,
                file_name: Some("ppclip".into()),
            }),
        ])
        .build()
}
