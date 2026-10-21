use std::io::Cursor;

use md5::Md5;
use sha2::{Digest, Sha256};

/// 缩略图最长边
pub const THUMB_MAX_EDGE: u32 = 320;

/// 把动态图编码成 PNG
pub fn encode_png(image: &image::DynamicImage) -> Result<Vec<u8>, String> {
    let mut buffer = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut buffer), image::ImageFormat::Png)
        .map_err(|e| format!("PNG 编码失败: {e}"))?;
    Ok(buffer)
}

/// 生成列表用的缩略图（保持长宽比）
pub fn thumbnail_png(image: &image::DynamicImage) -> Result<Vec<u8>, String> {
    encode_png(&image.thumbnail(THUMB_MAX_EDGE, THUMB_MAX_EDGE))
}

/// 摘要 → 小写十六进制
fn hex_digest(digest: &[u8]) -> String {
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// 内容的 sha256（小写十六进制），用于剪贴板去重
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex_digest(&hasher.finalize())
}

/// 生成列表用的单行预览。
///
/// 两个要点：
/// 1. 压缩换行 / 连续空白，否则列表里会出现多行抖动；
/// 2. 按 **字符** 而不是字节截断——中文占 3 字节，按字节切会 panic 在 UTF-8 中间。
pub fn truncate_preview(text: &str, max_chars: usize) -> String {
    let mut collapsed = String::with_capacity(text.len().min(max_chars * 4));
    let mut last_was_space = false;

    for ch in text.chars() {
        if ch.is_whitespace() {
            if !last_was_space && !collapsed.is_empty() {
                collapsed.push(' ');
            }
            last_was_space = true;
            continue;
        }
        last_was_space = false;
        collapsed.push(ch);
    }

    let trimmed = collapsed.trim_end();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }

    let mut out: String = trimmed.chars().take(max_chars).collect();
    out.push('…');
    out
}

/// 当前时间的 RFC3339 字符串（UTC）
pub fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// 文件路径列表的归一化：去空白、丢掉空串、按原顺序去重。
///
/// **采集（`clipboard.rs`）与自身写入抑制（`commands.rs`）必须用同一套归一化**：
/// 两边算出来的指纹要一致，否则「点一条文件历史去复制」会被监听线程当成新内容再录一遍。
pub fn normalize_files(paths: &[String]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(paths.len());
    for raw in paths {
        let path = raw.trim();
        if path.is_empty() {
            continue;
        }
        // Windows 路径大小写不敏感：`C:\A.txt` 与 `c:\a.txt` 是同一个文件
        if seen.insert(path.to_ascii_lowercase()) {
            out.push(path.to_string());
        }
    }
    out
}

/// 文件条目的内容指纹：路径列表 → JSON → sha256。
///
/// 两个要点：
/// 1. 序列化方式就是落库那一份（`storage::upsert_files` 也调它），所以「同一组文件」
///    无论如何进来都得到同一个 hash —— 去重和自身写入抑制都靠这个前提；
/// 2. **指纹按小写算**：Windows 路径大小写不敏感，`C:\A.txt` 与 `c:\a.txt` 是同一个文件，
///    从不同程序复制过来不该产生两条历史。落库的路径本身保持原样（大小写要还原给资源管理器）。
pub fn files_key(paths: &[String]) -> String {
    let lowered: Vec<String> = paths.iter().map(|p| p.to_ascii_lowercase()).collect();
    let json = serde_json::to_string(&lowered).unwrap_or_default();
    sha256_hex(json.as_bytes())
}

/// 文件条目的 JSON 载荷（落 `clips.text` 列）
pub fn files_payload(paths: &[String]) -> String {
    serde_json::to_string(paths).unwrap_or_else(|_| "[]".into())
}

/// 文件条目的单行预览：单个文件给文件名，多个给「第一个 等 N 个文件」
pub fn files_preview(paths: &[String]) -> String {
    let Some(first) = paths.first() else {
        return String::new();
    };
    let name = first
        .rsplit(['\\', '/'])
        .find(|part| !part.is_empty())
        .unwrap_or(first);

    let raw = if paths.len() == 1 {
        name.to_string()
    } else {
        format!("{name} 等 {} 个文件", paths.len())
    };
    truncate_preview(&raw, 120)
}

/// 文件名的大写扩展名（`Q4_计划账单.pdf` → `PDF`）；没有扩展名或文件夹 = 空串。
///
/// 用 `Path::extension` 而不是按 `.` 切：`... ` / `a.` / `.gitignore` 这些边界它都处理好了。
pub fn file_ext(name: &str) -> String {
    std::path::Path::new(name)
        .extension()
        .map(|ext| ext.to_string_lossy().to_uppercase())
        .unwrap_or_default()
}

