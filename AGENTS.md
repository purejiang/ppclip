# AGENTS.md

## 项目概览

Windows 剪贴板历史 + 截图 + 桌面贴图小工具。常驻一个置顶悬浮球作为入口，
没有托盘、没有全局热键（v1 有意为之）。

## 关键命令

```bash
npm run tauri:dev     # 开发（Vite + Rust 编译 + 悬浮球窗口，长时间运行）
npm run tauri:build   # 生产打包 → NSIS 安装器
npm run build         # 仅前端（tsc -b && vite build）—— 会生成 dist/
npm run lint          # oxlint
cd src-tauri && cargo test    # Rust 单元测试
cd src-tauri && cargo check   # Rust 类型检查

# 无需点击即可验证截图链路（写剪贴板 + 进历史后自动退出，退出码 0/1）
./src-tauri/target/release/ppclip.exe --capture-fullscreen

# 区域截图链路冒烟测试：建 overlay + 前端把它显示出来（选区仍需人拖），跑完自动退出
./src-tauri/target/release/ppclip.exe --capture-region
```

**注意**：

- 编译 Rust 前 `dist/` 必须存在，否则 `tauri::generate_context!` 直接 panic。
- 编译前确认 `ppclip.exe` 已退出，否则文件占用（os error 5）。

## 技术栈

**Tauri 2 + React 19 + TypeScript + Vite + oxlint + rusqlite(bundled) + SQLite**

不引入 UI 组件库、状态管理库、i18n 库：界面用 CSS 变量手写（同 openstart 风格）。

## 架构

### Rust 模块（`src-tauri/src/`）

| 文件 | 职责 | 注意 |
|---|---|---|
| `lib.rs` | 启动、状态注册、菜单事件、窗口事件、invoke_handler、退出钩子 | 状态必须**先** `manage` 再启动监听线程 |
| `ballhub.rs` | 悬浮球的悬停枢纽态：把 56×56 的球窗撑成 240×240（球心不动）再还原 | 撑着 / 收起都按「窗口中心 = 球心」算；收起尺寸用**实测值**（见已知问题 12） |
| `clipboard.rs` | 剪贴板监听线程、内容过滤、去重入库、自身写入抑制 | `start_watch()` 阻塞，必须独立线程；`SelfWrite` 见下 |
| `capture.rs` | 截图：显示器定位、冻结图、overlay 窗口、裁剪、写剪贴板入库 | `clamp_rect` 是纯函数，有单测 |
| `storage.rs` | SQLite：建表/迁移、clips 与 pins 的 CRUD、去重、LRU 淘汰 | **最大文件**，改淘汰逻辑务必跑单测 |
| `commands.rs` | 所有 `#[tauri::command]` + 菜单复用的 `*_impl` 内部函数 | 命令与 `api.ts` 必须两端同步 |
| `pins.rs` | 贴图窗口的创建/恢复/关闭、缩放、初始几何 | 几何一律用 **physical** 值 |
| `viewer.rs` | 完整内容窗口：单例（label `viewer`）、初始尺寸、贴在面板旁边 | 内容由 `ViewerState` 决定，`viewer:show` 事件只当「去问一次」的提示 |
| `menu.rs` | 原生右键菜单（悬浮球 / 条目 / 贴图）+ 菜单 id 分发 | id 格式 `<作用域>:<动作>` 或 `<作用域>:<行id>:<动作>` |
| `models.rs` | `AppSettings` / `Clip` / `Pin` / `ClipStats` / `UpsertOutcome` | 字段与 `src/types.ts` 严格对应 |
| `settings.rs` | `settings.json` 读写（缺字段补默认，原子写） | |
| `util.rs` | sha256、预览截断、PNG 编码、缩略图、人类可读字节 | 纯函数 + 单测 |
| `logger.rs` | 日志：控制台 + `<data_dir>/logs/ppclip.log` | 发布版也保留文件日志 |

### 前端（`src/`）

