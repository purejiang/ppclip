use std::sync::Mutex;
use std::time::{Duration, Instant};

use clipboard_rs::common::RustImage;
use clipboard_rs::{
    Clipboard, ClipboardContext, ClipboardHandler, ClipboardWatcher, ClipboardWatcherContext,
    ContentFormat, WatcherShutdown,
};
use tauri::{AppHandle, Emitter, Manager};

use crate::models::UpsertOutcome;
use crate::storage::Storage;
use crate::util;
use crate::SettingsState;

/// 轮询间隔。300ms 在「感知即时」与「空载开销」之间比较平衡。
const POLL_INTERVAL_MS: u64 = 300;
/// 文本上限：超过 1MB 的文本直接丢弃，不入库也不落盘
const MAX_TEXT_BYTES: usize = 1024 * 1024;
/// 一次最多记多少个文件。资源管理器里「全选整个目录」会给出上万条路径，
/// 那种东西既没人会拿来粘贴、又会把预览和指纹算爆。
const MAX_FILES: usize = 200;
/// 图片像素上限，超过先等比缩小（8K 图约 3300 万像素）
const MAX_IMAGE_PIXELS: u64 = 40_000_000;
/// 自身写入的抑制窗口：图片被重新编码后哈希对不上，靠时间窗兜底
const SELF_WRITE_WINDOW_MS: u64 = 1200;

/// 系统 / 密码管理器用来标记「别记录我」的剪贴板格式（小写比较）
const IGNORE_FORMATS: [&str; 2] = [
    "clipboard viewer ignore",
    "excludeclipboardcontentfrommonitorprocessing",
];

/// 值为 0 表示「不要记录」的 DWORD 格式
const OPT_OUT_DWORD_FORMATS: [&str; 2] = [
    "canincludeinclipboardhistory",
    "canuploadtocloudclipboard",
];

/// 记录本应用自己写入剪贴板的内容，避免「点一次历史 → 又录一条」。
#[derive(Default)]
pub struct SelfWrite {
    inner: Mutex<SelfWriteState>,
}

#[derive(Default)]
struct SelfWriteState {
    hash: Option<String>,
    until: Option<Instant>,
}

impl SelfWrite {
    pub fn new() -> Self {
        Self::default()
    }

    /// 本应用即将写入剪贴板时调用
    pub fn mark_written(&self, hash: String) {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.hash = Some(hash);
        guard.until = Some(Instant::now() + Duration::from_millis(SELF_WRITE_WINDOW_MS));
    }

    /// 判断这次变化是否来自自身写入。命中即消费掉抑制窗口（只抑制紧随其后的那一次）。
    pub fn take_if_self_write(&self, hash: &str) -> bool {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());

        if guard.hash.as_deref() == Some(hash) {
            guard.hash = None;
            guard.until = None;
            return true;
        }

        let in_window = guard
            .until
            .map(|until| Instant::now() < until)
            .unwrap_or(false);
        if in_window {
            // 例如我们写入图片 A，剪贴板读回来重新编码后哈希变了
            guard.hash = None;
            guard.until = None;
            return true;
        }

        false
    }
}

/// 监听线程的停止句柄，退出时用来收尾
pub struct WatcherHandle(pub Mutex<Option<WatcherShutdown>>);

/// 剪贴板变化处理器：读内容 → 过滤 → 入库 → 通知前端
struct ClipWatcher {
    app: AppHandle,
}

impl ClipboardHandler for ClipWatcher {
    fn on_clipboard_change(&mut self) {
        if let Err(err) = self.handle_change() {
            // 空剪贴板、被别的程序占用等都属于正常情况，只留 debug 日志
            log::debug!("剪贴板变化未入库: {err}");
        }
    }
}