/// 扩展名 → 中文类型说明（详情卡副标题）。不认识的给「文件」，不猜。
pub fn file_kind_label(ext: &str) -> &'static str {
    match ext.to_ascii_lowercase().as_str() {
        "pdf" => "PDF 文档",
        "doc" | "docx" | "rtf" | "odt" => "Word 文档",
        "xls" | "xlsx" | "csv" | "ods" => "表格",
        "ppt" | "pptx" | "odp" => "演示文稿",
        "txt" | "md" | "log" | "json" | "xml" | "yml" | "yaml" | "ini" | "toml" | "conf" => {
            "文本文件"
        }
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg" | "ico" | "tif" | "tiff" => "图片",
        "mp4" | "mov" | "mkv" | "avi" | "webm" | "flv" => "视频",
        "mp3" | "wav" | "flac" | "m4a" | "ogg" | "aac" => "音频",
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" => "压缩包",
        "exe" | "msi" | "bat" | "cmd" | "ps1" | "dll" => "程序",
        "psd" | "ai" | "fig" | "sketch" | "xd" => "设计稿",
        "ttf" | "otf" | "woff" | "woff2" => "字体",
        _ => "文件",
    }
}

/// 扩展名 → MIME（详情页页脚的 `MIME: …`）。只覆盖常见的，其它给 octet-stream。
pub fn mime_for_ext(ext: &str) -> &'static str {
    match ext.to_ascii_lowercase().as_str() {
        "pdf" => "application/pdf",
        "txt" | "md" | "log" => "text/plain",
        "json" => "application/json",
        "xml" => "application/xml",
        "html" | "htm" => "text/html",
        "csv" => "text/csv",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "zip" => "application/zip",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" => "application/vnd.ms-powerpoint",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        _ => "application/octet-stream",
    }
}

/// 单个文件的详情（现场 stat，不入库）。
///
/// 路径可能已经不存在：`exists = false`、`size = 0`，但名字 / 目录 / 扩展名仍按路径字符串给
/// —— 界面要能显示「这个文件已经不在了」，而不是留下一行空白。
pub fn file_detail(path: &str) -> crate::models::FileDetail {
    let parsed = std::path::Path::new(path);
    let name = parsed
        .file_name()
        .map(|part| part.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string());
    let dir = parsed
        .parent()
        .map(|parent| parent.to_string_lossy().to_string())
        .unwrap_or_default();

    let meta = std::fs::metadata(parsed).ok();
    let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
    let ext = if is_dir { String::new() } else { file_ext(&name) };
    let kind = if is_dir {
        "文件夹"
    } else {
        file_kind_label(&ext)
    };
    let mime = if is_dir {
        "inode/directory"
    } else {
        mime_for_ext(&ext)
    };

    crate::models::FileDetail {
        path: path.to_string(),
        name,
        dir,
        ext,
        kind: kind.to_string(),
        mime: mime.to_string(),
        size: meta.as_ref().map(|m| m.len() as i64).unwrap_or(0),
        modified: meta
            .as_ref()
            .and_then(|m| m.modified().ok())
            .map(|time| chrono::DateTime::<chrono::Local>::from(time).to_rfc3339()),
        exists: meta.is_some(),
        is_dir,
    }
}

/// 流式计算文件的 **SHA-256 + MD5**（详情卡的「文件校验和」，一次读盘算出两个）。
///
/// 为什么不把整个文件读进内存：这里可能是几百 MB 的镜像 / 视频，按 1 MiB 分块喂 hasher 就行。
/// 两个算法共用**同一遍读取** —— 分成两次读会把大文件的 IO 翻倍。
///
/// 为什么两个都给：SHA-256 是可信的完整性校验（MD5 早就能构造碰撞，不适合当安全校验），
/// 但发布页 / 第三方工具（Md5Checker 这类）习惯贴 MD5，只有 SHA-256 时用户没法直接比对。
pub fn file_hashes(path: &std::path::Path) -> Result<crate::models::FileHashes, String> {
    use std::io::Read;

    let mut file = std::fs::File::open(path).map_err(|e| format!("打开文件失败: {e}"))?;
    let mut sha = Sha256::new();
    let mut md5 = Md5::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|e| format!("读取文件失败: {e}"))?;
        if read == 0 {
            break;
        }
        sha.update(&buffer[..read]);
        md5.update(&buffer[..read]);
    }
    Ok(crate::models::FileHashes {
        sha256: hex_digest(&sha.finalize()),
        md5: hex_digest(&md5.finalize()),
    })
}

