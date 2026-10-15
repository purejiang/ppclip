import { useEffect, useState } from "react";

import { api, dataDir } from "../api";
import type { AppSettings } from "../types";

interface Props {
  onClose: () => void;
  onChanged: () => void;
}

/** 简易设置弹层：容量、数据目录、清空、退出 */
export default function SettingsSheet({ onClose, onChanged }: Props) {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [dir, setDir] = useState("");
  const [clearing, setClearing] = useState(false);

  useEffect(() => {
    void (async () => {
      setSettings(await api.getSettings());
      setDir(await dataDir());
    })();
  }, []);

  const patch = async (changes: Partial<AppSettings>) => {
    if (!settings) return;
    const next = { ...settings, ...changes };
    setSettings(next);
    await api.updateSettings(next);
    onChanged();
  };

  const clearAll = async () => {
    setClearing(true);
    try {
      await api.clearClips("all");
      onChanged();
    } finally {
      setClearing(false);
    }
  };

  return (
    <div className="sheet-backdrop" onClick={onClose}>
      <div className="sheet" onClick={(e) => e.stopPropagation()}>
        <header className="sheet-header">
          <span>设置</span>
          <button type="button" className="icon-button" onClick={onClose} title="关闭">
            ✕
          </button>
        </header>

        <div className="sheet-body">
          <label className="field">
            <span>历史条目上限</span>
            <input
              type="number"
              min={10}
              max={5000}
              value={settings?.maxItems ?? 300}
              onChange={(e) => void patch({ maxItems: Number(e.target.value) || 300 })}
            />
          </label>

          <label className="field">
            <span>图片存储上限（MB）</span>
            <input
              type="number"
              min={0}
              max={5000}
              value={settings?.maxImageMb ?? 200}
              onChange={(e) => void patch({ maxImageMb: Number(e.target.value) || 0 })}
            />
          </label>

          <div className="field field--stack">
            <span>数据目录</span>
            <code className="path">{dir}</code>
          </div>

          <div className="sheet-actions">
            <button type="button" className="btn" onClick={() => void api.openDataDir()}>
              打开数据目录
            </button>
            <button
              type="button"
              className="btn btn--danger"
              disabled={clearing}
              onClick={() => void clearAll()}
            >
              {clearing ? "清理中…" : "清空全部历史"}
            </button>
          </div>

          <div className="sheet-actions">
            <button
              type="button"
              className="btn"
              onClick={() => {
                void api.showBall();
              }}
            >
              显示悬浮球
            </button>
            <button type="button" className="btn btn--danger" onClick={() => void api.quitApp()}>
              退出 PPClip
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
