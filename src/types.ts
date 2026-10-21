/** 与 Rust 侧 `models::AppSettings` 严格对应（serde 使用 camelCase） */
export interface AppSettings {
  /** 历史条目上限 */
  maxItems: number;
  /** 图片存储上限（MB） */
  maxImageMb: number;
  /** 悬浮球窗口左上角（物理像素）。**只是给旧版本的镜像**，真相是 ballAnchorX/Y */
  ballX: number | null;
  ballY: number | null;
  /** 悬浮球的形状中心（物理像素，全局坐标）—— 几何唯一真相 */
  ballAnchorX: number | null;
  ballAnchorY: number | null;
  /** 悬浮球不透明度（20~100，%） */
  ballOpacity: number;
  /** 悬浮入口是不是「贴边胶囊」（窗口尺寸恒定，靠尺寸反推不出来了） */
  ballDocked: boolean;
  /** 面板位置（物理像素，null = 居中） */
  panelX: number | null;
  panelY: number | null;
  /** 面板桌面置顶：开着 = 常驻最上层 + 点外部不关；关着 = 失焦自动收起 */
  panelPinned: boolean;
  /** 面板不透明度（20~100，%）：只调玻璃卡片底色（100 = 完全不透明），文字保持不透明 */
  panelOpacity: number;
}

export type ClipKind = "text" | "image" | "file";

/** 一条剪贴板历史，对应 Rust `models::Clip` */
export interface Clip {
  id: number;
  kind: ClipKind;
  text: string | null;
  /** 文件条目的路径列表（kind = file）；其它类型为 null */
  files: string[] | null;
  preview: string;
  /** 相对数据目录，如 `images/<hash>.png` */
  imageRel: string | null;
  thumbRel: string | null;
  width: number | null;
  height: number | null;
  bytes: number;
  source: string;
  createdAt: string;
  lastUsedAt: string;
  useCount: number;
}

/** 文件条目里单个文件的现场信息，对应 Rust `models::FileDetail` */
export interface FileDetail {
  /** 原始路径（大小写 / 分隔符都不动） */
  path: string;
  name: string;
  /** 所在目录（没有目录部分就是空串） */
  dir: string;
  /** 大写扩展名；没有扩展名 / 文件夹 = 空串 */
  ext: string;
  /** 中文类型说明（PDF 文档 / 图片 / 文件夹 / 文件 …） */
  kind: string;
  /** 按扩展名猜的 MIME（文件夹是 `inode/directory`） */
  mime: string;
  /** 字节数；文件夹或已被删除 = 0 */
  size: number;
  /** 修改时间（RFC3339，本地时区）；读不到为 null */
  modified: string | null;
  /** 文件还在不在（不在时界面要提示，复制会被后端拒绝） */
  exists: boolean;
  isDir: boolean;
}

/** 一个文件的两种校验和，对应 Rust `models::FileHashes` */
export interface FileHashes {
  sha256: string;
  md5: string;
}

/** 一条桌面贴图，对应 Rust `models::Pin` */
export interface Pin {
  id: number;
  clipId: number;
  x: number;
  y: number;
  w: number;
  h: number;
  zoom: number;
  createdAt: string;
}

export interface ClipStats {
  total: number;
  textCount: number;
  imageCount: number;
  fileCount: number;
  imageBytes: number;
}

export interface UpsertOutcome {
  id: number;
  created: boolean;
}

/** 列表筛选类型 */
export type KindFilter = "all" | ClipKind;

/** 本应用用到的窗口标签 */
export type WindowLabel =
  | "ball"
  | "main"
  | "overlay"
  | "viewer"
  | `pin-${number}`;

/** 收起态两种形态，与 Rust 侧 `ballhub::geom::BallForm` 一一对应 */
export type BallForm = "tab" | "orb";

/** 贴边胶囊贴哪一侧，与 Rust 侧 `ballhub::geom::DockSide` 一一对应 */
export type DockSide = "left" | "right";

/**
 * 悬浮入口的完整状态快照（`ball:form` 事件与 `ball_snapshot` 命令共用）。
 *
 * `rev` 单调递增：前端只接受 `rev >= 已应用值` 的快照/事件。
 * 没有它，「先 listen 再拉快照」仍有竞态 —— 快照回来得比事件晚时会把新状态盖成旧的。
 */
export interface BallSnapshot {
  rev: number;
  form: BallForm;
  wheelOpen: boolean;
  /** 贴边侧：胶囊要放在窗口朝屏幕的那条边上 */
  side: DockSide;
  anchorX: number;
  anchorY: number;
  /** 胶囊相对窗口中心的横向偏移（逻辑像素），前端写进 `--tab-shift` */
  tabShift: number;
}

/**
 * GitHub 最新 Release 的检测结果，对应 Rust `github::UpdateCheck`。
 *
 * 只检测、不下载：`url` 是 Release 页，前端调用 `openUrl` 让浏览器打开。
 */
export interface UpdateCheck {
  currentVersion: string;
  latestVersion: string;
  hasUpdate: boolean;
  /** Release 说明（纯文本，Rust 侧已截断到 800 字符） */
  notes: string;
  url: string;
  publishedAt: string | null;
}