/// 人类可读的字节数，用于面板底部的存储占用提示
pub fn human_bytes(bytes: i64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    let value = bytes.max(0) as f64;
    if value >= MB {
        format!("{:.1} MB", value / MB)
    } else if value >= KB {
        format!("{:.0} KB", value / KB)
    } else {
        format!("{bytes} B")
    }
}

/// 只允许打开本仓库的 GitHub 链接（关于页的「前往下载」/「打开 Releases 页」）。
///
/// 前端只传固定常量，但命令层再挡一次：`open_url` 是通用外链入口，不能变成
/// 「任何 URL 都能让应用去打开」的洞。
pub fn is_allowed_url(url: &str) -> bool {
    const REPO: &str = "https://github.com/purejiang/ppclip";
    url == REPO || url.starts_with("https://github.com/purejiang/ppclip/")
}

/// 用默认浏览器打开本仓库的链接（关于页的「前往下载 / 打开 Releases 页」）。
///
/// 与 `open_data_dir` 同一套路：`explorer <url>` 会交给系统默认浏览器。
/// 用 `run_off_main` 调用，别在 WebView2 回调栈里做进程启动。
pub fn open_url(url: &str) -> Result<(), String> {
    if !is_allowed_url(url) {
        return Err(format!("不允许打开这个链接：{url}"));
    }
    log::info!("打开链接：{url}");
    std::process::Command::new("explorer")
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("打开链接失败: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_is_stable_and_hex() {
        let a = sha256_hex("你好".as_bytes());
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(a, sha256_hex("你好".as_bytes()));
        assert_ne!(a, sha256_hex("你好 ".as_bytes()));
    }

    #[test]
    fn preview_collapses_whitespace() {
        assert_eq!(truncate_preview("a\n\n  b\tc ", 120), "a b c");
        assert_eq!(truncate_preview("   ", 120), "");
    }

    #[test]
    fn preview_truncates_on_char_boundary_without_panic() {
        let text = "中文预览".repeat(50);
        let preview = truncate_preview(&text, 10);
        assert!(preview.ends_with('…'));
        assert_eq!(preview.chars().count(), 11);

        // 极限：一个多字节字符 + 上限 1
        assert_eq!(truncate_preview("中文", 1), "中…");
    }

    #[test]
    fn human_bytes_formats() {
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(2048), "2 KB");
        assert_eq!(human_bytes(5 * 1024 * 1024), "5.0 MB");
    }

    #[test]
    fn files_are_normalized_and_deduped() {
        let raw = vec![
            "  C:\\a\\b.txt ".to_string(),
            String::new(),
            "c:\\A\\B.txt".to_string(), // 与第一条同一个文件（大小写不敏感）
            "D:\\c.jpg".to_string(),
            "   ".to_string(),
        ];
        let files = normalize_files(&raw);
        assert_eq!(files, vec!["C:\\a\\b.txt".to_string(), "D:\\c.jpg".to_string()]);
    }

    #[test]
    fn files_key_ignores_duplicates_but_not_order() {
        let a = vec!["C:\\x.txt".to_string(), "C:\\y.txt".to_string()];
        let mut dup = a.clone();
        dup.push("C:\\X.TXT".to_string());
        assert_eq!(files_key(&normalize_files(&a)), files_key(&normalize_files(&dup)));

        let mut swapped = a.clone();
        swapped.reverse();
        assert_ne!(files_key(&a), files_key(&swapped));
    }

    #[test]
    fn files_preview_uses_file_name_only() {
        assert_eq!(
            files_preview(&["C:\\Users\\me\\报告.docx".to_string()]),
            "报告.docx"
        );
        assert_eq!(
            files_preview(&["C:\\a\\1.png".to_string(), "C:\\a\\2.png".to_string()]),
            "1.png 等 2 个文件"
        );
        assert_eq!(files_preview(&[]), "");
    }

    /// 单测写临时文件用：落在 `target/test-tmp`。
    ///
    /// **别用 `std::env::temp_dir()`**：受限环境（沙箱 / 受限令牌）下那里可能写不进去
    /// （只读的 `%TEMP%` 会直接 `PermissionDenied`），而且它跟项目数据无关，
    /// 出问题时很难判断是代码错还是环境限制。
    fn scratch_path(name: &str) -> std::path::PathBuf {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-tmp");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(format!("{}-{name}", std::process::id()))
    }

    #[test]
    fn file_ext_survives_odd_names() {
        assert_eq!(file_ext("Q4_计划账单.pdf"), "PDF");
        assert_eq!(file_ext("a.tar.gz"), "GZ");
        // 没有扩展名 / 只有结尾的点 / 隐藏文件：都算「没有扩展名」，不能显示成「.」
        assert_eq!(file_ext("README"), "");
        assert_eq!(file_ext("a."), "");
        assert_eq!(file_ext(".gitignore"), "");
    }

    #[test]
    fn file_kind_label_is_case_insensitive() {
        assert_eq!(file_kind_label("PDF"), "PDF 文档");
        assert_eq!(file_kind_label("docx"), "Word 文档");
        assert_eq!(file_kind_label("PNG"), "图片");
        // 不认识的不猜，落到「文件」
        assert_eq!(file_kind_label("weird"), "文件");
        assert_eq!(mime_for_ext("pdf"), "application/pdf");
        assert_eq!(mime_for_ext("bin"), "application/octet-stream");
    }

    /// 详情要读**真实**的磁盘状态：大小 / 修改时间 / 是否还在
    #[test]
    fn file_detail_reports_live_metadata() {
        let path = scratch_path("file-detail.txt");
        std::fs::write(&path, b"hello").unwrap();
        let raw = path.to_string_lossy().to_string();

        let detail = file_detail(&raw);
        assert_eq!(detail.name, path.file_name().unwrap().to_string_lossy());
        assert_eq!(detail.ext, "TXT");
        assert_eq!(detail.kind, "文本文件");
        assert_eq!(detail.size, 5);
        assert!(detail.exists);
        assert!(!detail.is_dir);
        assert!(detail.modified.is_some());
        let hashes = file_hashes(&path).unwrap();
        assert_eq!(
            hashes.sha256,
            sha256_hex(b"hello"),
            "流式 sha256 必须和一次性算的一致"
        );
        assert_eq!(
            hashes.md5, "5d41402abc4b2a76b9719d911017c592",
            "hello 的 MD5 是这个固定值"
        );

        // 删掉之后仍然能描述这个路径，但 exists = false / size = 0
        std::fs::remove_file(&path).unwrap();
        let gone = file_detail(&raw);
        assert!(!gone.exists);
        assert_eq!(gone.size, 0);
        assert_eq!(gone.kind, "文本文件", "名字里的类型信息不因文件消失而丢");
        assert!(file_hashes(&path).is_err());
    }

    /// 分块流式读取跨块后两个哈希都要和一次性算的一致（1 MiB 的块边界最容易出错）
    #[test]
    fn file_hashes_match_across_chunk_boundaries() {
        let path = scratch_path("hashes-big.bin");
        let bytes: Vec<u8> = (0..(1024 * 1024 * 2 + 7)).map(|i| (i % 251) as u8).collect();
        std::fs::write(&path, &bytes).unwrap();

        let hashes = file_hashes(&path).unwrap();
        assert_eq!(hashes.sha256, sha256_hex(&bytes));
        let mut md5 = Md5::new();
        md5.update(&bytes);
        assert_eq!(hashes.md5, hex_digest(&md5.finalize()));

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn file_detail_marks_directories() {
        let dir = std::env::temp_dir();
        let detail = file_detail(&dir.to_string_lossy());
        assert!(detail.is_dir);
        assert!(detail.exists);
        assert_eq!(detail.kind, "文件夹");
        assert_eq!(detail.ext, "", "文件夹不挂扩展名标签");
    }

    #[test]
    fn png_encoding_produces_png_signature() {
        let image = image::DynamicImage::ImageRgba8(image::RgbaImage::new(4, 4));
        let png = encode_png(&image).unwrap();
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);

        let thumb = thumbnail_png(&image).unwrap();
        assert_eq!(&thumb[..4], &[0x89, b'P', b'N', b'G']);
    }

    #[test]
    fn open_url_only_allows_this_repository() {
        assert!(is_allowed_url("https://github.com/purejiang/ppclip"));
        assert!(is_allowed_url(
            "https://github.com/purejiang/ppclip/releases"
        ));
        assert!(is_allowed_url(
            "https://github.com/purejiang/ppclip/releases/tag/v0.2.5"
        ));
        assert!(!is_allowed_url("http://github.com/purejiang/ppclip"));
        assert!(!is_allowed_url("https://github.com/someone/else"));
        assert!(!is_allowed_url("https://github.com/purejiang/ppclip-evil"));
        assert!(!is_allowed_url("file:///C:/Windows/System32/calc.exe"));
    }
}
