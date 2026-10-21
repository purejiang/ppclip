pub mod autostart;
pub mod ballhub;
pub mod capture;
pub mod clipboard;
pub mod commands;
pub mod github;
pub mod logger;
pub mod menu;
pub mod models;
pub mod pins;
pub mod settings;
pub mod storage;
pub mod tray;
pub mod util;
pub mod viewer;

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

use models::AppSettings;
use tauri::{Emitter, Manager};

pub use capture::CaptureState;
pub use clipboard::{SelfWrite, WatcherHandle};

/// 面板显示 / 隐藏时广播给悬浮球（payload 是 bool）：
/// 标签上的「新条目」角标靠它决定是计数还是归零
pub const PANEL_VISIBLE_EVENT: &str = "panel:visible";

/// 面板可见性变化后通知悬浮球
fn notify_panel_visibility(app: &tauri::AppHandle) {
    let visible = app
        .get_webview_window("main")
        .and_then(|win| win.is_visible().ok())
        .unwrap_or(false);
    let _ = app.emit_to("ball", PANEL_VISIBLE_EVENT, visible);
}

/// 应用数据目录（Windows：`%APPDATA%\com.ppclip.app`）
pub struct DataDir(pub PathBuf);

/// 设置的内存副本，与 `settings.json` 保持同步
pub struct SettingsState(pub Mutex<AppSettings>);

/// 显示历史面板并聚焦（悬浮球菜单 / 第二实例唤醒都用它）
pub fn show_panel(app: &tauri::AppHandle) {
    // 面板一露面就收回轮盘：面板是后来的置顶窗，轮盘要是还撑着，
    // 那圈按钮只会从面板边缘探出来，而且面板是半透明玻璃、撑开的盘会透出来。
    if let Err(err) = ballhub::close_wheel_now(app) {
        log::warn!("显示面板前收回轮盘失败: {err}");
    }
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
    }
    notify_panel_visibility(app);
    // 没开桌面置顶时，点到别的程序就收起（看门狗只在面板露着的时候活着）
    watch_panel_blur(app);
}

/// 隐藏历史面板
pub fn hide_panel(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.hide();
    }
    notify_panel_visibility(app);
}

/// 面板「桌面置顶」：把窗口的 `always_on_top` 跟设置对齐。
///
/// 只改窗口属性，不碰 settings —— 落盘由 `commands::set_panel_pinned` 负责，
/// 启动时由 `setup` 用已加载的设置调这里。
pub fn apply_panel_pinned(app: &tauri::AppHandle, pinned: bool) {
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    match win.set_always_on_top(pinned) {
        Ok(()) => log::info!("面板桌面置顶 → {}", if pinned { "开" } else { "关" }),
        Err(err) => log::warn!("设置面板桌面置顶失败: {err}"),
    }
}

/// 取某个进程的可执行文件全路径（拿不到就 None）
fn process_image_path(pid: u32) -> Option<String> {
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

    unsafe extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
        fn QueryFullProcessImageNameW(
            process: isize,
            flags: u32,
            name: *mut u16,
            size: *mut u32,
        ) -> i32;
        fn CloseHandle(handle: isize) -> i32;
    }

    if pid == 0 {
        return None;
    }
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle == 0 {
            return None;
        }
        let mut buf = [0u16; 512];
        let mut size = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut size);
        CloseHandle(handle);
        if ok == 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buf[..size as usize]))
    }
}

/// 前台窗口是不是**别的程序**的（= 用户真的去看别的程序 / 点了桌面）。
///
/// 面板失焦的两种含义：
/// ① 用户点了别的程序、或者点了桌面 → 该收起；
/// ② 焦点只是在我们自己的窗口之间转（悬浮球 / 贴图 / 查看 / overlay，以及启动时的抖动）
///    → 面板要留着（稿子里查看窗口就停在面板旁边，一条条往下翻）。
///
/// 判据：前台窗口的进程 **既不是本进程、也不是同一个 exe 的另一个实例**。
/// 后一条是必须的：用户再点一次图标唤面板时，第二个 `ppclip.exe` 会短暂持有前台，
/// 只比 pid 的话它长得像「别人」，刚露面的面板会立刻被关掉（实测踩过）。
/// 没有前台窗口（`hwnd == 0`，切桌面 / 刚藏完窗）一律当成「自己」，不乱关。
fn foreground_is_foreign() -> bool {
    unsafe extern "system" {
        fn GetForegroundWindow() -> isize;
        fn GetWindowThreadProcessId(hwnd: isize, lpdw_process_id: *mut u32) -> u32;
    }

    let pid = unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd == 0 {
            return false;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        pid
    };

    if pid == 0 || pid == std::process::id() {
        return false;
    }

    match (process_image_path(pid), std::env::current_exe()) {
        (Some(foreground_exe), Ok(own_exe)) => {
            foreground_exe.to_lowercase() != own_exe.to_string_lossy().to_lowercase()
        }
        // 拿不到路径就别乱关（宁可不关，也不要「面板自己消失」）
        _ => false,
    }
}

/// 面板此刻是不是「桌面置顶」（读内存里的设置，不读盘）
fn panel_pinned(app: &tauri::AppHandle) -> bool {
    app.try_state::<SettingsState>()
        .map(|state| {
            state
                .0
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .panel_pinned
        })
        .unwrap_or(false)
}

/// 面板「点击外部就收起」的看门狗（只在面板露着的时候活着）。
///
/// **为什么不用 `WindowEvent::Focused(false)`**：实测（`--smoke-panel`）把前台窗口换成桌面后，
/// 面板明明已经不是前台窗口，`is_focused()` 也翻了，**却收不到任何 Focused 事件** ——
/// 靠事件驱动会出现「点了别处面板不关」，而这正是用户要的行为。
/// 所以这里直接轮询 Win32 的前台窗口，判据只有一个：**前台窗口属于别的进程**
/// （`foreground_is_foreign`，自家窗口之间怎么跳都不算）。
///
/// `armed` 的用处：面板如果是在「别的程序当前台」时被唤出来的（Windows 的前台锁定会拒掉
/// `set_focus`），第一拍就会看到「前台是别人」，不能据此当成「用户点了外部」立刻藏掉。
/// 只有这一轮里真的见过「前台是自己」才算武装完成。
const PANEL_BLUR_TICK_MS: u64 = 200;
/// 连续多少拍「前台是别的程序」才真的收起（200ms × 2 = 400ms 的去抖）
const HIDE_AFTER_FOREIGN_TICKS: u32 = 2;
static PANEL_BLUR_WATCH: AtomicBool = AtomicBool::new(false);