| 文件 | 职责 |
|---|---|
| `main.tsx` | 单入口按窗口 label 分发（ball / main / overlay / pin-*） |
| `api.ts` | `invoke()` 类型化包装、`assetUrl()`、数据目录缓存 |
| `windows/PanelApp.tsx` | 历史面板：搜索、筛选、键盘导航、点击复制 |
| `windows/BallApp.tsx` | 悬浮球：拖动 vs 点击判定、右键调用原生菜单、悬停 0.3 秒撑成枢纽态（中心球 + 四个环绕按钮） |
| `windows/OverlayApp.tsx` | 截图选区：冻结图为背景 + 四块遮罩 + 尺寸提示 + 确定/取消 |
| `windows/PinApp.tsx` | 桌面贴图：拖动回写、滚轮缩放、关闭 |
| `windows/ViewerApp.tsx` | 完整内容窗口：全文（可选中复制）/ 原图（适应窗口 ↔ 原始尺寸） |
| `components/ClipRow.tsx` | 单条历史：预览 + 第二行时间 + 右侧复制 / 删除图标（单击行看详情） |
| `components/SettingsSheet.tsx` | 设置弹层 |
| `styles/index.css` | 设计系统 + 各窗口样式（按 `html[data-window]` 切底色） |

### 窗口

| label | 关键配置 | 说明 |
|---|---|---|
| `ball` | 56×56、transparent、alwaysOnTop、skipTaskbar、`visible:false` | 前端挂载后调 `show_ball` 命令显示，避免白闪（别用 JS 的 `show()`，见已知问题 9）；鼠标停满 0.3 秒后由 `ballhub` 撑成 240×240，见设计决策 13 |
| `main` | 420×560、不透明、alwaysOnTop、skipTaskbar、`visible:false` | 关闭事件被拦截 → 只隐藏；标题栏任意处可拖，位置回写 settings.json |
| `overlay` | 运行时创建，覆盖光标显示器**物理**范围，不透明 | 先抓图后建窗，抓图时不会把自己拍进去；拖出选区后停在「确定 / 取消」，不自动提交 |
| `pin-<id>` | 运行时创建，transparent、alwaysOnTop、skipTaskbar | 几何用 physical 设置 |
| `viewer` | 运行时创建，单例，alwaysOnTop、skipTaskbar、可缩放 | 全文 / 原图；文本 760×620、图片按原始像素钳到 1280×960；自动停在面板右侧（放不下换左侧），面板不收 |

## 关键设计决策（改动前先看这里）

1. **右键菜单全部用原生菜单**，不用 HTML 菜单：悬浮球窗口只有 56×56、贴图窗口也很小，
   HTML 菜单会被窗口边界裁掉。菜单项 id 用 `<作用域>:<动作>` 前缀，`menu.rs` 统一分发。
2. **overlay 不依赖窗口透明**：它是不透明窗口，铺满冻结图，暗化靠选区四周 4 块遮罩，
   选区内部露出未变暗的原图。这样视觉等于冻结画面，又少一个失败面。
3. **坐标换算不猜 DPI**：overlay 前端用 `img.naturalWidth / img.clientWidth` 求比例，
   把 CSS 像素换成冻结图的物理像素再交给 Rust；Rust 侧 `clamp_rect` 再钳制一次。
4. **自身写入抑制**（`clipboard.rs::SelfWrite`）：本应用写剪贴板时记下内容哈希，
   监听线程遇到相同哈希就跳过；图片被系统重新编码导致哈希变化时，用 1.2s 时间窗兜底，
   且只抑制紧随其后的那一次变化。
5. **淘汰保护贴图**：`evict` 跳过所有被 `pins` 引用的条目；删除条目时贴图行级联删除，
   命令层负责关掉对应窗口。
6. **去重是「命中即置顶」**：`upsert_*` 命中已有哈希时只更新 `last_used_at` / `use_count`，
   不产生重复条目、不重写文件。
