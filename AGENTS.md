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

不引入 UI 组件库、状态管理库、i18n 库、图标字体：界面用 CSS 变量手写（同 openstart 风格），
图标是 `src/components/icons.tsx` 里的手写 SVG。视觉稿来自 stitch 的
「floatclip desktop manager」（Glacier 玻璃主题 + 桌面悬浮球三态规范）。

## 架构

### Rust 模块（`src-tauri/src/`）

| 文件 | 职责 | 注意 |
|---|---|---|
| `lib.rs` | 启动、状态注册、菜单事件、窗口事件、invoke_handler、退出钩子 | 状态必须**先** `manage` 再启动监听线程 |
| `ballhub.rs` | 悬浮入口三态：16×64 贴边胶囊 ↔ 52×52 常规球 ↔ 200×200 HUD 轮盘（形状中心不动），外加贴边吸附 | 吸附 / 撑窗都按「形状中心」算；收起尺寸用**实测值**（见已知问题 12）；收起态窗口竖向留 `SHAPE_PAD_LOGICAL`=16 的余量（投影 / 角标不被切，别删）；展开时轮盘要钳进显示器 |
| `clipboard.rs` | 剪贴板监听线程、内容过滤、去重入库、自身写入抑制 | `start_watch()` 阻塞，必须独立线程；`SelfWrite` 见下 |
| `capture.rs` | 截图：显示器定位、冻结图、overlay 窗口、裁剪、写剪贴板入库 | `clamp_rect` 是纯函数，有单测 |
| `storage.rs` | SQLite：建表/迁移、clips 与 pins 的 CRUD、去重、LRU 淘汰 | **最大文件**，改淘汰逻辑务必跑单测 |
| `autostart.rs` | 开机自启：读写 `HKCU\...\CurrentVersion\Run` | **注册表是唯一真相**（settings.json 不存这个字段）；只申请 `KEY_QUERY_VALUE` / `KEY_SET_VALUE` |
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
| `main.tsx` | 单入口按窗口 label 分发（ball / main / overlay / viewer / pin-*） |
| `api.ts` | `invoke()` 类型化包装、`assetUrl()`、数据目录缓存、事件订阅（panel:visible / settings:changed / clips / pins） |
| `windows/PanelApp.tsx` | 历史面板：搜索、胶囊筛选（含「单击即复制」闪电）、键盘导航、状态条；**设置页是同一扇窗的第二屏** |
| `windows/BallApp.tsx` | 悬浮入口三态：贴边胶囊 / 常规球 / HUD 轮盘；拖动 vs 点击判定、右键原生菜单、悬停 0.3 秒展开、新条目角标、不透明度 |
| `windows/OverlayApp.tsx` | 截图选区：冻结图为背景 + 四块遮罩 + 尺寸提示 + 确定/取消 |
| `windows/PinApp.tsx` | 桌面贴图：拖动回写、滚轮缩放、关闭 |
| `windows/ViewerApp.tsx` | 完整内容窗口：meta 条、点阵画布、缩放控件、复制/贴图（toggle）+ toast 提示 |
| `components/ClipRow.tsx` | 单条历史：40×40 类型图标块 + 预览/缩略图 + 两行 meta + 复制 / 贴图 / 删除 |
| `components/SettingsPage.tsx` | 设置页（容量 / 数据目录 / 悬浮球 / 开机自启，卡片 + 滑杆 + 分段 + 开关） |
| `components/icons.tsx` | 全应用共享的线性图标集（24 网格、1.7 描边、`currentColor`），**图标一律从这里取** |
| `styles/index.css` | Glacier 设计系统（玻璃 + 冰蓝）+ 各窗口样式（按 `html[data-window]` 切底色） |

### 窗口

