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
    ///
    /// ⚠️ 这是**窗口左上角**，只为了给旧版本 / 旧配置文件留个镜像。真相是下面的
    /// `ball_anchor_*`（形状中心）—— 窗口宽度会被 `SM_CXMINTRACK` 撑，左上角跟球在哪
    /// 没有稳定关系，拿它反推位置会漂。
    pub ball_x: Option<i32>,
    pub ball_y: Option<i32>,
    /// 悬浮球的**形状中心**（物理像素，全局坐标）—— 几何唯一真相
    pub ball_anchor_x: Option<i32>,
    pub ball_anchor_y: Option<i32>,
    /// 悬浮球不透明度（20~100，%）
    pub ball_opacity: i64,
    /// 悬浮入口是不是「贴边胶囊」。**必须显式记**：常规球被钳到屏幕边时左边缘同样贴着 0，
    /// 光看坐标分不出「贴边胶囊」和「刚被唤出来的常规球」，那样鼠标一放上去唤出的常规态
    /// 会被下一次位置上报立刻吸回胶囊
    pub ball_docked: bool,
    /// 面板位置（物理像素，None = 居中显示）
    pub panel_x: Option<i32>,
    pub panel_y: Option<i32>,
    /// 面板是不是「桌面置顶」：开着 = 常驻在最上层、点外部不关；关着 = 普通窗口、
    /// 一失焦就收起（`lib.rs` 的 `WindowEvent::Focused(false)`）
    pub panel_pinned: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            max_items: 300,
            max_image_mb: 200,
            ball_x: None,
            ball_y: None,
            ball_anchor_x: None,
            ball_anchor_y: None,
            ball_opacity: 100,
            ball_docked: false,
            panel_x: None,
            panel_y: None,
            panel_pinned: false,
        }
    }
}

/// 历史条目类型
///
/// `File` 是「资源管理器里复制的一组文件」：Windows 上走 `CF_HDROP`。
/// 路径列表以 JSON 存在 `clips.text` 列里（不另开表 / 不加列，迁移零成本），
/// 好处是 `LIKE` 搜索照样能命中文件名；读出来时由 `storage::row_to_clip` 解析成
/// 真正的 `Vec<String>`，前端不碰 JSON。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClipKind {
    Text,
    Image,
    File,
}

impl ClipKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ClipKind::Text => "text",
            ClipKind::Image => "image",
            ClipKind::File => "file",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "text" => Some(ClipKind::Text),
            "image" => Some(ClipKind::Image),
            "file" => Some(ClipKind::File),
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
    /// 文件路径列表（kind = file）。
    ///
    /// 库里是 `text` 列的 JSON 数组，这里已经解析好了；`text` 字段同时仍是原始 JSON，
    /// 只为兼容「按 text 搜索」与旧数据，**显示请用这个字段**。
    pub files: Option<Vec<String>>,
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
    pub file_count: i64,
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
