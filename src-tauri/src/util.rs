use std::io::Cursor;

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


/// 内容的 sha256（小写十六进制），用于剪贴板去重
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
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

    #[test]
    fn png_encoding_produces_png_signature() {
        let image = image::DynamicImage::ImageRgba8(image::RgbaImage::new(4, 4));
        let png = encode_png(&image).unwrap();
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);

        let thumb = thumbnail_png(&image).unwrap();
        assert_eq!(&thumb[..4], &[0x89, b'P', b'N', b'G']);
    }
}
