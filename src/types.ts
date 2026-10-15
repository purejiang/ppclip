/** 与 Rust 侧 `models::AppSettings` 严格对应（serde 使用 camelCase） */
export interface AppSettings {
  /** 历史条目上限 */
  maxItems: number;
  /** 图片存储上限（MB） */
  maxImageMb: number;
  /** 悬浮球位置（物理像素，null = 默认位置） */
  ballX: number | null;
  ballY: number | null;
  /** 面板位置（物理像素，null = 居中） */
  panelX: number | null;
  panelY: number | null;
}

export type ClipKind = "text" | "image";

/** 一条剪贴板历史，对应 Rust `models::Clip` */
export interface Clip {
  id: number;
  kind: ClipKind;
  text: string | null;
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
  imageBytes: number;
}

export interface UpsertOutcome {
  id: number;
  created: boolean;
}

/** 列表筛选类型 */
export type KindFilter = "all" | ClipKind;

/** 本应用用到的窗口标签 */
export type WindowLabel = "ball" | "main" | "overlay" | `pin-${number}`;
