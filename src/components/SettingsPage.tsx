import { useEffect, useState } from "react";

import { api, dataDir } from "../api";
import {
  IconBall,
  IconClipboardArrow,
  IconDatabase,
  IconFolder,
  IconHistory,
  IconPower,
} from "./icons";
import type { AppSettings, ClipStats } from "../types";

interface Props {
  /** 设置变了：面板据此刷新列表（上限 / 存储上限会影响淘汰） */
  onChanged: () => void;
}

/** 存储上限的快捷档位（MB）：和稿子里的 `0.1GB / 1GB / 自定义` 同一个角色 */
const STORAGE_PRESETS = [100, 200, 500];

function formatMb(mb: number): string {
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`;
  return `${mb} MB`;
}

function formatBytes(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  if (bytes >= 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${bytes} B`;
}

/**
 * 设置页（照 stitch 稿）：面板窗口里的第二屏，标题栏由 `PanelApp` 换成
 * 「← 返回 + 剪贴板设置」，这里只管内容。
 *
 * 稿子里是「每项一张 `rounded-xl bg-surface-container/40` 卡片」：图标 + 标签 +
 * 右侧当前值，下面跟滑杆 / 进度条 + 分段按钮 / 开关。
 *
 * 我们砍掉了稿子里那些后端不存在的项（文件保留策略之类），并把两处「稿子有、
 * 后端原本没有」的项补齐了：**悬浮球不透明度**（写入 settings.json）与
 * **开机自启**（写注册表 Run 键，注册表是唯一真相）。
 * 「清空全部历史」按稿子放在面板状态条上，「退出」在悬浮球右键菜单里。
 */