impl ClipWatcher {
    fn handle_change(&self) -> Result<(), String> {
        let ctx = ClipboardContext::new().map_err(|e| e.to_string())?;

        if should_ignore(&ctx) {
            log::debug!("剪贴板被标记为「不记录」，跳过");
            return Ok(());
        }

        // 文件必须**先于文本**判定：资源管理器复制文件时会同时放出路径文本，
        // 先看文本就会把「复制三个文件」记成一条路径文本。反过来不成立
        // （普通文本复制不会带 CF_HDROP），所以这个顺序是安全的。
        if ctx.has(ContentFormat::Files) {
            return self.store_files(&ctx);
        }
        if let Some(text) = read_text(&ctx) {
            return self.store_text(&text);
        }
        if ctx.has(ContentFormat::Image) {
            return self.store_image(&ctx);
        }
        Ok(())
    }

    fn store_files(&self, ctx: &ClipboardContext) -> Result<(), String> {
        let raw = ctx.get_files().map_err(|e| e.to_string())?;
        let paths = util::normalize_files(&raw);

        if paths.is_empty() {
            return Ok(());
        }
        if paths.len() > MAX_FILES {
            log::info!("一次复制了 {} 个文件，超过上限 {MAX_FILES}，跳过入库", paths.len());
            return Ok(());
        }

        // 指纹与 `commands::write_to_clipboard` 的 `mark_written` 用同一个函数：
        // 归一化 + 同一套 JSON 序列化，两边算出来的 hash 必须一致（否则复制历史会被重录）
        let hash = util::files_key(&paths);
        if self.is_self_write(&hash) {
            return Ok(());
        }

        let outcome = {
            let storage = self.storage()?;
            storage.upsert_files(&paths, "clipboard")?
        };
        self.finish(outcome)
    }

    fn store_text(&self, text: &str) -> Result<(), String> {
        if text.len() > MAX_TEXT_BYTES {
            log::info!("文本 {} 字节超过上限，跳过入库", text.len());
            return Ok(());
        }

        let hash = util::sha256_hex(text.as_bytes());
        if self.is_self_write(&hash) {
            return Ok(());
        }

        let outcome = {
            let storage = self.storage()?;
            storage.upsert_text(text, "clipboard")?
        };
        self.finish(outcome)
    }

    fn store_image(&self, ctx: &ClipboardContext) -> Result<(), String> {
        let raw = ctx.get_image().map_err(|e| e.to_string())?;
        let (width, height) = raw.get_size();
        if width == 0 || height == 0 {
            return Ok(());
        }

        let dynamic = raw.get_dynamic_image().map_err(|e| e.to_string())?;
        let dynamic = downscale_if_huge(dynamic, width, height);

        let (final_w, final_h) = (dynamic.width(), dynamic.height());
        let png = util::encode_png(&dynamic)?;
        let thumb = util::thumbnail_png(&dynamic)?;

        let hash = util::sha256_hex(&png);
        if self.is_self_write(&hash) {
            return Ok(());
        }

        let outcome = {
            let storage = self.storage()?;
            storage.upsert_image(&png, &thumb, final_w, final_h, "clipboard")?
        };
        self.finish(outcome)
    }

    fn storage(&self) -> Result<tauri::State<'_, Storage>, String> {
        self.app
            .try_state::<Storage>()
            .ok_or_else(|| "存储尚未初始化".to_string())
    }

    fn is_self_write(&self, hash: &str) -> bool {
        match self.app.try_state::<SelfWrite>() {
            Some(state) => state.take_if_self_write(hash),
            None => false,
        }
    }

    /// 入库之后：按设置淘汰 + 通知前端刷新
    fn finish(&self, outcome: UpsertOutcome) -> Result<(), String> {
        let (max_items, max_image_mb) = match self.app.try_state::<SettingsState>() {
            Some(settings) => {
                let guard = settings.0.lock().unwrap_or_else(|e| e.into_inner());
                (guard.max_items, guard.max_image_mb)
            }
            None => (300, 200),
        };

        if let Some(storage) = self.app.try_state::<Storage>() {
            match storage.evict(max_items, max_image_mb) {
                Ok(report) if report.removed > 0 => log::info!(
                    "淘汰 {} 条旧记录，释放 {} 字节",
                    report.removed,
                    report.freed_bytes
                ),
                Ok(_) => {}
                Err(err) => log::warn!("淘汰失败: {err}"),
            }
        }

        if let Err(err) = self.app.emit("clips:changed", &outcome) {
            log::warn!("广播 clips:changed 失败: {err}");
        }
        Ok(())
    }
}

