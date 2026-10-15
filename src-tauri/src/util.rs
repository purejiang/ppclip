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
    fn png_encoding_produces_png_signature() {
        let image = image::DynamicImage::ImageRgba8(image::RgbaImage::new(4, 4));
        let png = encode_png(&image).unwrap();
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);

        let thumb = thumbnail_png(&image).unwrap();
        assert_eq!(&thumb[..4], &[0x89, b'P', b'N', b'G']);
    }
}
