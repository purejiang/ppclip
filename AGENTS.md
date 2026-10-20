# AGENTS.md

## 项目概览

Windows 剪贴板历史 + 截图 + 桌面贴图小工具。入口有两个：置顶悬浮球和系统托盘图标；
没有全局热键（v1 有意为之）。

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

# 悬浮轮盘冒烟：展开再收起一轮，断言盘整块在屏内 / 盘心居中在窗口里 /
# 收起后窗口矩形一比一还原 / 锚点全程不变，退出码 0/1
# （跑测试时建议给个独立数据目录：PPCLIP_DATA_DIR=... 免得跟正在跑的实例抢 WebView2 用户目录）
./src-tauri/target/release/ppclip.exe --smoke-wheel

# 吸附链路冒烟：模拟「拖到右边 10px 处松手」「悬停唤出」「鼠标离开自动贴回」
# 「屏幕中间松手」，验证拖动看门狗 + 吸附判定确实跑到、胶囊贴住屏幕边、
# 常规球离屏幕边留 6px、离开后能自己滑回胶囊、中间不吸，退出码 0/1
# （会挪一次真实光标，跑完恢复；挪不动就 warn 跳过自动贴回那条断言）
./src-tauri/target/release/ppclip.exe --smoke-dock

# 面板置顶冒烟：未置顶时把前台切成桌面必须自动收起、置顶时必须常驻、
# 第二个实例唤起面板时不许被误关，退出码 0/1（会短暂抢前台焦点，跑完自动退出）
./src-tauri/target/release/ppclip.exe --smoke-panel
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
| `ballhub.rs` + `ballhub/geom.rs` | 悬浮入口三态与贴边吸附。`geom.rs` 是**纯几何**（形状尺寸 / 窗口矩形 / 吸附判定 / 轮盘摆位），不碰窗口、全部有单测；`ballhub.rs` 只负责「什么时候调哪个函数」+ 落盘 | 几何真相是 `anchor`（形状中心）而不是窗口矩形；窗口尺寸一律用**实测值**（宽度被 `SM_CXMINTRACK` 撑着，见已知问题 12）；收起态竖向留 `SHAPE_PAD_LOGICAL`=16（投影 / 角标，别删）；展开时轮盘要钳进显示器，中心盘留在盘心（别再补偿回锚点） |
| `clipboard.rs` | 剪贴板监听线程、内容过滤、去重入库、自身写入抑制 | `start_watch()` 阻塞，必须独立线程；**文件（CF_HDROP）必须先于文本判定**；`SelfWrite` 见下 |
| `capture.rs` | 截图：显示器定位、冻结图、overlay 窗口、裁剪、写剪贴板入库 | `clamp_rect` 是纯函数，有单测 |
| `storage.rs` | SQLite：建表/迁移、clips 与 pins 的 CRUD、去重、LRU 淘汰 | **最大文件**，改淘汰逻辑务必跑单测；文件条目的路径列表以 JSON 存 `clips.text`（零迁移） |
| `autostart.rs` | 开机自启：读写 `HKCU\...\CurrentVersion\Run` | **注册表是唯一真相**（settings.json 不存这个字段）；只申请 `KEY_QUERY_VALUE` / `KEY_SET_VALUE` |
| `commands.rs` | 所有 `#[tauri::command]` + 菜单复用的 `*_impl` 内部函数 | 命令与 `api.ts` 必须两端同步 |
| `pins.rs` | 贴图窗口的创建/恢复/关闭、缩放、初始几何 | 几何一律用 **physical** 值 |
| `viewer.rs` | 完整内容窗口：单例（label `viewer`）、初始尺寸、贴在面板旁边 | 内容由 `ViewerState` 决定，`viewer:show` 事件只当「去问一次」的提示 |
| `menu.rs` | 原生右键菜单（悬浮球 / 条目 / 贴图）+ 菜单 id 分发 | id 格式 `<作用域>:<动作>` 或 `<作用域>:<行id>:<动作>` |
| `tray.rs` | 系统托盘：图标、提示文字、菜单、左键开合面板 | 菜单项沿用 `ball:` 前缀，和悬浮球右键菜单同一套动作（不另开分发）；左键开面板、右键出菜单 |
| `models.rs` | `AppSettings` / `Clip` / `Pin` / `ClipStats` / `UpsertOutcome` / `ClipKind` | 字段与 `src/types.ts` 严格对应；`ClipKind` 有 text / image / file 三种 |
| `settings.rs` | `settings.json` 读写（缺字段补默认，原子写） | |
| `util.rs` | sha256、预览截断、PNG 编码、缩略图、人类可读字节 | 纯函数 + 单测 |
| `logger.rs` | 日志：控制台 + `<data_dir>/logs/ppclip.log` | 发布版也保留文件日志 |