/// 启动监听线程，返回停止句柄
pub fn start_watcher(app: AppHandle) -> Result<WatcherShutdown, String> {
    let mut watcher =
        ClipboardWatcherContext::new_with_interval(Duration::from_millis(POLL_INTERVAL_MS))
            .map_err(|e| format!("创建剪贴板监听失败: {e}"))?;

    let shutdown = watcher
        .add_handler(ClipWatcher { app })
        .get_shutdown_channel();

    std::thread::Builder::new()
        .name("ppclip-clipboard".into())
        .spawn(move || {
            // start_watch 是阻塞调用，直到 shutdown.stop() 才会返回
            watcher.start_watch();
            log::info!("剪贴板监听线程已退出");
        })
        .map_err(|e| format!("启动剪贴板监听线程失败: {e}"))?;

    log::info!("剪贴板监听已启动，轮询间隔 {POLL_INTERVAL_MS}ms");
    Ok(shutdown)
}

/// 退出前停掉监听线程，避免留下轮询线程
pub fn stop_watcher(app: &AppHandle) {
    if let Some(handle) = app.try_state::<WatcherHandle>() {
        let mut guard = handle.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(shutdown) = guard.take() {
            shutdown.stop();
            log::info!("已请求停止剪贴板监听");
        }
    }
}

fn read_text(ctx: &ClipboardContext) -> Option<String> {
    match ctx.get_text() {
        Ok(text) if !text.trim().is_empty() => Some(text),
        _ => None,
    }
}

/// 系统 / 密码管理器显式要求不要记录的，直接放行（返回 true = 跳过）
fn should_ignore(ctx: &ClipboardContext) -> bool {
    let Ok(formats) = ctx.available_formats() else {
        return false;
    };

    for format in &formats {
        let lower = format.to_ascii_lowercase();
        if IGNORE_FORMATS.contains(&lower.as_str()) {
            log::debug!("剪贴板含忽略格式 {format}");
            return true;
        }
        if OPT_OUT_DWORD_FORMATS.contains(&lower.as_str()) {
            // 这些格式带一个 DWORD：0 表示「不要包含进历史」
            if let Ok(buffer) = ctx.get_buffer(format) {
                if buffer.len() >= 4
                    && u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) == 0
                {
                    log::debug!("剪贴板格式 {format} = 0，跳过");
                    return true;
                }
            }
        }
    }
    false
}

fn downscale_if_huge(dynamic: image::DynamicImage, width: u32, height: u32) -> image::DynamicImage {
    let pixels = width as u64 * height as u64;
    if pixels <= MAX_IMAGE_PIXELS {
        return dynamic;
    }

    let scale = (MAX_IMAGE_PIXELS as f64 / pixels as f64).sqrt();
    let target_w = ((width as f64 * scale) as u32).max(1);
    let target_h = ((height as f64 * scale) as u32).max(1);
    log::info!("图片 {width}×{height} 超过像素上限，缩放到 {target_w}×{target_h}");

    dynamic.resize(target_w, target_h, image::imageops::FilterType::Triangle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_write_suppresses_matching_hash_once() {
        let state = SelfWrite::new();
        state.mark_written("abc".into());

        assert!(state.take_if_self_write("abc"));
        // 抑制是一次性的
        assert!(!state.take_if_self_write("abc"));
    }

    #[test]
    fn self_write_window_suppresses_reencoded_content() {
        let state = SelfWrite::new();
        state.mark_written("original".into());

        // 图片重新编码后哈希不同，仍应落在时间窗内被抑制
        assert!(state.take_if_self_write("reencoded"));
        // 窗口已消费
        assert!(!state.take_if_self_write("reencoded"));
    }

    #[test]
    fn unrelated_change_is_recorded() {
        let state = SelfWrite::new();
        assert!(!state.take_if_self_write("whatever"));
    }
}