| label | 关键配置 | 说明 |
|---|---|---|
| `ball` | 16×96（运行时按形态变 52×84 / 64×96：形状居中 + 四周 16px 透明余量）、transparent、alwaysOnTop、skipTaskbar、`visible:false` | 稿子里的**桌面悬浮球三态**：前端挂载后调 `show_ball` 显示；Rust 启动时 / 每次拖动结束后 `ballhub::dock` 决定「贴边胶囊」还是「常规球」，鼠标停满 0.3 秒后撑成 200×200 的 HUD 轮盘，见设计决策 13 |
| `main` | 420×600、transparent（圆角玻璃卡片）、alwaysOnTop、skipTaskbar、`visible:false` | 关闭事件被拦截 → 只隐藏；标题栏任意处可拖，位置回写 settings.json；**设置页是这扇窗的第二屏** |
| `overlay` | 运行时创建，覆盖光标显示器**物理**范围，不透明 | 先抓图后建窗，抓图时不会把自己拍进去；拖出选区后停在「确定 / 取消」，不自动提交 |
| `pin-<id>` | 运行时创建，transparent、alwaysOnTop、skipTaskbar | 几何用 physical 设置 |
| `viewer` | 运行时创建，单例、transparent（同款圆角卡片）、alwaysOnTop、skipTaskbar、可缩放 | 内容卡片：meta 条 + 点阵画布 + 缩放控件；文本 760×620、图片按原始像素钳到 1280×960；自动停在面板右侧（放不下换左侧），面板不收 |

## 关键设计决策（改动前先看这里）

1. **右键菜单全部用原生菜单**，不用 HTML 菜单：悬浮入口的形状只有 16×64、贴图窗口也很小，
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
11. **面板行是「单击行为可切换 + 右侧三个图标：复制 / 贴图 / 删除」**：行用 `div role="button"`（里面还有按钮，
    嵌套 `<button>` 是非法结构），按钮都 `stopPropagation`，避免冒泡成「看详情」；
    第二行 meta 是稿子的两行结构：`n chars / 尺寸 · 大小` 一行、等宽字体时间一行，
    由面板每分钟刷新一次 `now`。单击默认看详情，筛选行右边那颗闪电打开后变成「单击即复制」
    （`AppSettings::click_to_copy`，真实持久化）。图钉是 toggle：已贴上时按钮常亮，再点取消
    （面板 refresh 时顺带 `list_pins` 拿已贴条目）。
    删除是破坏性的（文件 + 库记录一起没），所以**点两次**：第一次只把按钮变成红色「确认删除」，
    3 秒不点自动撤销（`PanelApp` 的 `pendingDelete`）；删除期间条目行还在，方便反悔。
    查看窗口是**单例**（label `viewer`）并停在面板旁边：连点几条只在同一个窗口里换内容，
    不然每点一条就多一个 webview。键盘：Enter = 详情，Ctrl+Enter = 复制。
12. **查看窗口的拖动**：标题栏 `="deep"` 整片可拖；内容区留白、图片、页脚也能拖
    （所以图片可以像贴图那样拖着走）；文本 `<pre>` 故意**不**挂 drag-region，
    否则一拖就变成移动窗口、没法选中复制。图片双击 = 切换适应窗口 / 原始尺寸，
    右下角缩放控件用 `clip.width/height × zoom` 直接给图片定尺寸（不用 transform，
    这样滚动条范围是对的），「适应」与「100%」两个按钮分别回到 fit 与 1:1。