7. **监听线程用 `try_state`**：状态缺失时降级而不是 panic（线程 panic 会让剪贴板历史静默失效）。
8. **日志用 info 级别**记录启动、截图、淘汰、贴图恢复；正常噪声（空剪贴板等）走 debug。
9. **overlay 是「两段式」交互**：拖拽只负责画选区，松开鼠标**不会**立刻截 ——
   选区、四周遮罩、尺寸提示都留在屏幕上，右下角出现「确定 / 取消」；
   还能在选区里拖动整体挪位置、在别处重新拖拽重画，双击或 Enter 确定、Esc / 右键取消。
   改这块注意两点：动作条要 `stopPropagation`，否则点按钮会被当成开始新选区；
   「确定 / 取消」用 `fired` ref 保证只触发一次。
10. **窗口拖拽写 `data-tauri-drag-region="deep"`**：裸属性只认「直接点在该元素上」，
   面板标题那个子 `<span>` 就拖不动（这正是「面板拖不了」的原因）；`="deep"` 让子树任意处都能拖，
   而按钮属于 clickable，Tauri 会自动挡掉拖拽，不影响点击。
   面板 / 查看窗口的位置由前端 `onMoved` 节流回写 settings.json。
11. **面板行是「单击看详情，右侧两个图标：复制 / 删除」**：行用 `div role="button"`（里面还有按钮，
    嵌套 `<button>` 是非法结构），两个按钮都 `stopPropagation`，避免冒泡成「看详情」；
    第二行显示相对时间（刚刚 / N 分钟前 / 今天 HH:MM / 昨天 …），由面板每分钟刷新一次 `now`。
    删除是破坏性的（文件 + 库记录一起没），所以**点两次**：第一次只把按钮变成红色「确认删除」，
    3 秒不点自动撤销（`PanelApp` 的 `pendingDelete`）；删除期间条目行还在，方便反悔。
    查看窗口是**单例**（label `viewer`）并停在面板旁边：连点几条只在同一个窗口里换内容，
    不然每点一条就多一个 webview。键盘：Enter = 详情，Ctrl+Enter = 复制。
12. **查看窗口的拖动**：标题栏 `="deep"` 整片可拖；正文留白、图片、页脚也能拖
    （所以图片可以像贴图那样拖着走）；文本 `<pre>` 故意**不**挂 drag-region，
    否则一拖就变成移动窗口、没法选中复制。图片双击 = 切换适应窗口 / 原始尺寸。
13. **悬浮球的枢纽态是「撑窗」，不是「画大一点」**：HTML 只能画在窗口里，悬停时要露出的
    四个环绕按钮必须靠窗口本身变大 —— `ballhub::set_expanded` 把 ball 窗口在
    56×56 ↔ 240×240 之间切换。展开的触发条件是**鼠标在球上停满 0.3 秒**
    （`BallApp` 的 `HOVER_OPEN_DELAY_MS`，中途移开 / 开始拖动就作废）：光标只是路过时
    不该弹出一圈按钮，更不该顺带把那块 240×240 的桌面点击吃掉。0.3 秒这个值的来由：
    停住不动本身已经是最强的意图信号，0.1s 是「感觉瞬时」的门槛、1s 是「流程被打断」的
    门槛，悬停类交互普遍落在 150~400ms（Windows 自己的鼠标悬停时间默认约 400ms）；
    短到 0.3s 还有个好处是**不需要进度提示**，长到 1~2s 就必须给反馈了。
    另外枢纽态一开，球上的 `title` 要摘掉（`title={open ? undefined : ...}`）：
    光标还停在球上，原生提示会比枢纽态晚一步冒出来，正好糊在那圈按钮上。两条硬约束：
    ① **球心不能动**，撑 / 收都按「窗口中心 = 球心」重算左上角（`top_left_for_center` 有单测），
    否则鼠标底下的球会当场跳走；② **收起尺寸不能用配置里的 56**，Windows 会按
    `SM_CXMINTRACK` 把它撑到 136（已知问题 12），所以第一次展开时把实测尺寸记进
    `BallHubState`，收起时照它还原。
    **窗口尺寸的唯一真相在 Rust**，前端只按 `ball:hub` 事件同步 CSS —— 因为窗口一藏起来
    前端就收不到 `mouseleave`：截图前藏窗口（`hide_own_windows`）、隐藏悬浮球、第二实例唤醒
    这几条路径都得由 Rust 主动收回，否则下次露出来是个 240×240 的大方框。
    前端那边收起时还要 `blur()` + 把 `scrollTop/scrollLeft` 归零：窗口一缩小，
    画面外那个「刚刚被点过、还带着焦点」的环绕按钮会诱使浏览器滚动画布把它露出来，
    球心就跟窗口中心对不上了（`overflow:hidden` 挡不住这种程序性滚动）。
    另外卫星按钮点截图前前端会先 `await` 收回枢纽态：截图冻结帧是**在藏窗口之前**抓的
    （`start_region_capture`），不收回就会把一圈按钮拍进冻结图。
    展开期间那块 240×240 的透明方区会吃掉桌面点击（已知问题 12 的同一个毛病），
    所以只做成「鼠标一离开就收回」的临时态，不做常驻。

