//! 更新检测（只检测、不下载安装）。
//!
//! 关于页的「检查更新」调用 [`check_latest`]：请求 GitHub Releases API 的
//! `releases/latest`，拿最新 tag 和当前版本做语义化比较，返回给前端展示。
//! **下载仍然是手动的**：前端只负责打开 Release 页（`commands::open_url`）。
//!
//! ## 为什么不用 HTTP 插件 / reqwest
//!
//! 本应用是 Windows-only，WinHTTP 是系统自带的：零新运行时依赖、走系统代理和
//! 系统证书库（对国内网络 / 公司代理更友好），也不用把通用网络能力暴露给 WebView。
//! 这里只发一个固定的 GET，没有重定向、没有请求体，raw Win32 足够。
//!
//! ## 注意
//!
//! - GitHub API 未认证时限流 60 次/小时；只在用户点「检查更新」时请求，足够。
//! - 拿不到网络 / 被限流 / 仓库还没有 Release 都返回可读的中文错误，
//!   前端显示错误 +「打开 Releases 页」兜底，不阻塞任何本地功能。
//! - API 的 `tag_name` 是唯一版本真相；`html_url` 只用于打开浏览器，必须再校验前缀。

use serde::{Deserialize, Serialize};
use semver::Version;

const REPO_RELEASES_URL: &str = "https://github.com/purejiang/ppclip/releases";
const RELEASE_URL_PREFIX: &str = "https://github.com/purejiang/ppclip/releases/";
const API_PATH: &str = "/repos/purejiang/ppclip/releases/latest";
const API_HOST: &str = "api.github.com";
/// 响应体上限：Release JSON 正常只有几 KB，超过这个数说明不对劲
const MAX_BODY_BYTES: usize = 512 * 1024;
/// 发布说明最多展示的字符数（中文按 char 截，别按字节）
const MAX_NOTES_CHARS: usize = 800;

/// GitHub Releases API 里我们需要的字段（未知字段 serde 默认忽略）
#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    published_at: Option<String>,
}

/// 给关于页的检测结果（字段名与 `src/types.ts` 的 `UpdateCheck` 一一对应）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub notes: String,
    pub url: String,
    pub published_at: Option<String>,
}

/// 请求 GitHub API 的最新 Release。
///
/// 同步阻塞调用：命令层必须用 `run_off_main` 包起来，别占 WebView2 回调栈。
fn fetch_latest_release() -> Result<GhRelease, String> {
    #[cfg(windows)]
    {
        fetch_latest_release_winhttp()
    }
    #[cfg(not(windows))]
    {
        Err("检查更新目前只支持 Windows".to_string())
    }
}

/// 检测 + 组装结果（不联网的单测覆盖 `build_update_check`）
pub fn check_latest(current_version: &str) -> Result<UpdateCheck, String> {
    let release = fetch_latest_release()?;
    build_update_check(current_version, release)
}

/// 把 API 结果转成前端要的状态（纯函数，有单测）
fn build_update_check(current_version: &str, release: GhRelease) -> Result<UpdateCheck, String> {
    let current = Version::parse(current_version)
        .map_err(|e| format!("当前版本不是语义化版本（{current_version}）：{e}"))?;
    let latest = normalize_tag(&release.tag_name)
        .ok_or_else(|| format!("无法解析最新版本标签：{}", release.tag_name))?;

    let url = if release.html_url.starts_with(RELEASE_URL_PREFIX) {
        release.html_url
    } else {
        // 不信任 API 返回的任意 URL：不是本仓库 Release 就退回 Releases 首页
        REPO_RELEASES_URL.to_string()
    };

    Ok(UpdateCheck {
        current_version: current.to_string(),
        latest_version: latest.to_string(),
        has_update: latest > current,
        notes: truncate_chars(release.body.trim(), MAX_NOTES_CHARS),
        url,
        published_at: release.published_at,
    })
}

/// `v0.2.5` / `0.2.5` 都归一化成 semver
fn normalize_tag(tag: &str) -> Option<Version> {
    let tag = tag.trim();
    let tag = tag
        .strip_prefix('v')
        .or_else(|| tag.strip_prefix('V'))
        .unwrap_or(tag);
    Version::parse(tag).ok()
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max_chars).collect();
    out.push('…');
    out
}

// ------------------------------------------------------------------ WinHTTP

#[cfg(windows)]
mod winhttp {
    use std::ffi::c_void;

    /// 自动使用系统代理（WinINET / PAC）；失败时回退默认代理配置
    const WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY: u32 = 4;
    const WINHTTP_ACCESS_TYPE_DEFAULT_PROXY: u32 = 0;
    const WINHTTP_FLAG_SECURE: u32 = 0x0080_0000;
    const WINHTTP_QUERY_STATUS_CODE: u32 = 19;
    const WINHTTP_QUERY_FLAG_NUMBER: u32 = 0x2000_0000;