13. **入口是三态悬浮球**（稿子「桌面悬浮球三态规范」）：HTML 只能画在窗口里，所以三种形态
    靠窗口本身切换 —— `ballhub` 把 ball 窗口在 **16×64 胶囊 ↔ 52×52 常规球 ↔ 200×200 轮盘**
    之间调整（稿子展开态写的是 240×240，实机看着太大，收到 200；改 `WHEEL_LOGICAL` 一个常量即可）。
    六条硬约束：
    ① **形状中心不能动**，撑开 / 收起 / 胶囊↔球切换 / 吸附一律按「窗口中心 = 形状中心」重算左上角
    （`top_left_for_center` 有单测），否则鼠标底下那个东西会当场跳走；
    ② **收起尺寸不能用配置里的值**，Windows 会把无边框窗口按 `SM_CXMINTRACK` 撑到 136 宽
    （已知问题 12），所以撑开那一刻把**实测矩形**记进 `BallHubState`，收起时照它还原；
    ③ **轮盘要钳进显示器**：胶囊贴边时按中心撑出来的 200×200 会有一半在屏幕外，
    那一侧的卫星按钮就点不到了（`clamp_into`，有单测）；
    ④ **展开态下不切形态**（`set_docked` 直接返回），免得把轮盘改成小球；
    ⑤ **形态由窗口自己的尺寸推**：前端挂 `onResized` + 读一次 `innerSize()`，按客户区高度认
    （200 轮盘 / 96 胶囊 / 84 常规球 —— 后两个 = 形状 + 上下各 16px 余量）。
    **不要改成自定义事件同步** —— 这里踩过坑：
    `listen()` 注册是异步的，启动时那一次吸附/摆正可能正好落在「问」与「注册完成」之间，
    事件漏掉、查询又拿到旧值，前端就会一直以为自己是常规球（悬停直接展开轮盘），
    而窗口其实已经是胶囊。尺寸推导没有这种时序问题，回收 / 撑开 / 吸附全都自动跟上。
    ⑥ **收起态窗口竖向必须比形状高**（`SHAPE_PAD_LOGICAL = 16`，上下各一圈）：窗口宽度我们
    说了不算（被撑到 136，形状两侧天然有 40+px 富余），**高度是我们定的** —— 一旦客户区高度
    = 形状高度，形状之外的每一像素都在窗口外，投影会被切平（实测：竖向剖面在形状上下边缘
    之外**全是 0**，横向却是完整的平滑渐隐，用户看到的「阴影被剪裁」就是这个），右上角标
    `top:-5px` 削掉上半截、贴边胶囊的顶部角标 `top:-9px` **整颗在窗口外、从来没显示过**。
    竖向钳制也一律按**形状**算（`clamp_center(…, shape_h)`）：那圈余量是透明的，
    允许探出屏幕，否则胶囊永远贴不到屏幕上下边缘。改形状或余量要同步前端阈值
    （`BallApp.tsx::applySize` 的 150 / 90）与 `ballhub` 的两条单测。
    **吸附**（`ballhub::dock`）**只在拖动结束**（`commands::finish_ball_drag`）与启动摆正时跑，
    判定看形状中心离屏幕左右边缘是否进了 40 逻辑像素：进了 → 吸成胶囊（窗口横向推出屏幕
    `(窗口宽 − 16)/2`，顺带把吃点击的透明区推到屏幕外；纵向钳制，胶囊整条可见）；
    没进 → 常规球。两个坑：
    ① **位置上报（`save_ball_position`）只记坐标、绝不判形态**：常规球被钳进可视区之后
    左边缘同样贴着屏幕，拿坐标反推会把刚唤出来的常规态立刻吸回胶囊（踩过）；
    形态另有 `AppSettings::ball_docked` 显式落盘，启动照它还原 —— 常规球和贴边胶囊的几何
    差别只有几十像素，靠坐标分不出来；
    ② **横向钳制按形状算、不按窗口**（`clamp_center`）：窗口宽 136、形状才 52，
    按窗口钳会把球从鼠标底下推走 60px，鼠标一转眼就落在球外面（点不到也悬停不到）。
    展开条件是**鼠标停满 0.3 秒**（`HOVER_OPEN_DELAY_MS`，中途移开 / 开始拖动就作废）：
    光标只是路过时不该弹出一圈按钮，更不该顺带把那块桌面点击吃掉。0.3 秒这个值的来由：
    停住不动本身已经是最强的意图信号，0.1s 是「感觉瞬时」的门槛、1s 是「流程被打断」的门槛，
    悬停类交互普遍落在 150~400ms（Windows 自己的鼠标悬停时间默认约 400ms）；
    短到 0.3s 还有个好处是**不需要进度提示**。另外展开后 `title` 要摘掉
    （`title={open ? undefined : ...}`）：光标还停在上头，原生提示会比轮盘晚一步冒出来。
    收起时前端还要 `blur()` + 把 `scrollTop/scrollLeft` 归零：窗口一缩小，画面外那个
    「刚刚被点过、还带着焦点」的卫星按钮会诱使浏览器滚动画布把它露出来，形状中心就跟窗口
    中心对不上了（`overflow:hidden` 挡不住这种程序性滚动）。
    另外卫星按钮点截图前前端会先 `await` 收回轮盘：截图冻结帧是**在藏窗口之前**抓的
    （`start_region_capture`），不收回就会把轮盘拍进冻结图。
    **三段式交互**（鼠标一路不用点也能走到底，判定压在哪一个形状上用 `event.target.closest`，
    窗口比形状宽得多，光标从旁边透明区划过时不该有任何反应）：
    ① 鼠标放到**贴边胶囊**上 → 立刻唤出**常规态**（`set_ball_docked(false)`），不进展开计时；
    ② 在**常规态**上停满 0.3 秒 → 展开**轮盘**，移开就收回；
    ③ 两种收起态的**单击**都是直接开合主面板（拖动则拖走，右键出原生菜单）。
    点击走 `runSatellite` 那一套（先收回轮盘再执行动作）：点完光标还停在形状上，
    那个「悬停满 0.3 秒展开」的倒计时不掐掉的话，面板刚露出来轮盘就自己弹回来 ——
    面板是后来的置顶窗，轮盘只会从它的边缘探出来，而且面板是半透明玻璃、撑开的轮盘会直接透出来。
    Rust 侧 `show_panel` / `toggle_panel` 打开面板前也会 `ballhub::set_expanded(false)` 兜底。