## 已知问题

1. **不要在 `Cargo.toml` 里删掉**：
   ```toml
   [profile.dev.package.libsqlite3-sys]
   debug = false
   ```
   本机环境下 MSVC 带调试信息编译 `sqlite3.c` 会失败
   （`cl: 命令行 error D8050 :无法执行 c1.dll`）。关掉这个 C 依赖的调试信息即可绕过，
   Rust 侧仍保留调试符号。排查过程：手动 `cl /c /Z7 sqlite3.c` 正常，
   说明是 cc-rs 调用路径下的 MSVC 行为，与沙箱/命令长度/管道无关。
2. 全屏独占应用（游戏、部分播放器）会盖住悬浮球与贴图，v1 不做定时重新置顶。
3. 混合 DPI 多显示器：只支持在光标所在屏幕内完成选区，跨屏选区未实现。
4. 悬浮球与贴图依赖窗口透明；若渲染成黑底方块，回退方案改成不透明圆角窗口。
5. 重新编译前 `ppclip.exe` 必须退出（os error 5）。
6. **调试「启动就 panic」的正确姿势**：Tauri 在 `setup` 阶段会**先**按 `tauri.conf.json`
   创建窗口（`WebviewWindowBuilder::from_config(...)?.build()?`），**之后**才调用我们注册的
   setup 闭包。所以窗口 / WebView2 初始化失败也会报成 `Failed to setup app: ...`，
   而我们闭包里的日志一行都不会打 —— 别一上来就怀疑数据目录。
   给 setup 每一步加错误上下文（本次就是这么做的）能立刻区分是哪一步：
   ```text
   Failed to setup app: error encountered during setup hook: 创建数据目录 C:\...\com.purejiang.ppclip 失败: 拒绝访问。 (os error 5)
   ```
7. **受限环境（沙箱 / 自动化）已经自动适配**：`run()` 里在 Builder 之前调
   `resolve_data_dir_early()`，按 `PPCLIP_DATA_DIR` → `%APPDATA%\<id>` →
   `%LOCALAPPDATA%\<id>` → `<exe 同级>/ppclip-data` 顺序**实测可写性**（建目录 + 写探测文件），
   命中回退时顺手把 `WEBVIEW2_USER_DATA_FOLDER` 一起挪过去，否则窗口创建会先失败。
   正常环境走标准位置，行为不变。
   注意 asset protocol 的 scope 必须覆盖所有可能的数据目录
   （当前是 `$APPDATA/**`、`$APPLOCALDATA/**`、`$EXE/**`；其中 `$EXE` 是 exe 所在**目录**），
   否则那一种模式下缩略图加载不出来。用户自己指定到别处的 `PPCLIP_DATA_DIR`
   不在 scope 里，缩略图不显示属预期。
