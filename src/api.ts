import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

import type { AppSettings, Clip, ClipStats, KindFilter, Pin } from "./types";

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
  /** 悬停枢纽态：Rust 按「形状中心不动」撑成 HUD 轮盘（见 src-tauri/src/ballhub.rs） */
  /** 展开 / 收回枢纽态；返回展开时窗口被钳进屏幕的位移（物理像素 `[dx, dy]`，收起恒为 0） */
  setBallExpanded: (expanded: boolean) =>
    invoke<[number, number]>("set_ball_expanded", { expanded }),
  /** 光标是否真的在悬浮球窗口里（分辨「真移开」和「窗口重定位造成的假 leave」） */
  ballCursorInside: () => invoke<boolean>("ball_cursor_inside"),
  /** 临时诊断：把收起轮盘的原因写进 Rust 日志 */
  ballTrace: (reason: string) => invoke<void>("ball_trace", { reason }),
  /** 切「贴边胶囊 ↔ 常规球」（胶囊上鼠标一放上去 + 单击都走这里） */
  setBallDocked: (docked: boolean) =>
    invoke<void>("set_ball_docked", { docked }),
  /** 拖动结束：让 Rust 按落点决定是不是吸成胶囊（位置上报本身不改形态） */
  finishBallDrag: () => invoke<void>("finish_ball_drag"),
  /** 订阅面板显示 / 隐藏（悬浮球标签上的「新条目」角标据此归零） */
  onPanelVisible: (handler: (visible: boolean) => void) =>
    listen<boolean>("panel:visible", (event) => handler(event.payload)),
  saveBallPosition: (x: number, y: number) =>
    invoke<void>("save_ball_position", { x, y }),
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