14. **样式统一在 `styles/index.css` 的 Glacier 设计系统里**（严格照 stitch 稿）：
    token 与稿子的 Tailwind 名字一一对应，带 `/xx` 透明度的存成 `R G B` 三元组，
    写法和稿子一样：`rgb(var(--surface) / 85%)` == `bg-surface/85`。
    底色 `background #0a0e1a` → `surface #0f1524`（卡片 `surface/85`）→ `surface-container #141c2e`；
    主色 `primary #7dd3fc`、`primary-container #0e4d6e`（选中态底色）、`tertiary #c8a0f0`（贴图）、
    `error #ff6b6b`。三条规矩：
    ① **窗口 = 一张圆角玻璃卡片**（`rounded-2xl bg-surface/85 backdrop-blur-2xl border-outline-variant/30`）；
    ② **每扇窗的 main 里都有两张发光斑**（右上 `primary/10`、左下 `tertiary/10`、`blur-3xl`）——
    这是稿子里最明显的氛围光，别省；发光斑是绝对定位的，`app-main` 里其它子元素统一抬一层；
    ③ 字号只有四档：12（正文/品牌）· 11（筛选/按钮/路径）· 10（meta）· 9（角标），
    数字与时间一律等宽字体，**滚动条按稿子全隐藏**（`::-webkit-scrollbar { width: 0 }`，滚轮照常）。
    导航：面板 / 查看窗口都是 48px 居中品牌头（左占位 + `PPCLIP` + 右上按钮），
    内容页是 `48px 头 → 内容卡片 → 24px 页脚（Esc 关闭）`；设置是面板窗口的**第二屏**
    （标题栏换成「← + 剪贴板设置」），不是弹层。
    图标全部走 `components/icons.tsx`（24 网格、`stroke-width:1.7`、`currentColor`）——
    别往界面上写「截 / 全 / ⚙ / ✕」这类中文字形当图标，也别引图标字体（离线环境）。
    **窗口透明只给 ball / pin / main / viewer**：前两者本来就透明，后两者要的是圆角卡片，
    透明区只有四个圆角（`tauri.conf.json` 的 main 与 `viewer.rs` 的 viewer 都是
    `transparent:true` + `shadow:false`；Windows 的方形投影会把圆角画成直角）。
    注意**桌面是拍不到模糊的**：WebView2 的 `backdrop-filter` 只模糊页面内的下层，
    所以「玻璃」的透视感来自半透明，模糊只用在玻璃叠玻璃的地方（输入框、内容卡片）。
    想真的把桌面糊掉得引 `window-vibrancy`（tauri 已经间接依赖了它），v1 不做。
    **悬浮入口的发光是刻意去掉的**：稿子给常规球配了 `-inset-2.5 blur-md` 的呼吸光晕
    与 `0 0 24px` 的主色外发光，实机看就是一圈糊光（用户原话「观感很差」），
    所以常规球 / 胶囊 / 角标都不发光了，只留 2px 渐变环 + 中性投影；
    展开态轮盘的中心盘仍有光（那是展开态的视觉焦点，没被吐槽过）。
