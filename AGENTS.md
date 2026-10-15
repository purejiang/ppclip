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
| `clipboard.rs` | 剪贴板监听线程、内容过滤、去重入库、自身写入抑制 | `start_watch()` 阻塞，必须独立线程；`SelfWrite` 见下 |
| `capture.rs` | 截图：显示器定位、冻结图、overlay 窗口、裁剪、写剪贴板入库 | `clamp_rect` 是纯函数，有单测 |
| `storage.rs` | SQLite：建表/迁移、clips 与 pins 的 CRUD、去重、LRU 淘汰 | **最大文件**，改淘汰逻辑务必跑单测 |
| `commands.rs` | 所有 `#[tauri::command]` + 菜单复用的 `*_impl` 内部函数 | 命令与 `api.ts` 必须两端同步 |
| `pins.rs` | 贴图窗口的创建/恢复/关闭、缩放、初始几何 | 几何一律用 **physical** 值 |
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
| `windows/BallApp.tsx` | 悬浮球：拖动 vs 点击判定、右键调用原生菜单 |
| `windows/OverlayApp.tsx` | 截图选区：冻结图为背景 + 四块遮罩 + 尺寸提示 |
| `windows/PinApp.tsx` | 桌面贴图：拖动回写、滚轮缩放、关闭 |
| `components/ClipRow.tsx` | 单条历史（文本预览 / 图片缩略图） |
| `components/SettingsSheet.tsx` | 设置弹层 |
| `styles/index.css` | 设计系统 + 各窗口样式（按 `html[data-window]` 切底色） |

### 窗口

| label | 关键配置 | 说明 |
|---|---|---|
| `ball` | 56×56、transparent、alwaysOnTop、skipTaskbar、`visible:false` | 前端挂载后 `show()`，避免白闪 |
| `main` | 420×560、不透明、alwaysOnTop、skipTaskbar、`visible:false` | 关闭事件被拦截 → 只隐藏 |
| `overlay` | 运行时创建，覆盖光标显示器**物理**范围，不透明 | 先抓图后建窗，抓图时不会把自己拍进去 |
| `pin-<id>` | 运行时创建，transparent、alwaysOnTop、skipTaskbar | 几何用 physical 设置 |

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

## 发布

沿用 openstart：`dev` 日常开发，`main` 只接收合并；tag 打在 `main` 上
（`git tag -a vX.Y.Z` + `git push`），打包后建 GitHub Release。
