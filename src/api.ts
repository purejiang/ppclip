import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

import type {
  AppSettings,
  BallSnapshot,
  Clip,
  ClipStats,
  FileDetail,
  FileHashes,
  KindFilter,
  Pin,
} from "./types";

/**
 * 后端命令的类型化包装。
 * 新增命令必须两端同步：`src-tauri/src/commands.rs` ↔ 这里。
 */
export const api = {
  // 设置
  getSettings: () => invoke<AppSettings>("get_settings"),
  updateSettings: (settings: AppSettings) =>
    invoke<void>("update_settings", { settings }),
  getDataDir: () => invoke<string>("get_data_dir"),
  openDataDir: () => invoke<void>("open_data_dir"),

  // 窗口 / 悬浮球
  togglePanel: () => invoke<void>("toggle_panel"),
  showPanel: () => invoke<void>("show_panel"),
  hidePanel: () => invoke<void>("hide_panel"),
  /**
   * 面板「桌面置顶」开关：同时改窗口的 always_on_top 与落盘设置。
   * 关掉之后面板会在失焦时自动收起（Rust 侧 `WindowEvent::Focused`）。
   */
  setPanelPinned: (pinned: boolean) =>
    invoke<void>("set_panel_pinned", { pinned }),
  hideBall: () => invoke<void>("hide_ball"),
  showBall: () => invoke<void>("show_ball"),
  /** 悬浮球当前是否可见（设置页的开关如实反映状态） */
  ballVisible: () => invoke<boolean>("ball_visible"),
  /** 开机自启：注册表是唯一真相，每次现场问 */
  isAutostartEnabled: () => invoke<boolean>("is_autostart_enabled"),
  setAutostart: (enabled: boolean) => invoke<void>("set_autostart", { enabled }),
  /** 设置变化（任何窗口改了设置都会广播，悬浮球据此实时改不透明度） */
  onSettingsChanged: (handler: () => void) =>
    listen("settings:changed", () => handler()),
  /** 诊断：把前端遇到的问题写进 Rust 日志（这两扇窗没有可看的控制台） */
  frontendLog: (scope: string, message: string) =>
    invoke<void>("frontend_log", { scope, message }),

  /**
   * 悬浮入口的几何 / 形态（真相在 Rust 侧 `ballhub.rs`：窗口尺寸恒定、锚点定位置）。
   *
   * 挂载顺序必须是「先 `onBallForm` 订阅，再 `ballSnapshot` 拉一次」：事件可能在
   * 订阅与拉取之间到达，靠 `rev` 单调递增把旧快照挡掉。
   */
  ballSnapshot: () => invoke<BallSnapshot>("ball_snapshot"),
  onBallForm: (handler: (snap: BallSnapshot) => void) =>
    listen<BallSnapshot>("ball:form", (event) => handler(event.payload)),
  /** 展开 / 收起 HUD 轮盘（一次调用把窗口几何摆好，返回摆好之后的快照） */
  setBallWheel: (open: boolean) =>
    invoke<BallSnapshot>("set_ball_wheel", { open }),
  /** 光标是否真的还在悬浮入口（球 ∪ 轮盘）里 —— 分辨「真移开」和「窗口动一下的假 leave」 */
  hubCursorInside: () => invoke<boolean>("hub_cursor_inside"),
  /** 切「贴边胶囊 ↔ 常规球」（胶囊上悬停 / 单击都走这里） */
  setBallForm: (docked: boolean) =>
    invoke<BallSnapshot>("set_ball_docked", { docked }),
  /**
   * 拖动开始：让 Rust 起一个看门狗，按真实的左键状态判定「松手了没有」，松手后自己做
   * 吸附判定。**必须在 `startDragging()` 之前发**，而且不要等它返回 —— 原生移动循环
   * 一起来，主线程就被占住了，等它反而会把拖动推迟。
   */
  beginBallDrag: () => invoke<void>("begin_ball_drag"),
  /** 拖动结束：让 Rust 按落点决定是不是吸成胶囊（位置上报本身不改形态） */
  finishBallDrag: () => invoke<BallSnapshot>("finish_ball_drag"),
  /** 订阅面板显示 / 隐藏（悬浮球标签上的「新条目」角标据此归零） */
  onPanelVisible: (handler: (visible: boolean) => void) =>
    listen<boolean>("panel:visible", (event) => handler(event.payload)),
  savePanelPosition: (x: number, y: number) =>
    invoke<void>("save_panel_position", { x, y }),
  showBallMenu: () => invoke<void>("show_ball_menu"),
  quitApp: () => invoke<void>("quit_app"),

  // 剪贴板历史
  listClips: (query: string, kind: KindFilter, limit = 200) =>
    invoke<Clip[]>("list_clips", {
      query: query.trim() === "" ? null : query,
      kind: kind === "all" ? null : kind,
      limit,
      offset: 0,
    }),
  clipStats: () => invoke<ClipStats>("clip_stats"),
  getClip: (id: number) => invoke<Clip>("get_clip", { id }),
  copyClip: (id: number) => invoke<void>("copy_clip", { id }),
  deleteClip: (id: number) => invoke<void>("delete_clip", { id }),
  clearClips: (kind: KindFilter) =>
    invoke<number>("clear_clips", { kind: kind === "all" ? null : kind }),
  showClipMenu: (id: number) => invoke<void>("show_clip_menu", { id }),
  /**
   * 文件条目的现场元信息（大小 / 修改时间 / 是否还在）。
   * **不落库**：打开详情时现场 stat，删过 / 挪过的文件会如实显示 `exists: false`。
   */
  fileDetails: (paths: string[]) => invoke<FileDetail[]>("file_details", { paths }),
  /** 在资源管理器里定位文件（选中它） */
  revealPath: (path: string) => invoke<void>("reveal_path", { path }),
  /** 复制任意文本（文件名 / 路径 / 校验码）；后端会记 SelfWrite，不会多出一条历史 */
  copyText: (text: string) => invoke<void>("copy_text", { text }),
  /**
   * 按需算文件的 SHA-256 + MD5（流式读一遍算两个，几百 MB 也不会卡主线程）。
   * 不随详情一起算：大文件算一遍要好几秒，打开详情就卡住不合适。
   */
  fileHashes: (path: string) => invoke<FileHashes>("file_hashes", { path }),
  /** 历史或贴图有变化（后端广播，前端据此刷新列表 / 计数） */
  onClipsChanged: (handler: () => void) =>
    listen("clips:changed", () => handler()),
  onPinsChanged: (handler: () => void) => listen("pins:changed", () => handler()),

  // 查看完整内容（全文 / 原图，单窗口跟着当前条目换内容）
  openViewer: (id: number) => invoke<void>("open_viewer", { id }),
  closeViewer: () => invoke<void>("close_viewer"),
  viewerClip: () => invoke<number | null>("viewer_clip"),

  // 贴图
  createPin: (clipId: number) => invoke<Pin>("create_pin", { clipId }),
  listPins: () => invoke<Pin[]>("list_pins"),
  getPin: (id: number) => invoke<Pin>("get_pin", { id }),
  deletePin: (id: number) => invoke<void>("delete_pin", { id }),
  updatePinGeometry: (id: number, x: number, y: number, w: number, h: number) =>
    // zoom 显式传 null：保持 Rust 侧记录的缩放值，不依赖「缺字段」的隐式语义
    invoke<void>("update_pin_geometry", { id, x, y, w, h, zoom: null }),
  zoomPin: (id: number, direction: 1 | -1) =>
    invoke<Pin>("zoom_pin", { id, direction }),
  showPinMenu: (id: number) => invoke<void>("show_pin_menu", { id }),

  // 截图
  startRegionCapture: () => invoke<void>("start_region_capture"),
  captureFullscreen: () => invoke<void>("capture_fullscreen"),
  getOverlayImage: () => invoke<string>("get_overlay_image"),
  overlayReady: () => invoke<void>("overlay_ready"),
  finishRegionSelection: (x: number, y: number, w: number, h: number) =>
    invoke<void>("finish_region_selection", { x, y, w, h }),
  cancelCapture: () => invoke<void>("cancel_capture"),
};

/** 当前窗口的 label（简化调用处样板） */
export function windowLabel(): string {
  return getCurrentWindow().label;
}

/** 把 `<数据目录>/<相对路径>` 转成 webview 可加载的 asset URL */
let dataDirCache: string | null = null;

export async function dataDir(): Promise<string> {
  if (dataDirCache === null) {
    dataDirCache = await api.getDataDir();
  }
  return dataDirCache;
}

export function assetUrl(dir: string, relPath: string): string {
  const sep = dir.endsWith("\\") || dir.endsWith("/") ? "" : "\\";
  return convertFileSrc(`${dir}${sep}${relPath.replace(/\//g, "\\")}`);
}