15. **设置项以「后端真的有没有」为准**：稿子里的滑杆 / 分段 / 开关全部照做，但只接真实设置 ——
    `历史条目上限`（滑杆，`max_items`）、`存储上限`（进度条用真实 `image_bytes` + 分段快档，
    `max_image_mb`）、`数据目录管理`（`open_data_dir`）、`悬浮球管理`（开关 = `ball_visible` /
    `show_ball` / `hide_ball`，不透明度 = `ball_opacity`，经 `settings:changed` 实时生效）、
    `开机自启`（`autostart.rs`，注册表为唯一真相）。
    「清空全部历史」按稿子放在面板状态条上，「退出」在标签右键菜单里 —— 设置页里不重复放。

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
4. 悬浮球 / 贴图 / 面板 / 查看窗口都依赖窗口透明（面板与查看窗口要的是圆角卡片）；
   若某台机器上渲染成黑底方块（`transparent` 失效），回退方案是把 `--glass-*` 换成实底、
   去掉 `border-radius`，其它样式不用动。
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
12. 悬浮球窗口在 Windows 上**宽度不是我们说了算**：带 `WS_DLGFRAME` 的窗口宽度受 `SM_CXMINTRACK`
    （本机 136）钳制，实测是 **136×96（胶囊）** 或 **136×84（常规球）** —— 窗口高度 = 形状
    + 上下各 16px 投影余量（设计决策 13 ⑥）；形状两侧各约 60px / 42px 透明区，上下各 16px。
    形状靠居中显示看着正常，但那块区域照样吃掉点击、而且窗口置顶 —— 现在**吸附**（设计决策 13）
    把窗口故意推出屏幕 `(窗口宽 − 16)/2`，那块透明区大半落到屏幕外，问题基本被绕掉了。
    彻底解决仍要按光标位置切 `set_ignore_cursor_events`，v1 先记着。
    撑到 200×200 期间（鼠标在形状上时）透明方区同样吃点击，所以展开态只在鼠标悬停期间存在，
    `ballhub` 也正因为这条才要记**实测**收起尺寸。
13. **受限令牌 / 沙箱下写 `HKCU\...\Run` 会被拒**（`拒绝访问。 (os error 5)`）：开机自启在
    正常桌面会话里没问题（已实测：把 exe 放到 `%TEMP%` 启动，开关 → 注册表出现该值、
    日志 `已开启开机自启`、再点一次能删掉）。但**从某些受限环境（写受限令牌 / 沙箱）启动
    工作目录里的 exe** 时，进程没有该键的写权限，注册表读得到、写不进去 ——
    界面会如实报错并回滚开关，日志里是 `切换开机自启失败`。别把它当代码 bug 查。
    自查姿势：`Get-ItemProperty HKCU:\Software\Microsoft\Windows\CurrentVersion\Run -Name PPClip`。

## 发布

沿用 openstart：`dev` 日常开发，`main` 只接收合并；tag 打在 `main` 上
（`git tag -a vX.Y.Z` + `git push`），打包后建 GitHub Release。