### 前端（`src/`）

| 文件 | 职责 |
|---|---|
| `main.tsx` | 单入口按窗口 label 分发（ball / main / overlay / viewer / pin-*） |
| `api.ts` | `invoke()` 类型化包装、`assetUrl()`、数据目录缓存、事件订阅（panel:visible / settings:changed / clips / pins） |
| `windows/PanelApp.tsx` | 历史面板：搜索、胶囊筛选（含「单击即复制」闪电）、键盘导航、状态条；**设置页是同一扇窗的第二屏** |
| `windows/BallApp.tsx` | 悬浮入口三态：贴边胶囊 / 常规球 / HUD 轮盘；形态由 `ball:form` 驱动（不再按尺寸猜）；`switching` + `nextPaint()` 遮掉换尺寸那一帧；拖动 vs 点击判定、右键原生菜单、悬停 2 秒展开、新条目角标、不透明度 |
| `windows/OverlayApp.tsx` | 截图选区：冻结图为背景 + 四块遮罩 + 尺寸提示 + 确定/取消 |
| `windows/PinApp.tsx` | 桌面贴图：拖动回写、滚轮缩放、关闭 |
| `windows/ViewerApp.tsx` | 完整内容窗口：meta 条、点阵画布、缩放控件、复制/贴图（toggle）+ toast 提示 |
| `components/ClipRow.tsx` | 单条历史：40×40 类型图标块 + 预览/缩略图 + 两行 meta + 复制 / 贴图 / 删除 |
| `components/SettingsPage.tsx` | 设置页（容量 / 数据目录 / 悬浮球 / 面板不透明度 / 开机自启，卡片 + 滑杆 + 分段 + 开关） |
| `components/icons.tsx` | 全应用共享的线性图标集（24 网格、1.7 描边、`currentColor`），**图标一律从这里取** |
| `styles/index.css` | Glacier 设计系统（玻璃 + 冰蓝）+ 各窗口样式（按 `html[data-window]` 切底色） |

### 窗口

| label | 关键配置 | 说明 |
|---|---|---|
| `ball` | 配置写 16×96，**实际按 DPI 实测**（无边框窗口被 `SM_CXMINTRACK` 撑宽，本机 1.5 缩放是 202×144）；transparent、alwaysOnTop、skipTaskbar、`visible:false` | 稿子里的**桌面悬浮球三态**：前端挂载后调 `show_ball` 显示。收起态尺寸**恒定**（胶囊与球共用一扇窗，只有 CSS 换形状），展开轮盘时窗口换成 232×232（200 的盘 + 四周各 16 的余量）。几何真相是**锚点**（形状中心），见设计决策 13 |
| `main` | 420×600、transparent（圆角玻璃卡片）、skipTaskbar、`visible:false`；`alwaysOnTop` **由 `panel_pinned` 决定**（启动时 `apply_panel_pinned` 校正） | 关闭事件被拦截 → 只隐藏；标题栏任意处可拖，位置回写 settings.json；**设置页是这扇窗的第二屏**；未置顶时**失焦自动收起** |
| `overlay` | 运行时创建，覆盖光标显示器**物理**范围，不透明 | 先抓图后建窗，抓图时不会把自己拍进去；拖出选区后停在「确定 / 取消」，不自动提交 |
| `pin-<id>` | 运行时创建，transparent、alwaysOnTop、skipTaskbar | 几何用 physical 设置 |
| `viewer` | 运行时创建，单例、transparent（同款圆角卡片）、alwaysOnTop、skipTaskbar、可缩放 | 内容卡片：meta 条 + 点阵画布 + 缩放控件；文本 760×620、图片按原始像素钳到 1280×960；自动停在面板右侧（放不下换左侧），面板不收 |

托盘不是窗口：图标和菜单由 `tray.rs` 建，不参与窗口几何，也不用登记到 capability 的 `windows` 列表。

## 关键设计决策（改动前先看这里）