/// 看门狗单拍的结论
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlurTick {
    /// 什么都不做，继续看
    Idle,
    /// 收起面板，然后下班
    Hide,
    /// 面板已经不在（或看门狗没事干了），下班
    Stop,
}

/// 失焦看门狗的一拍。抽成纯函数是为了能单测 —— 这里的坑都是实测踩出来的：
/// 「刚唤出来就被关」（前台还是别人时不武装）、「点了别处不关」（武装后前台变别人就关）、
/// 「焦点抖一下就关」（连续 `HIDE_AFTER_FOREIGN_TICKS` 拍才算数）。
fn blur_tick(
    visible: bool,
    pinned: bool,
    foreground_foreign: bool,
    armed: &mut bool,
    foreign_ticks: &mut u32,
) -> BlurTick {
    if !visible {
        *armed = false;
        *foreign_ticks = 0;
        return BlurTick::Stop;
    }
    if pinned {
        // 置顶 = 常驻，永不自动关；顺手清掉武装状态，
        // 免得关掉置顶的那一拍把「很久以前拿过焦点」当成刚失焦
        *armed = false;
        *foreign_ticks = 0;
        return BlurTick::Idle;
    }
    if !foreground_foreign {
        // 前台还是自己（面板 / 悬浮球 / 贴图 / 查看…）：这一轮算「武装完成」
        *armed = true;
        *foreign_ticks = 0;
        return BlurTick::Idle;
    }

    // 前台是别人：要连续几拍都这样才收 —— 进程退出、窗口销毁这些瞬间也会短暂
    // 出现「前台是别人」，一拍就关会让面板神经过敏
    *foreign_ticks += 1;
    if *armed && *foreign_ticks >= HIDE_AFTER_FOREIGN_TICKS {
        *armed = false;
        *foreign_ticks = 0;
        return BlurTick::Hide;
    }
    BlurTick::Idle
}

fn panel_blur_watch_on() -> bool {
    PANEL_BLUR_WATCH
        .compare_exchange(
            false,
            true,
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
        )
        .is_ok()
}

fn panel_blur_watch_off() {
    PANEL_BLUR_WATCH.store(false, std::sync::atomic::Ordering::SeqCst);
}

fn watch_panel_blur(app: &tauri::AppHandle) {
    if !panel_blur_watch_on() {
        return; // 已经有人看着了
    }

    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("ppclip-panel-blur".into())
        .spawn(move || {
            let mut armed = false;
            let mut foreign_ticks = 0u32;

            loop {
                std::thread::sleep(std::time::Duration::from_millis(PANEL_BLUR_TICK_MS));

                let visible = app
                    .get_webview_window("main")
                    .and_then(|win| win.is_visible().ok())
                    .unwrap_or(false);

                match blur_tick(
                    visible,
                    panel_pinned(&app),
                    foreground_is_foreign(),
                    &mut armed,
                    &mut foreign_ticks,
                ) {
                    BlurTick::Idle => {}
                    BlurTick::Stop => {
                        // 面板已经收起了（Esc / 点悬浮球 / 复制后自动收）：看门狗跟着下班，
                        // 下次 show 再起一条，不做常驻轮询
                        panel_blur_watch_off();
                        return;
                    }
                    BlurTick::Hide => {
                        log::info!("面板失焦（前台已切到别的程序）且未开启桌面置顶，自动收起");
                        panel_blur_watch_off();
                        hide_panel(&app);
                        return;
                    }
                }
            }
        });

    if spawned.is_err() {
        panel_blur_watch_off();
        log::warn!("启动面板失焦看门狗失败");
    }
}

/// 面板显示 / 隐藏切换
pub fn toggle_panel(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        if win.is_visible().unwrap_or(false) {
            let _ = win.hide();
        } else {
            // 和 show_panel 一样：面板露面就先收回轮盘
            if let Err(err) = ballhub::close_wheel_now(app) {
                log::warn!("显示面板前收回轮盘失败: {err}");
            }
            let _ = win.show();
            let _ = win.set_focus();
            // 和 show_panel 一样：面板露面就要有人盯着「点外部」这件事
            watch_panel_blur(app);
        }
    }
    notify_panel_visibility(app);
}

/// 把「会创建 / 销毁窗口」的活儿挪到独立线程执行，并等它跑完。
///
/// Windows 上 WebView2 **不允许在自己的回调里创建 widget**：`#[tauri::command]`
/// 的同步实现、原生菜单回调，都跑在 WebView2 的回调栈上，直接
/// `WebviewWindowBuilder::build()` 会永远等不到控制器创建完成的回调（死锁）。
/// Tauri 自己在 `WebviewWindowBuilder` 的文档里把这条列为已知问题（wry#583），
/// 建议「用 async 命令或独立线程」。
///
/// async 命令会被 Tauri 立刻挂到异步运行时上，IPC 回调当场返回，于是窗口创建
/// 发生在事件循环里而不是回调栈里。注意：这里之所以用 `spawn_blocking` 而不是
/// `spawn`，是因为截图路径本身是阻塞的（抓屏、写 PNG、写剪贴板）。
pub(crate) async fn run_off_main<T, F>(app: tauri::AppHandle, task: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&tauri::AppHandle) -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || task(&app))
        .await
        .map_err(|e| format!("后台任务异常结束: {e}"))?
}

/// 目录能不能真写：建目录 + 写一个探测文件（只有真写成功才算数）
fn is_writable(dir: &std::path::Path) -> bool {
    if std::fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(".ppclip-write-probe");
    let ok = std::fs::write(&probe, b"ppclip").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

/// 在 Tauri 建窗**之前**把数据目录定下来。
///
/// 窗口（以及 WebView2 自己的用户数据目录）是在 Tauri 的 setup 阶段、先于我们注册的
/// 闭包创建出来的，所以这件事不能等到闭包里再做 —— 否则受限环境会直接
/// `Failed to setup app: 拒绝访问`。
///
/// 正常走 `%APPDATA%\<identifier>`（与 tauri.conf.json 里 asset protocol 的
/// `$APPDATA/**` 对得上）；写不了（沙箱 / 受限企业环境）时依次回退到
/// `%LOCALAPPDATA%\<identifier>`、`<exe 同级>/ppclip-data`。
/// 返回 `(目录, 是否走了覆盖或回退)`。
fn resolve_data_dir_early(identifier: &str) -> (PathBuf, bool) {
    if let Ok(custom) = std::env::var("PPCLIP_DATA_DIR") {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            return (PathBuf::from(trimmed), true);
        }
    }

    let standard = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .map(|base| base.join(identifier));
    if let Some(dir) = &standard {
        if is_writable(dir) {
            return (dir.clone(), false);
        }
    }

    if let Some(base) = std::env::var_os("LOCALAPPDATA") {
        let dir = PathBuf::from(base).join(identifier);
        if is_writable(&dir) {
            return (dir, true);
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let dir = parent.join("ppclip-data");
            if is_writable(&dir) {
                return (dir, true);
            }
        }
    }

    // 都写不了：仍旧用标准位置，让 setup 里的报错把原因讲清楚
    (standard.unwrap_or_else(|| PathBuf::from(".")), false)
}