    #[link(name = "winhttp")]
    unsafe extern "system" {
        fn WinHttpOpen(
            agent: *const u16,
            access_type: u32,
            proxy: *const u16,
            proxy_bypass: *const u16,
            flags: u32,
        ) -> isize;
        fn WinHttpConnect(
            session: isize,
            host: *const u16,
            port: u16,
            reserved: u32,
        ) -> isize;
        fn WinHttpOpenRequest(
            connect: isize,
            verb: *const u16,
            object: *const u16,
            version: *const u16,
            referrer: *const u16,
            accept_types: *const *const u16,
            flags: u32,
        ) -> isize;
        fn WinHttpSetTimeouts(
            handle: isize,
            resolve: i32,
            connect: i32,
            send: i32,
            receive: i32,
        ) -> i32;
        fn WinHttpSendRequest(
            request: isize,
            headers: *const u16,
            headers_len: u32,
            optional: *mut c_void,
            optional_len: u32,
            total_len: u32,
            context: usize,
        ) -> i32;
        fn WinHttpReceiveResponse(request: isize, reserved: *mut c_void) -> i32;
        fn WinHttpQueryHeaders(
            request: isize,
            info_level: u32,
            name: *const u16,
            buffer: *mut c_void,
            buffer_len: *mut u32,
            index: *mut u32,
        ) -> i32;
        fn WinHttpReadData(
            request: isize,
            buffer: *mut c_void,
            to_read: u32,
            read: *mut u32,
        ) -> i32;
        fn WinHttpCloseHandle(handle: isize) -> i32;
    }

    /// HINTERNET 的 RAII 包装：无论哪一步失败都别漏句柄
    struct Handle(isize);

    impl Handle {
        fn check(raw: isize, step: &str) -> Result<Self, String> {
            if raw == 0 {
                Err(format!("{step}失败: {}", std::io::Error::last_os_error()))
            } else {
                Ok(Self(raw))
            }
        }
    }

    impl Drop for Handle {
        fn drop(&mut self) {
            if self.0 != 0 {
                // SAFETY: 句柄由 WinHttpOpen/Connect/OpenRequest 创建，且只在这里关闭一次
                unsafe {
                    WinHttpCloseHandle(self.0);
                }
            }
        }
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn last_error(step: &str) -> String {
        format!("{step}失败: {}", std::io::Error::last_os_error())
    }

    pub(super) fn fetch() -> Result<super::GhRelease, String> {
        let agent = wide(&format!("PPClip/{}", env!("CARGO_PKG_VERSION")));
        let session_raw = unsafe {
            WinHttpOpen(
                agent.as_ptr(),
                WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
                std::ptr::null(),
                std::ptr::null(),
                0,
            )
        };
        let session = if session_raw != 0 {
            Handle(session_raw)
        } else {
            // 老系统 / 组策略禁用自动代理：退回默认配置再试一次
            let raw = unsafe {
                WinHttpOpen(
                    agent.as_ptr(),
                    WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
                    std::ptr::null(),
                    std::ptr::null(),
                    0,
                )
            };
            Handle::check(raw, "初始化 WinHTTP")?
        };

        // 解析 / 连接 / 发送 / 接收各 10 秒；GitHub API 不可达时别让按钮转太久
        unsafe {
            WinHttpSetTimeouts(session.0, 5000, 10000, 10000, 10000);
        }

        let host = wide(super::API_HOST);
        let connect = Handle::check(
            unsafe { WinHttpConnect(session.0, host.as_ptr(), 443, 0) },
            "连接 GitHub API",
        )?;

        let verb = wide("GET");
        let object = wide(super::API_PATH);
        let request = Handle::check(
            unsafe {
                WinHttpOpenRequest(
                    connect.0,
                    verb.as_ptr(),
                    object.as_ptr(),
                    std::ptr::null(),
                    std::ptr::null(),
                    std::ptr::null(),
                    WINHTTP_FLAG_SECURE,
                )
            },
            "创建更新检查请求",
        )?;

        // User-Agent 用 WinHttpOpen 的 agent 参数（GitHub 要求有 UA）；这里补 Accept 和版本头
        let headers = wide(
            "Accept: application/vnd.github+json\r\n\
             X-GitHub-Api-Version: 2022-11-28\r\n",
        );
        if unsafe {
            WinHttpSendRequest(
                request.0,
                headers.as_ptr(),
                u32::MAX, // -1：headers 是 NUL 结尾字符串
                std::ptr::null_mut(),
                0,
                0,
                0,
            )
        } == 0
        {
            return Err(last_error("发送更新检查请求"));
        }

        if unsafe { WinHttpReceiveResponse(request.0, std::ptr::null_mut()) } == 0 {
            return Err(last_error("接收更新检查响应"));
        }

        let mut status: u32 = 0;
        let mut status_len: u32 = std::mem::size_of::<u32>() as u32;
        if unsafe {
            WinHttpQueryHeaders(
                request.0,
                WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                std::ptr::null(),
                &mut status as *mut u32 as *mut c_void,
                &mut status_len,
                std::ptr::null_mut(),
            )
        } == 0
        {
            return Err(last_error("读取 GitHub API 状态码"));
        }

        match status {
            200 => {}
            404 => return Err("GitHub 仓库还没有发布版本".to_string()),
            403 | 429 => {
                return Err("GitHub 接口暂时不可用（可能被限流），请稍后再试".to_string())
            }
            other => return Err(format!("GitHub API 返回 HTTP {other}")),
        }

        let mut body: Vec<u8> = Vec::new();
        let mut buffer = [0u8; 8192];
        loop {
            let mut read: u32 = 0;
            if unsafe {
                WinHttpReadData(
                    request.0,
                    buffer.as_mut_ptr() as *mut c_void,
                    buffer.len() as u32,
                    &mut read,
                )
            } == 0
            {
                return Err(last_error("读取更新检查响应"));
            }
            if read == 0 {
                break;
            }
            body.extend_from_slice(&buffer[..read as usize]);
            if body.len() > super::MAX_BODY_BYTES {
                return Err("GitHub API 响应过大，已中止".to_string());
            }
        }

        serde_json::from_slice(&body)
            .map_err(|e| format!("解析 GitHub API 响应失败: {e}"))
    }
}

#[cfg(windows)]
fn fetch_latest_release_winhttp() -> Result<GhRelease, String> {
    winhttp::fetch()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str) -> GhRelease {
        GhRelease {
            tag_name: tag.to_string(),
            body: "修复若干问题".to_string(),
            html_url: format!("https://github.com/purejiang/ppclip/releases/tag/{tag}"),
            published_at: Some("2026-10-20T08:00:00Z".to_string()),
        }
    }