1. **右键菜单全部用原生菜单**，不用 HTML 菜单：悬浮入口的形状只有 16×64、贴图窗口也很小，
   HTML 菜单会被窗口边界裁掉。菜单项 id 用 `<作用域>:<动作>` 前缀，`menu.rs` 统一分发。
   托盘菜单（`tray.rs`）复用同一批 `ball:` id，因此不需要第二套分发。
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
11. **面板行是「单击看详情 + 右侧图标：复制 / 贴图 / 删除」**：行用 `div role="button"`（里面还有按钮，
    嵌套 `<button>` 是非法结构），按钮都 `stopPropagation`，避免冒泡成「看详情」；
    第二行 meta 是稿子的两行结构：`n chars / 尺寸 · 大小` 一行、等宽字体时间一行，
    由面板每分钟刷新一次 `now`。**单击就看详情，没有第二种单击行为**：曾经的
    「单击即复制」开关（`AppSettings::click_to_copy`）已移除 —— 它和看详情互斥，
    开着的时候鼠标再没有详情入口，只能靠键盘 Enter 或右键菜单，等于把功能藏起来。
    贴到桌面是 toggle：已贴上时按钮常亮（`IconFloatCard` 画中画），再点取消
    （面板 refresh 时顺带 `list_pins` 拿已贴条目）。**文件条目没有贴图按钮**
    （`create_pin_impl` 也会拒），因为它没有像素、`text` 里只有路径 JSON。
    删除是破坏性的（文件 + 库记录一起没），所以**点两次**：第一次只把按钮变成红色「确认删除」，
    3 秒不点自动撤销（`PanelApp` 的 `pendingDelete`）；删除期间条目行还在，方便反悔。
    查看窗口是**单例**（label `viewer`）并停在面板旁边：连点几条只在同一个窗口里换内容，
    不然每点一条就多一个 webview。键盘：Enter = 详情，Ctrl+Enter = 复制。
    **选中态（`active`）由方向键和单击共同维护**：单击一行先 `setActive(index)` 再开详情
    （否则单击后高亮还停在第 0 条）。列表的「选中项进视野」**只依赖 `active` / `settingsOpen`，
    不能依赖 `clips`**：面板每次 refresh（复制了新内容、关掉详情窗口后重新聚焦）都会拿到
    新数组，跟着 `scrollIntoView` 会把用户手动滚到的位置拽回 active（多为第 0 条）——
    用户反馈「滚到底部点开详情，X 掉后跳回顶部」就是这条触发的。换搜索词 / 筛选是另一件事：
    要主动 `scrollTo({ top: 0 })` 并把 `active` 重置为 0（列表换了一批内容）。
12. **查看窗口的拖动**：标题栏 `="deep"` 整片可拖；内容区留白、图片、页脚也能拖
    （所以图片可以像贴图那样拖着走）；文本 `<pre>` 故意**不**挂 drag-region，
    否则一拖就变成移动窗口、没法选中复制。图片双击 = 切换适应窗口 / 原始尺寸，
    右下角缩放控件用 `clip.width/height × zoom` 直接给图片定尺寸（不用 transform，
    这样滚动条范围是对的），「适应」与「100%」两个按钮分别回到 fit 与 1:1。