8. `Get-ChildItem <不存在的目录> -Recurse` 在本机沙箱里会挂起，探路径前先用 `Test-Path`。
9. **`core:default` 里没有 `allow-show`**：悬浮球是 `visible:false` 启动、由前端 `show()` 的，
   而 JS 的 `getCurrentWindow().show()` 属于 `core:window:allow-show`，`core:default` 只给查询类权限
   （`allow-inner-size` / `allow-is-visible` 之类）。被 ACL 拒掉时 Promise 被 `void` 吞掉，
   现象是「双击 exe 之后什么都不出现，没有托盘 / 任务栏 / 桌面图标，再也进不来」。
   现在 `BallApp` 改走应用自己的 `show_ball` 命令（app 命令不受 ACL 约束），
   capability 里也补了 `allow-show`；**新增前端窗口 API 前先确认权限在不在 `core:default` 里**
   （作者已经为 `allow-start-dragging` 踩过一次）。
   另一半是 **capability 的 `windows` 列表**：不在列表里的窗口，一条窗口 API 都用不了 ——
   查看窗口漏登记时 `data-tauri-drag-region` 拖不动（命令照样能用，因为 app 命令不走 ACL），
   所以「窗口能开、能复制、但拖不动」这个组合基本就是漏登记。加运行时窗口时记得同步这里。
10. **Windows 上建 / 关 webview 必须离开 WebView2 的回调栈**（wry#583，Tauri 在
    `WebviewWindowBuilder` 文档里明确写了「用 async 命令或独立线程」）。同步 `#[tauri::command]`
    的实现、以及原生菜单回调（`TrackPopupMenu` 的模态循环仍在 `show_*_menu` 的 IPC 回调栈里）
    都跑在 WebView2 回调里，在那里 `build()` 永远等不到控制器创建完成的回调 → 死锁。
    症状很有迷惑性：overlay 只建出 `WRY_WEBVIEW` + `Chrome_WidgetWin_0` 空壳
    （没有 `Chrome_WidgetWin_1` / renderer / D3D），日志停在建窗那一行之前，
    而悬浮球与面板已经在 `hide_own_windows` 里藏好了 —— 应用成了看不见也点不到的僵尸，
    连重启都唤不醒（单实例回调也跑在那条被卡住的主线程上），只能杀进程。
    规矩：**命令层 `async` + `crate::run_off_main`（spawn_blocking），菜单层 `menu::spawn_detached`**；
    只要新命令碰 `WebviewWindowBuilder` / `window.close()` 就这么办。
11. `hide_own_windows` 之后的任何失败路径都必须把窗口恢复回来：建 overlay 失败会回滚
    （`cancel_capture_impl`），另有 10s 看门狗兜底（overlay 没显示出来就取消并恢复）。
    改截图流程时别破坏这两条兜底 —— 没有托盘，窗口一藏就等于应用消失了。
12. 悬浮球窗口在 Windows 上**不是 56×56**：带 `WS_DLGFRAME` 的窗口宽度受 `SM_CXMINTRACK`
    （本机 136）钳制，实际是 136×56，两侧各约 40px 透明区域。`place-items:center` 让球的位置看着正常，
    但那块区域照样吃掉点击、而且窗口置顶。彻底解决要按光标位置切 `set_ignore_cursor_events`，v1 先记着。
    悬停枢纽态（设计决策 13）继承了这条：撑到 240×240 期间那块透明方区同样吃点击，
    所以展开态只在「鼠标在里面」时存在；`ballhub` 也正因为这条才要记**实测**收起尺寸。

## 发布

沿用 openstart：`dev` 日常开发，`main` 只接收合并；tag 打在 `main` 上
（`git tag -a vX.Y.Z` + `git push`），打包后建 GitHub Release。