/// 解析数据目录。
///
/// 默认用 Tauri 的 `app_data_dir()`（Windows：`%APPDATA%\<identifier>`）；
/// `resolve_data_dir_early` 决定走覆盖/回退时会把 `PPCLIP_DATA_DIR` 设好，
/// 这里优先读它
fn resolve_data_dir(app: &tauri::AppHandle) -> std::io::Result<PathBuf> {
    if let Ok(custom) = std::env::var("PPCLIP_DATA_DIR") {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }

    app.path()
        .app_data_dir()
        .map_err(|e| std::io::Error::other(e.to_string()))
}

/// `--smoke-wheel` 的实现：无人值守验证轮盘链路。
///
/// 四条断言（都是「窗口是矩形裁剪框」这个前提下的硬要求）：
/// 1. 展开后**轮盘整块在显示器内** —— 否则那一侧的卫星按钮点不到；
/// 2. 展开后**盘心 = 窗口中心** —— CSS 里中心盘就居中在盘里，盘心一偏中心球就贴边；
/// 3. 收起后**窗口矩形一比一还原** —— 位置只由 anchor 推导，来回一次不许漂；
/// 4. 全程 **anchor 不变** —— 这是「球不跳」的定义。
fn smoke_wheel(app: &tauri::AppHandle) -> i32 {
    use tauri::Manager;

    let Some(ball) = app.get_webview_window("ball") else {
        log::error!("--smoke-wheel 失败：没有 ball 窗口");
        return 1;
    };
    let scale = ball.scale_factor().unwrap_or(1.0);

    // 先切成常规态：贴边胶囊上本来就不许展开轮盘，基线也要在切换之后再量
    if let Err(err) = ballhub::set_form(app, false) {
        log::error!("--smoke-wheel 失败：切常规态 {err}");
        return 1;
    }
    let collapsed_size = ball.outer_size().unwrap_or_default();
    let collapsed_pos = ball.outer_position().unwrap_or_default();
    let before = ballhub::snapshot(app);
    log::info!(
        "--smoke-wheel 起手：球窗口 {}×{} @({}, {})，锚点 ({}, {})，缩放 {:.2}",
        collapsed_size.width,
        collapsed_size.height,
        collapsed_pos.x,
        collapsed_pos.y,
        before.anchor_x,
        before.anchor_y,
        scale
    );

    let open = match ballhub::set_wheel(app, true) {
        Ok(snap) => snap,
        Err(err) => {
            log::error!("--smoke-wheel 失败：展开轮盘 {err}");
            return 1;
        }
    };
    std::thread::sleep(std::time::Duration::from_millis(250));

    let mut failures: Vec<String> = Vec::new();
    let wheel_size = ball.outer_size().unwrap_or_default();
    let wheel_pos = ball.outer_position().unwrap_or_default();
    let disc = ballhub::geom::to_physical(ballhub::geom::WHEEL_LOGICAL, scale);
    let disc_x = wheel_pos.x + (wheel_size.width as i32 - disc) / 2;
    let disc_y = wheel_pos.y + (wheel_size.height as i32 - disc) / 2;
    let disc_cx = disc_x + disc / 2;
    let disc_cy = disc_y + disc / 2;
    log::info!(
        "--smoke-wheel 展开：窗口 {}×{} @({}, {})，盘 {} @({}, {})，盘心 ({}, {}) 偏离锚点 {:+}",
        wheel_size.width,
        wheel_size.height,
        wheel_pos.x,
        wheel_pos.y,
        disc,
        disc_x,
        disc_y,
        disc_cx,
        disc_cy,
        disc_cx - open.anchor_x
    );

    if !open.wheel_open {
        failures.push("展开后 wheel_open 仍是 false".into());
    }

    let monitors: Vec<ballhub::geom::MonitorRect> = app
        .available_monitors()
        .map(|list| {
            list.iter()
                .map(|m| {
                    let p = m.position();
                    let s = m.size();
                    ballhub::geom::MonitorRect::new(p.x, p.y, s.width as i32, s.height as i32)
                })
                .collect()
        })
        .unwrap_or_default();
    let anchor = ballhub::geom::Anchor::new(open.anchor_x, open.anchor_y);
    match ballhub::geom::monitor_at(anchor, &monitors) {
        Some(mon) => {
            if disc_x < mon.x
                || disc_y < mon.y
                || disc_x + disc > mon.right()
                || disc_y + disc > mon.bottom()
            {
                failures.push(format!(
                    "轮盘盘面超出显示器：盘 ({}, {}) {}，屏幕 ({}, {}) {}×{}",
                    disc_x, disc_y, disc, mon.x, mon.y, mon.w, mon.h
                ));
            }
        }
        None => failures.push("锚点不在任何显示器上".into()),
    }

    // 盘必须居中在窗口里：CSS 的 `.ball-core` 就是盘心，盘心偏了中心球就贴到盘边上
    let win_cx = wheel_pos.x + wheel_size.width as i32 / 2;
    let win_cy = wheel_pos.y + wheel_size.height as i32 / 2;
    if (win_cx - disc_cx).abs() > 1 || (win_cy - disc_cy).abs() > 1 {
        failures.push(format!(
            "盘没居中在窗口里：盘心 ({}, {}) vs 窗口中心 ({}, {})",
            disc_cx, disc_cy, win_cx, win_cy
        ));
    }
    if anchor.x != before.anchor_x || anchor.y != before.anchor_y {
        failures.push("展开前后锚点变了（球会跳）".into());
    }

    // 收起：几何必须一比一还原，锚点一分不动
    let closed = match ballhub::set_wheel(app, false) {
        Ok(snap) => snap,
        Err(err) => {
            log::error!("--smoke-wheel 失败：收起轮盘 {err}");
            return 1;
        }
    };
    std::thread::sleep(std::time::Duration::from_millis(250));
    let back_size = ball.outer_size().unwrap_or_default();
    let back_pos = ball.outer_position().unwrap_or_default();
    log::info!(
        "--smoke-wheel 收起：窗口 {}×{} @({}, {})（起手是 {}×{} @({}, {})）",
        back_size.width,
        back_size.height,
        back_pos.x,
        back_pos.y,
        collapsed_size.width,
        collapsed_size.height,
        collapsed_pos.x,
        collapsed_pos.y
    );
    if back_size != collapsed_size {
        failures.push(format!(
            "收起后尺寸没还原：{}×{} vs {}×{}",
            back_size.width, back_size.height, collapsed_size.width, collapsed_size.height
        ));
    }
    if (back_pos.x - collapsed_pos.x).abs() > 1 || (back_pos.y - collapsed_pos.y).abs() > 1 {
        failures.push(format!(
            "收起后位置漂了：({}, {}) vs ({}, {})",
            back_pos.x, back_pos.y, collapsed_pos.x, collapsed_pos.y
        ));
    }
    if closed.wheel_open {
        failures.push("收起后 wheel_open 仍是 true".into());
    }
    if closed.anchor_x != before.anchor_x || closed.anchor_y != before.anchor_y {
        failures.push("收起前后锚点变了".into());
    }

    if failures.is_empty() {
        log::info!("--smoke-wheel 完成：全部通过");
        0
    } else {
        for item in &failures {
            log::error!("--smoke-wheel 失败：{item}");
        }
        1
    }
}

