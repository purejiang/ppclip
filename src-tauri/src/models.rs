use serde::{Deserialize, Serialize};

/// 应用设置，落盘为 `<data_dir>/settings.json`。
///
/// 整个结构体带 `#[serde(default)]`：旧配置文件缺字段时自动补默认值，
/// 新增字段不会让老用户的白配置解析失败。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    /// 历史条目上限（超出后按 last_used_at 淘汰）
    pub max_items: i64,
    /// 图片存储上限（MB）
    pub max_image_mb: i64,
    /// 悬浮球位置（物理像素，None = 首次启动用默认位置）
    pub ball_x: Option<i32>,
    pub ball_y: Option<i32>,
    /// 面板位置（物理像素，None = 居中显示）
    pub panel_x: Option<i32>,
    pub panel_y: Option<i32>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            max_items: 300,
            max_image_mb: 200,
            ball_x: None,
            ball_y: None,
            panel_x: None,
            panel_y: None,
        }
    }
}

/// 历史条目类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClipKind {
    Text,
    Image,
}

impl ClipKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ClipKind::Text => "text",
            ClipKind::Image => "image",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "text" => Some(ClipKind::Text),
            "image" => Some(ClipKind::Image),
            _ => None,
        }
    }
}

/// 一条剪贴板历史
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub id: i64,
    pub kind: ClipKind,
    /// 文本内容（kind = text）
    pub text: Option<String>,
    /// 列表用单行预览
    pub preview: String,
    /// 相对 data_dir 的路径，如 `images/<hash>.png`（前端用 convertFileSrc 加载）
    pub image_rel: Option<String>,
    pub thumb_rel: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    /// 原图 PNG 字节数，用于容量淘汰
    pub bytes: i64,
    /// `clipboard` | `screenshot`
    pub source: String,
    pub created_at: String,
    pub last_used_at: String,
    pub use_count: i64,
}

/// 一条桌面贴图
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pin {
    pub id: i64,
    pub clip_id: i64,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub zoom: f64,
    pub created_at: String,
}

/// 存储统计（面板底部展示）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipStats {
    pub total: i64,
    pub text_count: i64,
    pub image_count: i64,
    pub image_bytes: i64,
}

/// 新增 / 命中已有条目时的返回值
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertOutcome {
    pub id: i64,
    /// true = 新条目；false = 命中已有条目（只置顶 + 计数 +1）
    pub created: bool,
}
