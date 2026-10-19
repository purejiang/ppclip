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
  anchorX: number;
  anchorY: number;
}