13. **入口是三态悬浮球**（稿子「桌面悬浮球三态规范」）：一扇 `ball` 窗、三种形状 ——
    16×64 贴边胶囊 / 52×52 常规球 / 200×200 HUD 轮盘（稿子写 240，实机太大收到 200，
    改 `WHEEL_LOGICAL` 一个常量即可）。几何在 `ballhub/geom.rs`（纯函数 + 单测），
    编排在 `ballhub.rs`。

    **几何真相只有 `anchor`（形状中心，物理像素）+ `form`（tab/orb）+ `wheel_open`**，
    全在 `BallHubState` 里；前端只吃 `ball:form` 快照 + 事件（带单调递增 `rev`，
    旧快照不许盖掉新事件）。四条硬约束：
    ① **窗口位置永远由 `geom::window_pos(anchor, 实测尺寸)` 反推，不读当前窗口几何** ——
    读一次（启动 / 拖动结束）→ 钳制 → 算矩形 → 写回，因此不累积取整误差。形状在窗口里
    居中，所以「窗口中心 = 形状中心」让撑开 / 收起 / 形态切换都不动屏幕上的形状。
    ② **收起态窗口尺寸恒定**：胶囊与球共用一扇窗（本机 1.5 缩放实测 202×144，宽度被
    `SM_CXMINTRACK` 撑着，别写死），换形态只改位置 —— 单次 `set_position` 是原子的，
    没有「先 resize 再 reposition」的半帧错位。**竖向上下的 16px 余量别删**
    （`SHAPE_PAD_LOGICAL`）：投影与右上角标就画在那圈里，客户区一贴形状就被切平。
    ③ **展开轮盘时窗口换成 232×232**（200 的盘 + 四周各 16，`0 12px 40px` 的投影要画得下）。
    换尺寸 / 换位置仍是两次 OS 调用，中间必有一帧按旧窗口中心绘制 —— **这一帧由前端遮掉**：
    `BallApp` 的 `switching` 把内容 `visibility: hidden`。注意 **`setSwitching(true)` 只是排了
    一次渲染，紧接着的 IPC 不会等它** —— 中间不加 `await nextPaint()`（两个 rAF）就会漏出一帧
    错位：`set_size` 只改右下边（居中内容被挤到右下）、随后的 `set_position` 才把它拉回左上，
    观感是「悬停时往左上角闪一下」（踩过）。
    ④ **贴边时轮盘必须钳进显示器**（否则那一侧的卫星按钮点不到），盘心因此和球心分离
    （最多 74px）。**中心盘 `.ball-core` 恒在盘心，不做任何偏移补偿** —— 轮盘里居中装的是
    *盘*，盘心偏了中心球就贴到盘边上，看起来像球飞了（按「球钉在原地」补偿过一次，
    真机就是这个观感）。轮盘展开时球已经藏起来了，没人会看见盘心的位移。
    **也别改成「整扇窗跟着球走」**：那会把轮盘裁掉一半。

    **吸附（`geom::should_dock`）只看「落点」和「鼠标离开」两个时机**，判据是
    **当前形态的形状左右边**距屏幕边的空隙 ≤ `DOCK_EDGE_MARGIN_LOGICAL`(24)：
    - 不能用形状**中心**：窗口被撑宽后球心恒距窗口左沿一大截，旧实现（中心 ≤ 60 逻辑像素）
      在 100% 缩放下**永远判不中** —— 这就是「拖到边上不吸」的真因；
    - 也不能用**窗口边**：窗口贴住屏幕时球离边还有 42px，会在球明明很远时就吸住；
    - 传**当前形态**是为了拿天然回差：球要贴到边距 24 才吸，胶囊要拖过 24 才松手；
    - 形态（`AppSettings::ball_docked`）必须显式落盘 —— 窗口尺寸恒定之后推不出来了。

    **贴回胶囊带 120ms 滑入动画**（`DOCK_ANIM_MS`，ease-out cubic，15 步 `set_position`）：
    以前是单次 `set_position` 瞬移，用户反馈「吸附太突然」。做法是**先提交形态 / 锚点 / rev
    并 emit，再让独立线程把窗口从松手位置插值到目标**（`apply_dock` → `start_dock_animation`）；
    动画只动窗口、不动 anchor，`persist` 在动画收尾时才写。动画与看门狗都用**单调递增的
    世代号**（`anim_gen` / `redock_gen`）取消，**不要改成跨线程持锁串行化**：动画线程里的
    `set_position` 在 Windows 上要等窗口线程，主线程拿着锁等它就是死锁。
    `finish_drag` 在动画进行中直接返回快照（前端 Promise 和拖动看门狗各调一次，不能拿半路的
    窗口位置重新判定落点）。

    **悬停唤出的常规球离屏幕边留 6px**（`geom::UNDOCK_EDGE_INSET_LOGICAL` → `clamp_orb_inset`）：
    贴边胶囊悬停变成球时，若球只钳到「整块在屏内」，球的右沿会正好压着屏幕右沿，右上角的
    新条目角标（向外挑 5px）、hover 的 `scale(1.05)` 描边、那圈投影都会被切掉（用户反馈
    「球太靠右，右侧被挡住」）。**别把余量调大过 6**：胶囊中心到球心只有 24px（球半径 26），
    再大光标就落到球外了 —— 前端 `.ball-orb::before` 的 4px 命中区就是给这 2px 兜底的。
    **Tab 不走这个余量**（胶囊本来就要贴住边）。

    **鼠标离开球 → 自动滑回原侧胶囊**（`ballhub::arm_redock`）：悬停唤出之后原本没有回程，
    球会一直挂在边上（用户反馈「贴边展示成正常态之后就没有自动贴边了」）。
    看门狗只在命令层 `set_ball_docked(false)` 之后武装（内部 / 冒烟直接调 `set_form` 不武装），
    每 100ms 用真实 `GetCursorPos` 量光标到球**外接矩形**的距离，**连续 3 拍 > 10 逻辑像素**
    才贴回。三条硬约束：
    - **不能按位置上报判**：球被钳进可视区后边缘同样贴着屏幕，位置一上报就会把刚唤出的球
      吸回去（踩过）。只看「鼠标真的离开」；
    - 离开阈值 10 必须大于胶囊比球高出来的 `(TAB_H - ORB)/2 = 6`，否则贴回那一刻光标正好
      落在胶囊上，前端悬停又把胶囊唤回常规态，来回闪（单测
      `redock_leave_gap_clears_the_tab_overshoot` 守着）；
    - 拖动 / 隐藏 / 已贴回都会 `redock_gen + 1` 让看门狗过期；轮盘开着时**暂停计数**
      （收起后若鼠标已离开，仍会贴回）；5 分钟还没离开就放弃。

    **「拖动结束」由 Rust 侧的看门狗判定，不信 JS 的 Promise**：前端在 `startDragging()`
    之前发 `begin_ball_drag`，Rust 起线程每 50ms 查一次真实的左键状态（`GetAsyncKeyState`），
    松手后再等 120ms 让窗口停稳，然后调 `finish_drag`。原因：`startDragging()` 进的是
    Windows 的**模态移动循环**，主线程被占住，JS 那条 `startDragging().then(→ finish)`
    什么时候落地完全说不准 —— 实测会整条丢掉，表现为「拖到边上松手，球停在原地、甚至
    半挂在屏幕外，日志里连一条『拖动结束』都没有」。`finish_drag` 幂等，两边都调不会错；
    它还会在**左键仍按着**时直接跳过（那说明拖动没结束，这时候把窗口拽回边缘会被移动循环覆盖）。

    **交互三段**（判定压在哪一个形状上用 `event.target.closest`，窗口比形状宽得多，
    光标从旁边透明区划过时不该有任何反应）：
    ① 鼠标放到**贴边胶囊**上 → 立刻唤出**常规态**（`set_ball_docked(false)`），不进展开计时；
       之后鼠标离开球（>10 逻辑像素、连续 3 拍）→ Rust 看门狗自动滑回原侧胶囊（见上）；
    ② 在**常规态**上停满 **2 秒** → 展开**轮盘**（`set_ball_wheel(true)`），移开就收回；
    贴边胶囊上**不展开**（`ballhub::set_wheel` 里也再挡一次）；
    ③ 两种收起态的**单击**都是直接开合主面板（拖动则拖走，右键出原生菜单）。
    2 秒是产品定的（宁可慢也不误触：窗口一撑开就吃掉桌面点击）。**别再改回 0.3 秒** ——
    实测那么短会在「鼠标顺手划过球」时误展开。

    其他必须知道的细节：
    - 卫星按钮 / 中心盘点击走 `runAfterClose`：先收轮盘再执行动作 ——
      区域截图的冻结帧**在藏窗口之前**抓（`start_region_capture`），不收就会把轮盘拍进去。
    - 面板打开前 `show_panel` / `toggle_panel` 也会 `close_wheel_now` 兜底。
    - **按住 / 拖动期间不接受任何悬停驱动的状态变化**（`holding` 守卫，`evaluateHover`
      开头就挡）：`onPointerDown` 立刻清掉悬停计时并落 `holding`，`startDragging()` 的
      Promise resolve（松手）时再解除并**把计时器彻底清一次**。不这么写会出两个错：
      ① 拖动开始那一帧的 pointermove 会冒泡回根节点重新 `armExpand()`（形状上的处理器
      刚 disarm 完，根节点的 `evaluateHover` 又装回去）→ 拖着球不动两秒，**拖动中途**轮盘
      直接展开、窗口跳成 232×232；
      ② 松手后那个计时器还活着 → 到点就展开，刚吸上的胶囊立刻被打回圆球（「拖到边上贴不住」）。
    - **拖动结束后的 600ms 不算悬停**（`dragEndedAt` + `POST_DRAG_HOVER_GUARD_MS`，
      挡在 `evaluateHover` 顶部，对「展开」和「胶囊唤出常规态」同时生效）：
      松手时鼠标还在球上，不挡就会把刚吸上的胶囊打回圆球。
    - **形态不再按客户区高度推**（旧的 150/90 阈值已删）：尺寸恒定后根本推不出来，
      而且读到一帧旧尺寸就挂错 CSS 类。前端订阅 `ball:form`，**先 listen 再拉快照**，
      靠 `rev` 挡乱序。
    - 收回轮盘不能只信 pointer 事件：窗口改尺寸会派一次**假 pointerleave**（指针没动，
      之后也不会补 pointerover）。`onPointerLeave` 后 120ms 问 Rust `hub_cursor_inside`
      （Win32 `GetCursorPos`；Tauri 的 `cursor_position()` 给的不是屏幕物理坐标），
      确实不在窗口里才收。
    - 悬浮球这扇窗没有控制台：排查用 `frontend_log(scope, message)` + info 级几何日志
      （`轮盘展开：… 盘心 (cx, cy)（偏离锚点 +dx）` / `悬浮球形态 → …`），比看现象快得多。
    - 无人值守冒烟：`ppclip.exe --smoke-wheel`（展开再收起一轮，断言盘整块在屏内、
      盘心居中在窗口里、收起后矩形一比一还原、锚点全程不变；退出码 0/1）；
      `ppclip.exe --smoke-dock`（拖动松手吸附 + 悬停唤出留 6px + 鼠标离开自动贴回 +
      屏幕中间松手不吸，会挪一次真实光标再恢复）。

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
    导航：面板 / 查看窗口都是 48px 居中品牌头。面板是「左上设置 + 居中品牌 + 右上 桌面置顶 / 关闭」，
    **关闭按钮永远在最右**（设置屏也是，左上换成「← 返回」；Esc 在设置屏只回上一层）；
    查看窗口是「左占位 + `PPCLIP` + 右上按钮」。
    内容页是 `48px 头 → 内容卡片 → 24px 页脚（Esc 关闭）`；设置是面板窗口的**第二屏**
    （标题栏换成「← + 剪贴板设置」），不是弹层。
    **面板不透明度**（设置里的滑杆）只缩放玻璃**底色**的 alpha —— 卡片的 `100%`、标题栏 `70%`、
    状态条 `90%`、页脚 `80%`、描边 `30%` 都乘同一个 CSS 变量 `--panel-opacity`（由 `PanelApp` 写在
    `<html>` 上，只在 `main` 上生效）。**别改成整窗 `opacity`**：12px 中文会被压成灰雾，
    而且桌面本来也糊不到（见上文），「更透」只能靠降底色 alpha。
    **卡片底色基数必须是 100%**（不能退回 85%）：用户反馈「100% 还是有点透」——
    玻璃卡片的 85% 在桌面上仍能透出来，滑杆拉到 100 必须完全不透明；想要玻璃感往下拖，
    标题栏 / 状态条 / 页脚再按原来的 70% / 90% / 80% 比例跟着变透。
    **页脚右端只放快捷键提示**（面板 `Esc 关闭`、查看窗口 `Esc 返回`），别的信息一律走左边：
    稿子 `600x500_v3` 的 footer 就是「左 MIME、右 `Esc Close`」，把「创建时间」排在 Esc 后面
    会把它从最右边挤开（用户反馈「Esc 返回应该常驻最右侧」）。查看窗口左边那组挂
    `.app-footer-facts`（`flex: 1 1 auto` + `min-width: 0` + 子项省略号），
    右组 `flex: 0 0 auto` —— 窗口拖窄时截左边文字，**别把 Esc 挤出可视区**
    （实测 300px 宽时左边两段都出省略号、Esc 仍贴右边距 12px）。
    图标全部走 `components/icons.tsx`（24 网格、`stroke-width:1.7`、`currentColor`）——
    别往界面上写「截 / 全 / ⚙ / ✕」这类中文字形当图标，也别引图标字体（离线环境）。
    **一图一义，且不留未使用的图标**：
    - 同一件事在不同窗口必须用同一个图标（区域截图 = `IconScan`、全屏截图 = `IconMonitor`，
      轮盘卫星按钮与面板状态条一致）；反过来说，**视觉相近但含义不同的东西必须能一眼分开**：
      「适应窗口」是四角**内收**（`IconFitScreen`），「原始尺寸」是对角**外扩**（`IconActualSize`）——
      旧的「取景框」版本和 `IconScan` 撞脸，已换掉。
    - **贴到桌面 ≠ 窗口置顶**，两个概念别共用一个图形：**贴到桌面**（把一条内容变成桌面小窗）
      用**画中画** `IconFloatCard`（大框右下开口 + 里面一张小窗）；**窗口桌面置顶**（把面板推到
      最上层、点外部不关）用「顶线 + 向上箭头」`IconKeepOnTop`。
      两代踩坑都记着：图钉（读作「钉住」）和便签（折角纸，和文件条目撞脸）都不行；
      置顶曾用「两扇窗」，读起来像多窗口，也不对。
    - **清空全部历史 = 橡皮擦 `IconEraser`**，不是扫帚（语义糊），更不能是垃圾桶
      （那是「删除这一条」的 `IconTrash`）；破坏性靠按钮的 danger 红色标识，不靠图形。
    - 来源徽章用剪贴板 `IconClipboard`，**别用地球**（地球 = 网络 / 公开，语义不符，
      `IconPublic` 已删）。
    - 词典里躺着的没人用的图形会被下一个人顺手乱用，所以删干净：
      `IconBolt`（单击即复制功能已下线）、`IconTerminal`（来源统一成剪贴板 / 截图徽章）都已移除。
      加图标时在注释里写清「用在哪」。
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
    `历史条目上限`（滑杆，50 一档，`max_items`）、`存储上限`（进度条用真实 `image_bytes` + 分段快档，
    `max_image_mb`）、`数据目录管理`（`open_data_dir`）、`悬浮球管理`（开关 = `ball_visible` /
    `show_ball` / `hide_ball`，不透明度 = `ball_opacity`，经 `settings:changed` 实时生效）、
    `面板不透明度`（滑杆，`panel_opacity`，只调玻璃底色）、
    `开机自启`（`autostart.rs`，注册表为唯一真相）。
    「清空全部历史」按稿子放在面板状态条上，「退出」在标签右键菜单里 —— 设置页里不重复放。
