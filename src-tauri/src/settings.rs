use std::path::{Path, PathBuf};

use crate::models::AppSettings;

/// 设置文件路径
pub fn settings_path(data_dir: &Path) -> PathBuf {
    data_dir.join("settings.json")
}

/// 读取设置；文件缺失或损坏都退回默认值，不让启动失败
pub fn load_settings(data_dir: &Path) -> AppSettings {
    let path = settings_path(data_dir);
    match std::fs::read_to_string(&path) {
        Ok(raw) => match serde_json::from_str::<AppSettings>(&raw) {
            Ok(settings) => settings,
            Err(err) => {
                log::warn!("settings.json 解析失败，改用默认设置: {err}");
                AppSettings::default()
            }
        },
        Err(_) => AppSettings::default(),
    }
}

/// 原子性写设置：先写临时文件再替换，避免写到一半崩溃留下坏文件
pub fn save_settings(data_dir: &Path, settings: &AppSettings) -> Result<(), String> {
    let path = settings_path(data_dir);
    let raw = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;

    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, raw).map_err(|e| format!("写入 {} 失败: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("替换 {} 失败: {e}", path.display()))
}