/// 冒烟用：当前光标的物理坐标（拿不到返回 None）
fn smoke_cursor_pos() -> Option<(i32, i32)> {
    #[repr(C)]
    struct Pt {
        x: i32,
        y: i32,
    }
    unsafe extern "system" {
        fn GetCursorPos(lp_point: *mut Pt) -> i32;
    }
    let mut pt = Pt { x: 0, y: 0 };
    if (unsafe { GetCursorPos(&mut pt) }) != 0 {
        Some((pt.x, pt.y))
    } else {
        None
    }
}

/// 冒烟用：把光标挪到物理坐标（失败返回 false）
fn smoke_move_cursor(x: i32, y: i32) -> bool {
    unsafe extern "system" {
        fn SetCursorPos(x: i32, y: i32) -> i32;
    }
    (unsafe { SetCursorPos(x, y) }) != 0
}

/// `--smoke-dock` 的实现：验证三条与贴边相关的链路。
///
/// 1. **拖动松手 → 吸附判定**：走的是和真拖动一样的两步（`begin_drag` 起看门狗 →
///    看门狗发现左键是松的 → 停稳后调 `finish_drag`）。冒烟时左键本来就是松的，所以它
///    立刻就会收尾 —— 正好覆盖「拖动结束时到底有没有判定」这段最容易丢的链路
///    （真机上出过：拖到边上不贴边）。
/// 2. **悬停唤出 → 离屏幕边留余量**：球右沿必须是 `right - inset`，整块在屏内。
/// 3. **鼠标离开 → 自动贴回**：把真实光标挪到屏幕左侧，等看门狗把球滑回胶囊，
///    跑完恢复光标。sandbox 里挪不动光标就记 warn 跳过这条断言（跟 `--smoke-panel`
///    对 `SetForegroundWindow` 的降级一致）。
fn smoke_dock(app: &tauri::AppHandle) -> i32 {
    use tauri::Manager;

    let Some(ball) = app.get_webview_window("ball") else {
        log::error!("--smoke-dock 失败：没有 ball 窗口");
        return 1;
    };
    let scale = ball.scale_factor().unwrap_or(1.0);
    let Some(mon) = ball.current_monitor().ok().flatten() else {
        log::error!("--smoke-dock 失败：拿不到显示器");
        return 1;
    };
    let origin = mon.position();
    let msize = mon.size();
    let right = origin.x + msize.width as i32;
    let bottom = origin.y + msize.height as i32;
    let mid_y = origin.y + msize.height as i32 / 2;
    let win = ball.outer_size().unwrap_or_default();
    let orb_half = ballhub::geom::shape_size(ballhub::geom::BallForm::Orb, scale).0 / 2;
    let tab_half = ballhub::geom::shape_size(ballhub::geom::BallForm::Tab, scale).0 / 2;
    let orb_w = ballhub::geom::shape_size(ballhub::geom::BallForm::Orb, scale).0;
    let tab_w = ballhub::geom::shape_size(ballhub::geom::BallForm::Tab, scale).0;
    let pad_x = ballhub::geom::to_physical(ballhub::geom::SHAPE_PAD_X_LOGICAL, scale);
    let inset = ballhub::geom::to_physical(ballhub::geom::UNDOCK_EDGE_INSET_LOGICAL, scale);

    let mut failures: Vec<String> = Vec::new();
    // 收起窗必须已经收窄：否则常规球四周会有一大片透明置顶窗挡住点击
    if win.width as i32 > orb_w + 2 * pad_x + 2 {
        failures.push(format!(
            "收起态窗口太宽：{} 物理像素（球 {} + 两侧余量 {} ≈ {}）",
            win.width,
            orb_w,
            pad_x,
            orb_w + 2 * pad_x
        ));
    }

    /// 模拟一次「拖到 anchor 处松手」：摆位 → 起看门狗 → 等它收尾
    fn drop_and_dock(
        ball: &tauri::WebviewWindow,
        app: &tauri::AppHandle,
        win: tauri::PhysicalSize<u32>,
        label: &str,
        anchor: ballhub::geom::Anchor,
        failures: &mut Vec<String>,
    ) {
        let target = ballhub::geom::window_pos(anchor, win);
        if let Err(err) = ball.set_position(target) {
            failures.push(format!("{label}：摆位失败 {err}"));
            return;
        }
        log::info!(
            "--smoke-dock {label}：模拟落点锚点 ({}, {})，窗口 @({}, {})",
            anchor.x,
            anchor.y,
            target.x,
            target.y
        );
        // 真链路：起看门狗 → 左键是松的 → 停 120ms → finish_drag → 120ms 滑入动画
        ballhub::begin_drag(app);
        std::thread::sleep(std::time::Duration::from_millis(500));
        let snap = ballhub::snapshot(app);
        let pos = ball.outer_position().unwrap_or_default();
        log::info!(
            "--smoke-dock {label}：形态 {}，锚点 ({}, {})，窗口 @({}, {})",
            snap.form.name(),
            snap.anchor_x,
            snap.anchor_y,
            pos.x,
            pos.y
        );
    }

    // ① 离右边 10px 松手 → 必须吸成胶囊，而且胶囊贴住屏幕边
    if let Err(err) = ballhub::set_form(app, false) {
        log::error!("--smoke-dock 失败：切常规态 {err}");
        return 1;
    }
    drop_and_dock(
        &ball,
        app,
        win,
        "近边",
        ballhub::geom::Anchor::new(right - 10 - orb_half, mid_y),
        &mut failures,
    );
    let snap = ballhub::snapshot(app);
    if snap.form != ballhub::geom::BallForm::Tab {
        failures.push(format!("近边松手没吸成胶囊（形态 {}）", snap.form.name()));
    }
    let expect_x = right - tab_half;
    if (snap.anchor_x - expect_x).abs() > 1 {
        failures.push(format!(
            "胶囊没贴住右边：锚点 {} vs 期望 {}",
            snap.anchor_x, expect_x
        ));
    }
    // 滑入动画必须真的落到目标位置（不能停在半路）；贴边时窗口主体必须在屏幕外，
    // 屏幕上只剩「胶囊 + SHAPE_PAD_X」——这就是贴边态不再挡住下层点击的机器保证。
    let expect_pos = ballhub::geom::window_pos_for(
        ballhub::geom::BallForm::Tab,
        ballhub::geom::DockSide::Right,
        ballhub::geom::Anchor::new(snap.anchor_x, snap.anchor_y),
        win,
        scale,
    );
    let pos_after = ball.outer_position().unwrap_or_default();
    if expect_pos != pos_after {
        failures.push(format!(
            "贴边后窗口没落在目标位置：@({}, {}) vs @({}, {})",
            pos_after.x, pos_after.y, expect_pos.x, expect_pos.y
        ));
    }
    let on_screen = right - pos_after.x;
    if on_screen > tab_w + pad_x + 2 {
        failures.push(format!(
            "贴边后屏幕内还占着 {on_screen} 物理像素（应为胶囊 {tab_w} + 余量 {pad_x} ≈ {}）",
            tab_w + pad_x
        ));
    }
    if pos_after.x + win.width as i32 <= right {
        failures.push(format!(
            "贴边后窗口没有探出屏幕：窗口 @({}, {})，右沿 {}，屏幕右沿 {right}",
            pos_after.x,
            pos_after.y,
            pos_after.x + win.width as i32
        ));
    }

    // ② 悬停唤出常规球：球要离屏幕边留 6px（角标 / hover 描边 / 投影不被切）
    if let Err(err) = ballhub::set_form(app, false) {
        failures.push(format!("切常规态失败 {err}"));
    } else {
        let orb = ballhub::snapshot(app);
        let size = ball.outer_size().unwrap_or_default();
        let pos = ball.outer_position().unwrap_or_default();
        log::info!(
            "--smoke-dock 悬停唤出：形态 {}，锚点 ({}, {})，窗口 {}×{} @({}, {})",
            orb.form.name(),
            orb.anchor_x,
            orb.anchor_y,
            size.width,
            size.height,
            pos.x,
            pos.y
        );
        if orb.form != ballhub::geom::BallForm::Orb {
            failures.push(format!("悬停唤出后形态不是 orb（{}）", orb.form.name()));
        }
        let orb_right = orb.anchor_x + orb_half;
        if (orb_right - (right - inset)).abs() > 1 {
            failures.push(format!(
                "常规球没离右边留 {inset}px：右沿 {} vs 期望 {}",
                orb_right,
                right - inset
            ));
        }
        if orb.anchor_x - orb_half < origin.x || orb_right > right {
            failures.push(format!("常规球越出屏幕左右：锚点 {}", orb.anchor_x));
        }
        if orb.anchor_y - orb_half < origin.y || orb.anchor_y + orb_half > bottom {
            failures.push(format!("常规球越出屏幕上下：锚点 {}", orb.anchor_y));
        }
        let expect_pos = ballhub::geom::window_pos(
            ballhub::geom::Anchor::new(orb.anchor_x, orb.anchor_y),
            size,
        );
        if expect_pos != pos {
            failures.push(format!(
                "悬停唤出后窗口不在锚点推导的位置：@({}, {}) vs @({}, {})",
                pos.x, pos.y, expect_pos.x, expect_pos.y
            ));
        }

        // ③ 鼠标离开球 → 看门狗自动滑回原侧胶囊
        let saved_cursor = smoke_cursor_pos();
        let away_x = origin.x + msize.width as i32 / 4;
        if smoke_move_cursor(away_x, mid_y) {
            ballhub::arm_redock(app);
            std::thread::sleep(std::time::Duration::from_millis(1500));
            let back = ballhub::snapshot(app);
            let back_pos = ball.outer_position().unwrap_or_default();
            log::info!(
                "--smoke-dock 自动贴回：形态 {}，锚点 ({}, {})，窗口 @({}, {})",
                back.form.name(),
                back.anchor_x,
                back.anchor_y,
                back_pos.x,
                back_pos.y
            );
            if back.form != ballhub::geom::BallForm::Tab {
                failures.push("鼠标离开后没有自动贴回胶囊".into());
            } else {
                let expect_x = right - tab_half;
                if (back.anchor_x - expect_x).abs() > 1 {
                    failures.push(format!(
                        "自动贴回的胶囊没贴住右边：锚点 {} vs 期望 {}",
                        back.anchor_x, expect_x
                    ));
                }
                let expect_pos = ballhub::geom::window_pos_for(
                    ballhub::geom::BallForm::Tab,
                    ballhub::geom::DockSide::Right,
                    ballhub::geom::Anchor::new(back.anchor_x, back.anchor_y),
                    win,
                    scale,
                );
                if expect_pos != back_pos {
                    failures.push(format!(
                        "自动贴回的窗口没落在目标位置：@({}, {}) vs @({}, {})",
                        back_pos.x, back_pos.y, expect_pos.x, expect_pos.y
                    ));
                } else if right - back_pos.x > tab_w + pad_x + 2 {
                    failures.push(format!(
                        "自动贴回后屏幕内还占着 {} 物理像素（应为 ≤ {}）",
                        right - back_pos.x,
                        tab_w + pad_x
                    ));
                }
            }
        } else {
            log::warn!("--smoke-dock：挪不动光标，跳过自动贴回断言（环境限制）");
        }
        if let Some((x, y)) = saved_cursor {
            if !smoke_move_cursor(x, y) {
                log::warn!("--smoke-dock：光标没恢复回 ({x}, {y})");
            }
        }
    }

    // ④ 屏幕中间松手 → 不该吸（回差带之外）
    if let Err(err) = ballhub::set_form(app, false) {
        failures.push(format!("切常规态失败 {err}"));
    }
    drop_and_dock(
        &ball,
        app,
        win,
        "离边远",
        ballhub::geom::Anchor::new(origin.x + msize.width as i32 / 2, mid_y),
        &mut failures,
    );
    if ballhub::snapshot(app).form == ballhub::geom::BallForm::Tab {
        failures.push("屏幕中间松手也吸成了胶囊".into());
    }

    if failures.is_empty() {
        log::info!("--smoke-dock 完成：全部通过");
        0
    } else {
        for item in &failures {
            log::error!("--smoke-dock 失败：{item}");
        }
        1
    }
}