16. **文件条目（`ClipKind::File`）= 资源管理器里复制的一组文件**：Windows 上就是 `CF_HDROP`，
    读写走 `clipboard-rs` 的 `get_files` / `set_files`。
    - **判定顺序：文件 → 文本 → 图片**。资源管理器复制文件时会同时放出路径文本，
      先看文本就会把「复制三个文件」记成一条路径文本；反过来不成立，所以顺序安全。
    - 路径列表以 **JSON 存在 `clips.text`**：不另开表、不加列（迁移零成本），
      而且 `text LIKE ?` 的搜索照样能命中文件名。`Clip.files` 由 `storage::row_to_clip`
      解析出来，**前端不碰 JSON**（解析失败给空数组，别让一条烂数据把整个列表带塌）。
    - **归一化 + 指纹必须两边同一套**（`util::normalize_files` / `util::files_key`）：
      采集（`clipboard.rs`）与自身写入抑制（`commands::write_to_clipboard`）算出的 hash
      要一致，否则「点一条文件历史去复制」会被监听线程当成新内容再录一遍。
      指纹**按小写算**（Windows 路径大小写不敏感），落库的路径保持原样。
    - `bytes` 只用于显示「总共多大」，**不参与图片容量淘汰**（那条 SQL 只统计 `kind='image'`）。
    - 复制时剔掉已经不存在的路径（资源管理器粘不存在的路径会整个失败），剔完为空就报错
      让用户看见「这些文件都不在了」，而不是静默复制成功。
    - **详情卡是「现场 stat」，不落库**（用户反馈「文件复制没有显示详细信息」后补的，
      照 stitch 稿 `600x500_v3`「CLIPBOARD DETAIL」）：`file_details` 现场读
      大小 / 修改时间 / 是否还在 / 是否文件夹，返回 `models::FileDetail`
      （含大写扩展名、中文类型说明、MIME）。**别把这几项加进 `clips` 表** —— 用户随时
      可能改 / 删 / 挪文件，存快照只会显示过期信息，还会和「复制前剔掉不存在的路径」打架。
      查看窗口里单个文件是「文件卡 → 格式 / 大小 / 修改时间 → 绝对路径块 → 校验和块」，
      多个文件退化成「总览卡 + 一行一个文件」（每行仍能复制路径 / 定位）。
    - 配套四个命令（都在 `commands.rs`）：`file_details`（现场 stat）、`reveal_path`
      （`explorer /select,<path>` 在资源管理器里定位）、`copy_text`（复制文件名 / 路径 /
      校验码，**必须记 `SelfWrite`** —— 否则复制出来的路径会被监听线程当成一条新历史）、
      `file_hashes`（流式 SHA-256 **+ MD5**，**按需**点按钮才算：几百 MB 的文件不能一开详情就卡，
      所以设计稿里「直接显示 MD5」在这里改成「计算校验和 → SHA-256 / MD5 两行 + 各自复制」）。
    - **校验和为什么两个都给**：稿子那屏写的是「MD5 校验和」（另一屏的文件卡片又写
      `Verified checksum SHA-256`，稿子自己就不一致）；MD5 早就能构造碰撞、不适合当安全校验，
      但发布页和 Md5Checker 这类工具习惯贴 MD5，只给 SHA-256 用户没法直接比对。
      所以 `util::file_hashes` **一次读盘喂两个 hasher**（别写成读两遍，大文件 IO 直接翻倍），
      详情卡上 SHA-256 在上、MD5 在下，两个都能单独复制。
    - 面板行照稿子给**单个文件**挂类型标签（`PDF` 那种，`.clip-badge`）+ 只报大小；
      多个文件仍报「N 个文件 · 总大小」（类型标签那一格留给数量）。