export default function SettingsPage({ onChanged }: Props) {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [stats, setStats] = useState<ClipStats | null>(null);
  const [dir, setDir] = useState("");
  const [ballShown, setBallShown] = useState(true);
  const [autostart, setAutostart] = useState(false);
  const [customStorage, setCustomStorage] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void (async () => {
      try {
        const [next, nextStats, path, shown, auto] = await Promise.all([
          api.getSettings(),
          api.clipStats(),
          dataDir(),
          api.ballVisible(),
          api.isAutostartEnabled(),
        ]);
        setSettings(next);
        setStats(nextStats);
        setDir(path);
        setBallShown(shown);
        setAutostart(auto);
        setCustomStorage(!STORAGE_PRESETS.includes(next.maxImageMb));
      } catch (err) {
        setError(`读取设置失败：${String(err)}`);
      }
    })();
  }, []);

  const patch = async (changes: Partial<AppSettings>) => {
    if (!settings) return;
    const next = { ...settings, ...changes };
    setSettings(next);
    try {
      await api.updateSettings(next);
      onChanged();
    } catch (err) {
      setSettings(settings);
      setError(`保存设置失败：${String(err)}`);
    }
  };

  const toggleBall = async () => {
    const next = !ballShown;
    setBallShown(next);
    try {
      if (next) await api.showBall();
      else await api.hideBall();
    } catch (err) {
      setBallShown(!next);
      setError(`切换悬浮球失败：${String(err)}`);
    }
  };

  const toggleAutostart = async () => {
    const next = !autostart;
    setAutostart(next);
    try {
      await api.setAutostart(next);
    } catch (err) {
      setAutostart(!next);
      setError(`切换开机自启失败：${String(err)}`);
    }
  };

  const usedMb = (stats?.imageBytes ?? 0) / 1024 / 1024;
  const maxMb = settings?.maxImageMb ?? 200;
  const percent = maxMb > 0 ? Math.min(100, (usedMb / maxMb) * 100) : 0;

  return (
    <div className="settings-body">
      {/* 历史条目上限 */}
      <section className="settings-card">
        <div className="settings-card-head">
          <span className="settings-label">
            <IconHistory size={16} />
            历史条目上限
          </span>
          <span className="settings-value">
            {(settings?.maxItems ?? 300).toLocaleString()}
          </span>
        </div>
        <input
          className="slider"
          type="range"
          min={50}
          max={5000}
          step={50}
          value={settings?.maxItems ?? 300}
          aria-label="历史条目上限"
          onChange={(e) => void patch({ maxItems: Number(e.target.value) || 300 })}
        />
      </section>

      {/* 存储上限 */}
      <section className="settings-card">
        <div className="settings-card-head">
          <span className="settings-label">
            <IconDatabase size={16} />
            存储上限
          </span>
          <span className="settings-hint">{formatBytes(stats?.imageBytes ?? 0)} used</span>
        </div>
        <span className="settings-hint">
          已用 {usedMb.toFixed(1)} MB（已用 {percent.toFixed(0)}%）
        </span>
        <div className="progress-track">
          <div className="progress-fill" style={{ width: `${percent}%` }} />
        </div>
        <div className="settings-segment">
          {STORAGE_PRESETS.map((preset) => (
            <button
              key={preset}
              type="button"
              className={`segment-button${maxMb === preset ? " is-active" : ""}`}
              onClick={() => {
                setCustomStorage(false);
                void patch({ maxImageMb: preset });
              }}
            >
              {formatMb(preset)}
            </button>
          ))}
          <button
            type="button"
            className={`segment-button${customStorage ? " is-active" : ""}`}
            onClick={() => setCustomStorage(true)}
          >
            自定义
          </button>
        </div>
        {customStorage && (
          <input
            className="slider"
            type="range"
            min={50}
            max={5000}
            step={50}
            value={maxMb}
            aria-label="图片存储上限（MB）"
            onChange={(e) => void patch({ maxImageMb: Number(e.target.value) || 50 })}
          />
        )}
      </section>

      {/* 数据目录 */}
      <section className="settings-card">
        <div className="settings-card-head">
          <span className="settings-label">
            <IconFolder size={16} />
            数据目录管理
          </span>
        </div>
        <div className="path-row">
          <span className="path-text" title={dir}>
            {dir}
          </span>
          <button
            type="button"
            className="ghost-button"
            onClick={() => void api.openDataDir()}
          >
            <IconFolder size={13} />
            打开
          </button>
        </div>
      </section>

      {/* 悬浮球 */}
      <section className="settings-card">
        <div className="settings-card-head">
          <span className="settings-label">
            <IconBall size={16} />
            悬浮球管理
          </span>
          <button
            type="button"
            className={`switch${ballShown ? " is-on" : ""}`}
            role="switch"
            aria-checked={ballShown}
            aria-label="显示悬浮球"
            title={ballShown ? "隐藏悬浮球（托盘菜单里可找回）" : "显示悬浮球"}
            onClick={() => void toggleBall()}
          >
            <span className="switch-knob" />
          </button>
        </div>
        <div>
          <div className="settings-row">
            <span className="settings-hint">悬浮球不透明度</span>
            <span className="settings-value">{settings?.ballOpacity ?? 100}%</span>
          </div>
          <input
            className="slider"
            type="range"
            min={20}
            max={100}
            step={5}
            value={settings?.ballOpacity ?? 100}
            aria-label="悬浮球不透明度"
            onChange={(e) => void patch({ ballOpacity: Number(e.target.value) || 100 })}
          />
        </div>
      </section>

      {/* 面板外观：不透明度 */}
      <section className="settings-card">
        <div className="settings-card-head">
          <span className="settings-label">
            <IconClipboardArrow size={16} />
            面板不透明度
          </span>
          <span className="settings-value">{settings?.panelOpacity ?? 100}%</span>
        </div>
        <input
          className="slider"
          type="range"
          min={20}
          max={100}
          step={5}
          value={settings?.panelOpacity ?? 100}
          aria-label="面板不透明度"
          onChange={(e) => void patch({ panelOpacity: Number(e.target.value) || 100 })}
        />
        <span className="settings-hint">
          只影响面板的玻璃底色；100% 完全不透明，文字始终清晰
        </span>
      </section>

      {/* 开机自启 */}
      <section className="settings-card">
        <div className="settings-card-head">
          <span className="settings-label">
            <IconPower size={16} />
            开机自启
          </span>
          <button
            type="button"
            className={`switch${autostart ? " is-on" : ""}`}
            role="switch"
            aria-checked={autostart}
            aria-label="开机自启"
            title={autostart ? "关闭开机自启" : "登录时自动启动 PPClip"}
            onClick={() => void toggleAutostart()}
          >
            <span className="switch-knob" />
          </button>
        </div>
        <span className="settings-hint">
          写当前用户的 Run 注册表项，不申请管理员权限
        </span>
      </section>

      {error && <div className="panel-error">{error}</div>}
    </div>
  );
}
