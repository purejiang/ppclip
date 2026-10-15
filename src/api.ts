import { convertFileSrc, invoke } from "@tauri-apps/api/core";
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

  // 贴图
  createPin: (clipId: number) => invoke<Pin>("create_pin", { clipId }),
  listPins: () => invoke<Pin[]>("list_pins"),
  getPin: (id: number) => invoke<Pin>("get_pin", { id }),
  deletePin: (id: number) => invoke<void>("delete_pin", { id }),
  updatePinGeometry: (id: number, x: number, y: number, w: number, h: number) =>
    invoke<void>("update_pin_geometry", { id, x, y, w, h }),
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