17. **面板「桌面置顶」（`AppSettings::panel_pinned`）**：面板右上角那个置顶按钮
    （顶线 + 向上箭头 `IconKeepOnTop`），
    开 = `always_on_top` + 常驻；关 = 普通窗口 + **失焦自动收起**。三处必须一致：
    - `set_panel_pinned` 命令同时改窗口属性与落盘设置，并广播 `settings:changed`；
    - `setup` 里启动时先 `apply_panel_pinned` 再决定要不要把面板摆出来（置顶状态启动即常驻，
      但**不抢焦点** —— 开机自启时不要抢走用户正在打字的窗口）；
    - **「点击外部关闭」是轮询前台窗口，不是 `WindowEvent::Focused`**（`lib::watch_panel_blur`）：
      实测把前台换成桌面 / 别的程序后，`is_focused()` 已经翻了，**面板却收不到任何 Focused
      事件**，事件驱动会漏（`--smoke-panel` 就是这么发现并守住这条的）。
      现在由一条**只在面板露着时存在**的线程每 200ms 查一次单拍决策 `blur_tick`：
      - 判据是 `foreground_is_foreign`：前台窗口的进程**既不是本进程、也不是同一个 exe 的
        另一个实例**。后一条必须留着 —— 用户再点一次图标唤面板时，第二个 `ppclip.exe`
        会短暂持有前台，只比 pid 的话它长得像「别人」，刚露面的面板会立刻被关（实测踩过，
        `--smoke-panel` ③ 守这条）；
      - `armed` 表示这一轮里真的见过「前台是自己」——面板若是在别的程序当前台时被唤出来的
        （Windows 前台锁定会拒掉 `set_focus`），不能第一拍就判成「用户点了外部」；
      - 连续 `HIDE_AFTER_FOREIGN_TICKS`(2) 拍才真收（400ms 去抖）：进程退出 / 窗口销毁
        那一瞬间也会短暂「前台是别人」；
      - 自家窗口（悬浮球 / 贴图 / 查看 / overlay）之间怎么跳都不算失焦。
      置顶状态下这段逻辑整体不生效（常驻）。
    - `tauri.conf.json` 里 `main` 的 `alwaysOnTop` 写 **false**（默认不置顶），
      置顶由 `apply_panel_pinned` 打开 —— 别写回 true，否则未置顶时窗口仍是常驻最上层。
    - 标题栏品牌名改成**绝对居中**（`.app-brand`）：右上角按钮数量会变（设置 / 置顶），
      靠 flex 留白的话每多一个按钮标题就往左挪 36px。

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
   现象是「双击 exe 之后悬浮球不出现」—— 任务栏 / 桌面都没有图标，只能从托盘图标开面板。
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
    改截图流程时别破坏这两条兜底 —— 球和面板一起藏起来时，用户就只剩托盘图标这一条入口了。
