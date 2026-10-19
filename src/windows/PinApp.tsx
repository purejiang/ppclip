import { useEffect, useMemo, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { api, assetUrl, dataDir, windowLabel } from "../api";
import { IconClose } from "../components/icons";
import type { Clip } from "../types";

/** 贴图几何回写节流：拖动过程中不必每一帧都写库 */
const SAVE_DELAY_MS = 300;

/**
 * 桌面贴图窗口。
 *
 * 图片铺满窗口 —— 窗口尺寸由 Rust 按原始长宽比换算，所以这里不会变形；
 * 文本则显示为可滚动的卡片。
 */
export default function PinApp() {
  const pinId = useMemo(() => Number(windowLabel().replace("pin-", "")), []);
  const [clip, setClip] = useState<Clip | null>(null);
  const [src, setSrc] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    void (async () => {
      try {
        const pin = await api.getPin(pinId);
        const loaded = await api.getClip(pin.clipId);
        setClip(loaded);
        if (loaded.imageRel) {
          setSrc(assetUrl(await dataDir(), loaded.imageRel));
        }
      } catch (err) {
        console.error("加载贴图失败", err);
        setFailed(true);
      } finally {
        // 窗口是隐藏创建的：内容就绪后再显示，避免白底闪一下
        void getCurrentWindow().show();
      }
    })();
  }, [pinId]);

  // 拖动后回写位置
  useEffect(() => {
    const win = getCurrentWindow();
    let timer: number | undefined;

    const unlisten = win.onMoved(({ payload }) => {
      window.clearTimeout(timer);
      timer = window.setTimeout(async () => {
        const size = await win.innerSize();
        void api.updatePinGeometry(
          pinId,
          Math.round(payload.x),
          Math.round(payload.y),
          size.width,
          size.height,
        );
      }, SAVE_DELAY_MS);
    });

    return () => {
      window.clearTimeout(timer);
      void unlisten.then((fn) => fn());
    };
  }, [pinId]);

  const onWheel = (e: React.WheelEvent) => {
    if (clip?.kind !== "image") return;
    void api.zoomPin(pinId, e.deltaY < 0 ? 1 : -1);
  };

  return (
    <div
      className="pin"
      data-tauri-drag-region
      onWheel={onWheel}
      onContextMenu={(e) => {
        e.preventDefault();
        void api.showPinMenu(pinId);
      }}
    >
      <button
        type="button"
        className="pin-close"
        title="关闭贴图"
        aria-label="关闭贴图"
        onClick={() => void api.deletePin(pinId)}
      >
        <IconClose size={13} />
      </button>

      {src ? (
        <img
          className="pin-image"
          src={src}
          alt=""
          draggable={false}
          data-tauri-drag-region
          onError={() => setFailed(true)}
        />
      ) : (
        /* 文本贴图：文字要能选中复制，所以**不能**把 drag-region 挂在文字上
           （挂了的话鼠标按下就被接管成拖窗，一个字都选不中）——
           拖动改走顶部那条 grip，图片贴图仍整张可拖 */
        <div className="pin-body">
          <div className="pin-grip" data-tauri-drag-region title="拖动移动贴图">
            <span className="pin-grip-dot" />
            <span className="pin-grip-dot" />
            <span className="pin-grip-dot" />
          </div>
          <div className="pin-text">
            {failed ? "贴图内容已丢失" : (clip?.text ?? clip?.preview ?? "…")}
          </div>
        </div>
      )}
    </div>
  );
}