/// 把前台窗口切成桌面（模拟「用户点了别的地方」）。
///
/// 沙箱里 PowerShell 的 `AppActivate` / COM 都被拦了，冒烟只能自己动手：
/// 本进程此刻是前台，`SetForegroundWindow` 才允许成功。
fn activate_desktop() -> bool {
    unsafe extern "system" {
        fn FindWindowW(class: *const u16, title: *const u16) -> isize;
        fn SetForegroundWindow(hwnd: isize) -> i32;
    }

    let class: Vec<u16> = "Progman\0".encode_utf16().collect();
    unsafe {
        let hwnd = FindWindowW(class.as_ptr(), std::ptr::null());
        hwnd != 0 && SetForegroundWindow(hwnd) != 0
    }
}

/// `--smoke-panel` 的实现：验证面板「桌面置顶 / 失焦自动收起」。
///
/// 三段，全走真链路（`show_panel` + `WindowEvent::Focused` + `foreground_is_foreign`）：
/// ① 未置顶：显示面板 → 前台切桌面 → 必须**自动收起**；
/// ② 置顶：显示面板 → 前台切桌面 → 必须**还在**；
/// ③ 未置顶：回到起点（收起、恢复设置的内存值，不动盘）。
///
/// ⚠️ 它会短暂抢走前台焦点（这是这条链路唯一的验证方式），跑完自动退出。
fn smoke_panel(app: &tauri::AppHandle) -> i32 {
    use tauri::Manager;

    let mut failures: Vec<String> = Vec::new();
    let visible = || {
        app.get_webview_window("main")
            .and_then(|win| win.is_visible().ok())
            .unwrap_or(false)
    };
    let sleep = |ms: u64| std::thread::sleep(std::time::Duration::from_millis(ms));

    /// 只改内存里的置顶设置（冒烟不落盘，别把用户的设置写脏）
    fn set_pinned_memory(app: &tauri::AppHandle, pinned: bool) {
        if let Some(state) = app.try_state::<SettingsState>() {
            state.0.lock().unwrap_or_else(|e| e.into_inner()).panel_pinned = pinned;
        }
        apply_panel_pinned(app, pinned);
    }

    set_pinned_memory(app, false);
    show_panel(app);
    let focused_now = app
        .get_webview_window("main")
        .and_then(|win| win.is_focused().ok())
        .unwrap_or(false);
    log::info!("--smoke-panel ①：show_panel 之后立刻 is_focused={focused_now}");
    sleep(700);
    let focused_later = app
        .get_webview_window("main")
        .and_then(|win| win.is_focused().ok())
        .unwrap_or(false);
    log::info!(
        "--smoke-panel ①：700ms 后 is_focused={focused_later}，前台是自己的={}",
        !foreground_is_foreign()
    );
    match visible() {
        true => log::info!("--smoke-panel ①：面板已显示"),
        false => failures.push("show_panel 之后面板不可见".into()),
    }

    if !activate_desktop() {
        log::warn!("--smoke-panel：前台窗口切不走，跳过失焦相关断言（环境限制）");
    } else {
        sleep(900);
        let foreground_foreign = foreground_is_foreign();
        log::info!(
            "--smoke-panel ①：切到桌面后 前台是别家进程={foreground_foreign}，面板可见={}",
            visible()
        );
        if !foreground_foreign {
            log::warn!("--smoke-panel ①：前台没能切成桌面，跳过断言");
        } else if visible() {
            failures.push("未置顶时切到桌面，面板没有自动收起".into());
        }

        // ② 置顶
        set_pinned_memory(app, true);
        show_panel(app);
        sleep(700);
        if !activate_desktop() {
            log::warn!("--smoke-panel ②：前台窗口切不走，跳过断言");
        } else {
            sleep(900);
            log::info!("--smoke-panel ②：置顶 + 切到桌面，面板可见={}", visible());
            if !visible() {
                failures.push("置顶状态下切到桌面，面板不该收起".into());
            }
        }
    }

    // ③ 第二个实例唤出面板：那一刻前台是**另一个 ppclip.exe 进程**，
    //    不能因此判定成「用户走开了」把面板立刻关掉（真机上出现过）
    set_pinned_memory(app, false);
    hide_panel(app);
    sleep(300);
    match std::env::current_exe() {
        Ok(exe) => {
            if let Err(err) = std::process::Command::new(exe).spawn() {
                log::warn!("--smoke-panel ③：起第二个实例失败 {err}");
            }
        }
        Err(err) => log::warn!("--smoke-panel ③：拿不到自己的路径 {err}"),
    }
    sleep(1800);
    log::info!("--smoke-panel ③：第二个实例唤起后 面板可见={}", visible());
    if !visible() {
        failures.push("第二个实例唤出面板后，面板被自动收起了".into());
    }

    // ④ 回到起点
    set_pinned_memory(app, false);
    hide_panel(app);

    if failures.is_empty() {
        log::info!("--smoke-panel 完成：全部通过");
        0
    } else {
        for item in &failures {
            log::error!("--smoke-panel 失败：{item}");
        }
        1
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 诊断入口：只跑更新检测，不建窗口、不碰 WebView2。
    // 调试「检查更新」链路时不用启动整个应用：`ppclip.exe --check-update`。
    if std::env::args().any(|arg| arg == "--check-update") {
        match github::check_latest(env!("CARGO_PKG_VERSION")) {
            Ok(check) => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&check).unwrap_or_else(|_| "{}".to_string())
                );
                std::process::exit(0);
            }
            Err(err) => {
                eprintln!("检查更新失败：{err}");
                std::process::exit(1);
            }
        }
    }

    let context: tauri::Context<tauri::Wry> = tauri::generate_context!();

    // 必须早于 Builder：窗口与 WebView2 的目录要在这时就定好
    let (early_dir, custom) = resolve_data_dir_early(&context.config().identifier);
    if custom {
        std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", early_dir.join("webview2"));
        std::env::set_var("PPCLIP_DATA_DIR", &early_dir);
    }

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            log::info!("检测到第二个实例，唤醒已有悬浮球与面板");
            let _ = ballhub::close_wheel_now(app);
            if let Some(ball) = app.get_webview_window("ball") {
                let _ = ball.show();
            }
            show_panel(app);
        }))
        .on_menu_event(|app, event| menu::handle_menu_event(app, event.id().as_ref()))
        .on_window_event(|window, event| match event {
            // 面板和悬浮球只有隐藏、没有关闭：没有托盘的情况下误关就没有入口了
            tauri::WindowEvent::CloseRequested { api, .. } => {
                let label = window.label();
                if label == "main" || label == "ball" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
            // 面板「点击外部关闭」**不走 WindowEvent::Focused**：实测前台切到别的程序后
            // 面板收不到任何 Focused 事件（is_focused() 都翻了，事件不来），
            // 所以改由 `watch_panel_blur` 轮询前台窗口 —— 见那个函数的注释。
            // DPI / 跨屏：物理尺寸变了（WM_DPICHANGED），几何得按实测值重算
            tauri::WindowEvent::ScaleFactorChanged { .. } => {
                let label = window.label().to_string();
                if label == "ball" {
                    let app = window.app_handle().clone();
                    // 不能在自己的事件回调里改窗口几何（会重入），丢到别处跑
                    std::thread::spawn(move || {
                        if let Err(err) = ballhub::on_scale_changed(&app, &label) {
                            log::warn!("DPI 变化后重算悬浮球几何失败: {err}");
                        }
                    });
                }
            }
            _ => {}
        })
        .setup(|app| {
            let data_dir = resolve_data_dir(app.handle())?;
            std::fs::create_dir_all(&data_dir)
                .map_err(|e| format!("创建数据目录 {} 失败: {e}", data_dir.display()))?;

            // 日志：控制台 + <data_dir>/logs/ppclip.log
            app.handle()
                .plugin(logger::init_plugin(&data_dir))
                .map_err(|e| format!("初始化日志失败: {e}"))?;

            log::info!(
                "PPClip v{} 启动，数据目录 {}",
                app.package_info().version,
                data_dir.display()
            );

            // 走了回退/覆盖就说明标准位置写不了，明确记一笔方便排查
            if let Ok(standard) = app.path().app_data_dir() {
                if standard != data_dir {
                    log::warn!(
                        "标准数据目录 {} 不可写，已改用 {}",
                        standard.display(),
                        data_dir.display()
                    );
                }
            }

            // 状态必须在监听线程启动前挂好：监听线程会 try_state 这些状态
            let storage = storage::Storage::new(&data_dir)?;
            app.manage(storage);

            let settings = settings::load_settings(&data_dir);
            app.manage(SettingsState(Mutex::new(settings.clone())));
            app.manage(SelfWrite::new());
            app.manage(CaptureState::default());
            app.manage(viewer::ViewerState::default());
            app.manage(ballhub::BallHubState::default());
            app.manage(DataDir(data_dir));

            // 托盘：入口之一（另一条是悬浮球）。建失败不该阻断启动 —— 悬浮球还在。
            if let Err(err) = tray::init(app.handle()) {
                log::warn!("创建托盘图标失败: {err}");
            }

            // 恢复悬浮球：尺寸摆正 → 还原锚点（形状中心）→ 按上次记录的形态落位。
            // 全部在窗口还藏着（visible:false）的时候做完，所以首帧就是最终形态，
            // 不会先闪一下常规球再变成贴边胶囊。形态不靠坐标反推 —— 常规球被钳到屏幕边时
            // 形状边同样贴着 0，分不出来。
            if let Err(err) = ballhub::init(app.handle(), &settings) {
                log::warn!("恢复悬浮球失败: {err}");
            }
            if let Some(panel) = app.get_webview_window("main") {
                if let (Some(x), Some(y)) = (settings.panel_x, settings.panel_y) {
                    let _ = panel.set_position(tauri::PhysicalPosition::new(x, y));
                }
            }

            // 「桌面置顶」= 常驻：置顶状态下启动直接把面板摆出来（不抢焦点，
            // 开机自启时不要抢走用户正在打字的窗口），没置顶就还是老样子只留悬浮球
            apply_panel_pinned(app.handle(), settings.panel_pinned);
            if settings.panel_pinned {
                if let Some(panel) = app.get_webview_window("main") {
                    let _ = panel.show();
                }
                notify_panel_visibility(app.handle());
            }

            // 剪贴板监听
            match clipboard::start_watcher(app.handle().clone()) {
                Ok(shutdown) => {
                    app.manage(WatcherHandle(Mutex::new(Some(shutdown))));
                }
                Err(err) => log::error!("剪贴板监听启动失败: {err}"),
            }

            // 恢复上次钉在桌面上的贴图
            pins::restore_pins(app.handle());

            // 便于脚本化与冒烟测试：启动后完成一次全屏截图并退出
            // （截图结果照常写剪贴板 + 进历史）
            if std::env::args().any(|arg| arg == "--capture-fullscreen") {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(1500));
                    let code = match capture::capture_fullscreen(&handle) {
                        Ok(()) => {
                            log::info!("--capture-fullscreen 完成");
                            0
                        }
                        Err(err) => {
                            log::error!("--capture-fullscreen 失败: {err}");
                            1
                        }
                    };
                    handle.exit(code);
                });
            }

            // 区域截图冒烟测试：只验证「冷冻图 → 藏窗口 → 建 overlay → 前端把 overlay 显示出来」
            // 这条链路（选区要人来拖），跑完就退出。这条链路曾经因为 wry#583 死锁，
            // 把悬浮球和面板永久藏在后台 —— 见 `run_off_main`。
            if std::env::args().any(|arg| arg == "--capture-region") {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(1000));
                    let code = match capture::start_region_capture(&handle) {
                        Ok(()) => {
                            let mut shown = false;
                            for _ in 0..50 {
                                std::thread::sleep(std::time::Duration::from_millis(100));
                                if capture::overlay_is_visible(&handle) {
                                    shown = true;
                                    break;
                                }
                            }
                            if shown {
                                log::info!("--capture-region 完成：overlay 已显示");
                                0
                            } else {
                                log::error!("--capture-region 失败：overlay 没有显示出来");
                                1
                            }
                        }
                        Err(err) => {
                            log::error!("--capture-region 失败: {err}");
                            1
                        }
                    };
                    handle.exit(code);
                });
            }

            // 轮盘链路冒烟：验证「球窗口尺寸恒定 + 轮盘整块可见 + 中心盘钉在球的位置」，
            // 不需要人点鼠标（跑完自动退出，退出码 0/1）
            if std::env::args().any(|arg| arg == "--smoke-wheel") {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(4000));
                    let code = smoke_wheel(&handle);
                    handle.exit(code);
                });
            }

            // 吸附链路冒烟：模拟「拖到边上松手」，验证看门狗 + 吸附判定确实跑到了，
            // 并断言胶囊贴住屏幕边、屏幕中间不吸（跑完自动退出，退出码 0/1）
            if std::env::args().any(|arg| arg == "--smoke-dock") {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(4000));
                    let code = smoke_dock(&handle);
                    handle.exit(code);
                });
            }

            // 面板置顶冒烟：置顶关时切到桌面必须自动收起，置顶开时必须常驻
            // （会短暂抢前台焦点，跑完自动退出，退出码 0/1）
            if std::env::args().any(|arg| arg == "--smoke-panel") {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(4000));
                    let code = smoke_panel(&handle);
                    handle.exit(code);
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::update_settings,
            commands::get_data_dir,
            commands::open_data_dir,
            commands::open_url,
            commands::check_latest_release,
            commands::file_details,
            commands::reveal_path,
            commands::copy_text,
            commands::file_hashes,
            commands::toggle_panel,
            commands::show_panel,
            commands::hide_panel,
            commands::set_panel_pinned,
            commands::hide_ball,
            commands::show_ball,
            commands::ball_visible,
            commands::is_autostart_enabled,
            commands::set_autostart,
            commands::ball_snapshot,
            commands::frontend_log,
            commands::set_ball_wheel,
            commands::hub_cursor_inside,
            commands::set_ball_docked,
            commands::begin_ball_drag,
            commands::finish_ball_drag,
            commands::save_panel_position,
            commands::show_ball_menu,
            commands::quit_app,
            commands::list_clips,
            commands::clip_stats,
            commands::get_clip,
            commands::copy_clip,
            commands::delete_clip,
            commands::clear_clips,
            commands::show_clip_menu,
            commands::open_viewer,
            commands::close_viewer,
            commands::viewer_clip,
            commands::create_pin,
            commands::list_pins,
            commands::get_pin,
            commands::delete_pin,
            commands::update_pin_geometry,
            commands::zoom_pin,
            commands::show_pin_menu,
            commands::start_region_capture,
            commands::capture_fullscreen,
            capture::overlay_ready,
            capture::get_overlay_image,
            capture::finish_region_selection,
            capture::cancel_capture,
        ])
        .build(tauri::generate_context!())
        .expect("PPClip 构建失败");

    app.run(|handle, event| {
        if let tauri::RunEvent::Exit = event {
            clipboard::stop_watcher(handle);
            log::info!("PPClip 已退出");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{blur_tick, BlurTick, HIDE_AFTER_FOREIGN_TICKS};

    /// 跑一拍
    fn tick(visible: bool, pinned: bool, foreign: bool, armed: &mut bool, ticks: &mut u32) -> BlurTick {
        blur_tick(visible, pinned, foreign, armed, ticks)
    }

    #[test]
    fn blur_tick_stops_when_panel_is_hidden() {
        let (mut armed, mut ticks) = (true, 3);
        assert_eq!(tick(false, false, true, &mut armed, &mut ticks), BlurTick::Stop);
        assert!(!armed, "面板没了，武装状态也要清掉");
        assert_eq!(ticks, 0);
    }

    #[test]
    fn blur_tick_never_hides_when_pinned() {
        let (mut armed, mut ticks) = (true, 3);
        assert_eq!(tick(true, true, true, &mut armed, &mut ticks), BlurTick::Idle);
        assert!(!armed);
        assert_eq!(ticks, 0);
    }

    #[test]
    fn blur_tick_arms_while_our_process_owns_the_foreground() {
        let (mut armed, mut ticks) = (false, 0);
        assert_eq!(tick(true, false, false, &mut armed, &mut ticks), BlurTick::Idle);
        assert!(armed);
    }

    #[test]
    fn blur_tick_hides_only_after_being_armed() {
        // 面板是在别人当前台时被唤出来的（Windows 前台锁定会让 set_focus 失败）：
        // 不能当成「用户点了外部」，否则面板一露头就消失
        let (mut armed, mut ticks) = (false, 0);
        for _ in 0..10 {
            assert_eq!(tick(true, false, true, &mut armed, &mut ticks), BlurTick::Idle);
        }

        // 用户点过面板（前台变成自己）之后再点别处 → 才收起
        let (mut armed, mut ticks) = (false, 0);
        assert_eq!(tick(true, false, false, &mut armed, &mut ticks), BlurTick::Idle);
        assert_eq!(tick(true, false, true, &mut armed, &mut ticks), BlurTick::Idle);
        assert_eq!(tick(true, false, true, &mut armed, &mut ticks), BlurTick::Hide);
        assert!(!armed, "收起后武装状态归零，下一次要重新武装");
    }

    #[test]
    fn blur_tick_debounces_a_single_foreign_tick() {
        // 进程退出 / 窗口销毁那一瞬间也可能短暂「前台是别人」，一拍不算数
        let (mut armed, mut ticks) = (false, 0);
        tick(true, false, false, &mut armed, &mut ticks); // 武装
        tick(true, false, true, &mut armed, &mut ticks); // 第 1 拍：不动
        assert_eq!(tick(true, false, false, &mut armed, &mut ticks), BlurTick::Idle);
        assert_eq!(ticks, 0, "回到自己这边，去抖计数归零");

        // 但持续「前台是别人」一定要收（HIDE_AFTER_FOREIGN_TICKS 拍后）
        let (mut armed, mut ticks) = (false, 0);
        tick(true, false, false, &mut armed, &mut ticks);
        for n in 1..HIDE_AFTER_FOREIGN_TICKS {
            assert_eq!(tick(true, false, true, &mut armed, &mut ticks), BlurTick::Idle);
            assert_eq!(ticks, n);
        }
        assert_eq!(tick(true, false, true, &mut armed, &mut ticks), BlurTick::Hide);
    }
}
