import { useEffect, useRef, useState } from "react";

import { api } from "../api";
import type { UpdateCheck } from "../types";
import {
  IconClipboardArrow,
  IconClose,
  IconCopy,
  IconExternalLink,
  IconInfo,
  IconRefresh,
} from "./icons";

const REPO_URL = "https://github.com/purejiang/ppclip";
const REPO_LABEL = "github.com/purejiang/ppclip";
const RELEASES_URL = "https://github.com/purejiang/ppclip/releases";
/** 版权年份在模块加载时算一次，避免在 render 里调用 impure 的 Date */
const COPYRIGHT_YEAR = new Date().getFullYear();

/** 更新检测的 UI 状态；「检测到新版本」和「检查失败」是两条不同路径 */
type CheckState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "upToDate"; latest: string }
  | { kind: "available"; info: UpdateCheck }
  | { kind: "error"; message: string };

interface Props {
  /** 当前版本，由 PanelApp 的 getVersion() 传入 */
  version: string;
  onClose: () => void;
}

/**
 * 关于 / 版本页（照 stitch 参考稿的「关于 GLACIER PASTE」弹层）。
 *
 * - 入口：面板页脚版本号、设置页「关于 PPClip」卡片；
 * - 只检测更新：点「检查更新」请求 GitHub API，发现新版本后「前往下载」在浏览器
 *   打开 Release 页，**不自动下载、不自动安装**（不引 updater / 不签安装包）；
 * - Esc / 右上 ✕ / 点背景关闭，Esc 的层级由 `PanelApp` 处理。
 */
export default function AboutPage({ version, onClose }: Props) {
  const [check, setCheck] = useState<CheckState>({ kind: "idle" });
  const [copied, setCopied] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const copyTimer = useRef<number | undefined>(undefined);

  useEffect(() => {
    return () => window.clearTimeout(copyTimer.current);
  }, []);

  const runCheck = async () => {
    if (check.kind === "checking") return;
    setActionError(null);
    setCheck({ kind: "checking" });
    try {
      const info = await api.checkLatestRelease();
      setCheck(
        info.hasUpdate
          ? { kind: "available", info }
          : { kind: "upToDate", latest: info.latestVersion },
      );
    } catch (err) {
      const message = String(err);
      setCheck({ kind: "error", message });
      void api.frontendLog("about", `检查更新失败：${message}`).catch(() => undefined);
    }
  };

  const openReleasePage = async (url: string) => {
    setActionError(null);
    try {
      await api.openUrl(url);
    } catch (err) {
      setActionError(`打开浏览器失败：${String(err)}`);
    }
  };

  const copyRepo = async () => {
    setActionError(null);
    try {
      await api.copyText(REPO_URL);
      setCopied(true);
      window.clearTimeout(copyTimer.current);
      copyTimer.current = window.setTimeout(() => setCopied(false), 1600);
    } catch (err) {
      setActionError(`复制失败：${String(err)}`);
    }
  };

  const checking = check.kind === "checking";

  return (
    <div
      className="about-backdrop"
      role="dialog"
      aria-modal="true"
      aria-label="关于 PPClip"
      onClick={onClose}
    >
      <div className="about-card" onClick={(e) => e.stopPropagation()}>
        <header className="about-header">
          <span className="about-title">
            <IconInfo size={16} />
            关于 PPCLIP
          </span>
          <button
            type="button"
            className="app-header-button"
            title="关闭（Esc）"
            aria-label="关闭关于页"
            autoFocus
            onClick={onClose}
          >
            <IconClose size={16} />
          </button>
        </header>

        <main className="about-main">
          <div className="about-body">
          <span className="about-icon" aria-hidden="true">
            <IconClipboardArrow size={30} />
          </span>

          <div className="about-headline">
            <h2 className="about-name">PPClip</h2>
            <p className="about-version">
              {version ? `v${version}` : "PPClip"} · Windows 桌面版
            </p>
          </div>

          <div className="about-check">
            <div className="about-check-row">
              <button
                type="button"
                className="ghost-button ghost-button--primary"
                disabled={checking}
                onClick={() => void runCheck()}
              >
                <IconRefresh size={14} className={checking ? "icon-spin" : undefined} />
                {checking ? "检查中…" : check.kind === "idle" ? "检查更新" : "重新检查"}
              </button>
              {check.kind === "upToDate" && (
                <span className="about-status about-status--ok">
                  已是最新版本{version ? `（v${version}）` : ""}
                  {check.latest !== version && (
                    <span className="about-status-sub">
                      GitHub 最新发布 v{check.latest}
                    </span>
                  )}
                </span>
              )}
              {check.kind === "idle" && (
                <span className="about-status">
                  会请求 GitHub 检查版本；不上传剪贴板内容
                </span>
              )}
            </div>

            {check.kind === "available" && (
              <div className="about-update">
                <div className="about-update-head">
                  <strong>发现新版本 v{check.info.latestVersion}</strong>
                  {check.info.publishedAt && (
                    <span className="about-update-date">
                      {check.info.publishedAt.slice(0, 10)}
                    </span>
                  )}
                </div>
                {check.info.notes && (
                  <p className="about-notes">{check.info.notes}</p>
                )}
                <div className="about-actions">
                  <button
                    type="button"
                    className="ghost-button ghost-button--primary"
                    onClick={() => void openReleasePage(check.info.url)}
                  >
                    <IconExternalLink size={14} />
                    前往下载
                  </button>
                  <button
                    type="button"
                    className="ghost-button"
                    onClick={() => void openReleasePage(RELEASES_URL)}
                  >
                    打开 Releases 页
                  </button>
                </div>
                <span className="about-hint">
                  只检测版本；下载和安装仍在浏览器 / 安装包里完成。
                </span>
              </div>
            )}

            {check.kind === "error" && (
              <div className="about-error" title={check.message}>
                <span>检查更新失败：{check.message}</span>
                <button
                  type="button"
                  className="ghost-button"
                  onClick={() => void openReleasePage(RELEASES_URL)}
                >
                  <IconExternalLink size={14} />
                  打开 Releases 页
                </button>
              </div>
            )}
          </div>

          {/* 仓库卡：复制的是完整 URL，展示用短地址 */}
          <div className="about-repo">
            <div className="about-repo-meta">
              <span className="about-repo-name">GitHub 仓库</span>
              <span className="about-repo-url" title={REPO_URL}>
                {REPO_LABEL}
              </span>
            </div>
            <button
              type="button"
              className="ghost-button"
              title="复制仓库地址"
              onClick={() => void copyRepo()}
            >
              <IconCopy size={13} />
              {copied ? "已复制" : "复制"}
            </button>
          </div>

          <p className="about-desc">
            Windows 剪贴板历史、截图与桌面贴图。默认不联网；仅检查更新时请求
            GitHub，不上传剪贴板内容。
          </p>

          {actionError && <p className="about-error-line">{actionError}</p>}
          </div>
        </main>

        <footer className="about-footer">
          © {COPYRIGHT_YEAR} purejiang · MIT License
        </footer>
      </div>
    </div>
  );
}