12. 悬浮球窗口在 Windows 上**宽度不是我们说了算**：带 `WS_DLGFRAME` 的窗口宽度受 `SM_CXMINTRACK`
    （本机 136）钳制，**收起态实测 202×144（1.5 缩放）**，胶囊与常规球共用这一扇窗
    （窗口高度 = 形状 + 上下各 16px 投影余量，设计决策 13 ②）。
    形状靠居中显示看着正常，但窗口两侧那片透明区照样吃掉点击、而且窗口置顶 ——
    **吸附**（设计决策 13）把窗口故意推出屏幕 `(窗口宽 − 16)/2`，那块透明区大半落到屏幕外，
    问题基本被绕掉了。展开态（232×232）只在鼠标悬停期间存在。
    彻底解决仍要按光标位置切 `set_ignore_cursor_events`，v1 先记着。
13. **受限令牌 / 沙箱下写 `HKCU\...\Run` 会被拒**（`拒绝访问。 (os error 5)`）：开机自启在
    正常桌面会话里没问题（已实测：把 exe 放到 `%TEMP%` 启动，开关 → 注册表出现该值、
    日志 `已开启开机自启`、再点一次能删掉）。但**从某些受限环境（写受限令牌 / 沙箱）启动
    工作目录里的 exe** 时，进程没有该键的写权限，注册表读得到、写不进去 ——
    界面会如实报错并回滚开关，日志里是 `切换开机自启失败`。别把它当代码 bug 查。
    自查姿势：`Get-ItemProperty HKCU:\Software\Microsoft\Windows\CurrentVersion\Run -Name PPClip`。

## 发布

沿用 openstart：`dev` 日常开发，`main` 只接收合并；tag 打在 `main` 上
（`git tag -a vX.Y.Z` + `git push`），打包后建 GitHub Release。