    #[test]
    fn normalizes_the_v_prefix() {
        assert_eq!(normalize_tag("v0.2.5"), Some(Version::new(0, 2, 5)));
        assert_eq!(normalize_tag("V0.2.5"), Some(Version::new(0, 2, 5)));
        assert_eq!(normalize_tag("0.2.5"), Some(Version::new(0, 2, 5)));
        assert_eq!(
            normalize_tag("v0.2.6-beta.1").map(|v| v.to_string()),
            Some("0.2.6-beta.1".to_string())
        );
        assert_eq!(normalize_tag("release-2026"), None);
    }

    #[test]
    fn compares_versions_numerically() {
        let current = Version::new(0, 2, 9);
        assert!(Version::new(0, 2, 10) > current, "0.2.10 必须大于 0.2.9");
        assert!(!(Version::new(0, 2, 9) > current));
        assert!(!(Version::new(0, 2, 8) > current));
    }

    #[test]
    fn builds_an_update_when_remote_is_newer() {
        let check = build_update_check("0.2.5", release("v0.2.6")).unwrap();
        assert!(check.has_update);
        assert_eq!(check.current_version, "0.2.5");
        assert_eq!(check.latest_version, "0.2.6");
        assert_eq!(check.notes, "修复若干问题");
        assert_eq!(
            check.url,
            "https://github.com/purejiang/ppclip/releases/tag/v0.2.6"
        );
        assert!(check.published_at.is_some());
    }

    #[test]
    fn no_update_for_same_or_older_remote() {
        assert!(!build_update_check("0.2.5", release("v0.2.5")).unwrap().has_update);
        assert!(!build_update_check("0.2.5", release("v0.2.0")).unwrap().has_update);
        assert!(build_update_check("0.2.9", release("v0.2.10")).unwrap().has_update);
    }

    #[test]
    fn rejects_a_non_semver_remote_tag() {
        let err = build_update_check("0.2.5", release("nightly")).unwrap_err();
        assert!(err.contains("无法解析最新版本标签"), "err={err}");
    }

    #[test]
    fn falls_back_to_the_releases_page_for_untrusted_urls() {
        let mut rel = release("v0.2.6");
        rel.html_url = "https://evil.example.com/ppclip".to_string();
        let check = build_update_check("0.2.5", rel).unwrap();
        assert_eq!(check.url, REPO_RELEASES_URL);
    }

    #[test]
    fn truncates_long_release_notes_on_char_boundaries() {
        let mut rel = release("v0.2.6");
        rel.body = "更".repeat(MAX_NOTES_CHARS + 20);
        let check = build_update_check("0.2.5", rel).unwrap();
        assert_eq!(check.notes.chars().count(), MAX_NOTES_CHARS + 1);
        assert!(check.notes.ends_with('…'));
        // 边界附近不能因为按字节截断而 panic（中文 3 字节）
        let mut rel = release("v0.2.6");
        rel.body = "中文发布说明".repeat(200);
        assert!(build_update_check("0.2.5", rel).is_ok());
    }
}
