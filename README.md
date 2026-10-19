# PPClip

轻量剪贴板 + 截图小工具：常驻一个桌面悬浮球（外加一个系统托盘图标），自动攒剪贴板历史
（文本 / 图片 / 文件），支持区域与全屏截图，能把内容「贴」在桌面上随时查看。

Tauri 2 + React 19；常驻内存约 100MB 量级（Rust 进程 + WebView2）。

## 功能（v1）

| 能力 | 说明 |
|---|---|
| 剪贴板历史 | 文本、图片、一次复制的多个文件；内容哈希去重（重复复制只置顶不新增）；搜索、按类型筛选；单条删除、清空全部；默认上限 300 条 + 图片 200MB |
| 截图 | 区域截图（拖出选区后可继续微调，Enter / 双击确定，Esc 取消）、全屏截图（光标所在显示器）；结果自动写入剪贴板并进历史 |
| 悬浮球 | 置顶、可拖动的入口，三种形态：贴边胶囊 → 鼠标放上去变成圆球 → 停 2 秒展开快捷盘（区域截图 / 全屏截图 / 剪贴板历史 / 隐藏悬浮球）；拖到屏幕边会吸附，鼠标离开自动滑回；有新条目时带角标 |
| 托盘图标 | 左键开合面板；右键菜单：打开面板 / 区域截图 / 全屏截图 / 显示悬浮球 / 退出 PPClip |
| 历史面板 | 列表显示预览（文本 / 图片缩略图 / 文件清单）；单击一条在右侧查看窗口打开详情，行尾图标是复制 / 贴到桌面 / 删除；↑↓ 选择、Enter 看详情、Ctrl+Enter 复制、Esc 收起；左上角设置，右上角桌面置顶与关闭 |
| 查看窗口 | 完整内容：文本可选中复制，图片可缩放（适应窗口 / 原始尺寸），文件显示格式 / 大小 / 修改时间 / 路径，可按需算 SHA-256 与 MD5、在资源管理器里定位；窗口停在面板旁边，连点只换内容 |
| 桌面贴图 | 把文本或图片钉在桌面上，可拖动、滚轮缩放、关闭；重启后按上次的位置与尺寸恢复 |
| 设置 | 面板的第二屏：历史条数上限（50 一档）、图片存储上限、数据目录、悬浮球显隐与不透明度、面板不透明度、开机自启 |
| 其他 | 单实例（再启动一次就唤回面板）、开机自启、文件日志 |

## 交互速查

- 悬浮球：单击开合面板；按住拖动可挪位置，拖到屏幕边会吸附成胶囊
- 贴边胶囊：鼠标放上去展开成圆球；圆球上停 2 秒展开快捷盘；右键出原生菜单
- 托盘图标：左键开合面板，右键出菜单
- 面板：左上角齿轮进设置，右上角是桌面置顶和最右边的关闭
- 面板：单击条目 → 右侧查看窗口打开详情；行尾图标是复制 / 贴到桌面 / 删除（删除要点两次）
- 设置：面板不透明度只把玻璃底色调透，文字始终清晰；100% = 完全不透明
- 查看窗口：Esc 返回；右上角复制、贴到桌面；图片双击在「适应窗口 / 原始尺寸」之间切换
- 贴图：滚轮缩放，右键菜单，右上角 ✕ 关闭
- 截图：拖出选区 → Enter 或双击确定，Esc / 右键取消

## 快速开始

要求：Windows 10/11、Node 20+、Rust（MSVC 工具链）、WebView2 运行时（Win11 自带）。

```bash
npm install
npm run tauri:dev      # 开发：Vite + Rust 编译 + 悬浮球
npm run tauri:build    # 打包：产出 NSIS 安装器
npm run build          # 仅前端构建（tsc -b && vite build）
npm run lint           # oxlint
cd src-tauri && cargo test    # Rust 单元测试
cd src-tauri && cargo check   # Rust 类型检查
```

命令行截图（给脚本/冒烟测试用，结果照常写剪贴板并进历史）：

```bash
ppclip.exe --capture-fullscreen
```

打包产物：`src-tauri/target/release/bundle/nsis/PPClip_0.1.0_x64-setup.exe`

## 数据位置

`%APPDATA%\com.purejiang.ppclip\`

| 路径 | 内容 |
|---|---|
| `ppclip.db` | SQLite（clips / pins 两张表） |
| `images/`、`thumbs/` | 图片原图与缩略图，文件名 = 内容 sha256 |
| `cache/overlay.png` | 截图时的冻结图（用完即删） |
| `settings.json` | 历史上限、图片上限、悬浮球/面板位置 |
| `logs/ppclip.log` | 运行日志 |

## 在受限环境（沙箱 / 自动化）里运行

正常安装使用不需要看这段。应用启动时会**自己探测数据目录能不能写**，按顺序尝试：

1. `%APPDATA%\<identifier>` —— 标准位置，与 asset protocol 的 `$APPDATA/**` 对得上
2. `%LOCALAPPDATA%\<identifier>` —— `$APPLOCALDATA/**`
3. `<exe 同级>/ppclip-data` —— 便携兜底，`$EXE/**`

前一个写不了就用下一个，并在日志里留一条
`WARN 标准数据目录 ... 不可写，已改用 ...`。**沙箱终端（例如 DSH 的开发终端）就是这种情况**：
除项目目录外都写不了，它会自动落到第 2 或第 3 个位置 —— 所以 `npm run tauri:dev` 照常能跑，
缩略图也照常显示（这三种位置都在 asset protocol 的 scope 里）。

想手动指定位置就用环境变量。注意自定义位置**不在** scope 里，
面板里的缩略图会加载不出来（日志出现 `asset protocol not configured to allow the path`）：

```powershell
$env:PPCLIP_DATA_DIR = 'D:\tmp\ppclip-data'
.\ppclip.exe --capture-fullscreen     # 附带：命令行全屏截图，写完剪贴板与历史后自动退出
```

## 已知限制（v1 有意不做）

- 没有全局热键 —— 入口是悬浮球和托盘图标；隐藏悬浮球后，用托盘菜单的「显示悬浮球」召回。
- 托盘上只有图标和菜单，不显示新条目数量（角标只在悬浮球上）。
- 截图不支持标注（矩形/箭头/马赛克/文字）、OCR、窗口截图、滚动长截图。
- 只记录文本、图片与文件列表，不保留富文本/HTML 格式。
- 多显示器仅支持在**光标所在的那块屏幕**内完成选区（混合 DPI 下跨屏选区留给后续版本）。

## 构建相关注意事项

1. **`src-tauri/Cargo.toml` 里的 `[profile.dev.package.libsqlite3-sys] debug = false` 不能删。**
   本机环境里 MSVC 带调试信息编译 `sqlite3.c` 会失败（`cl: error D8050 无法执行 c1.dll`），
   关掉该 C 依赖的调试信息即可绕过；Rust 侧仍保留调试符号。
2. **编译 Rust 前必须存在 `dist/`**（`tauri::generate_context!` 会检查 `frontendDist`）。
   先跑一次 `npm run build`，或直接用 `npm run tauri:dev`。
3. 重新编译前确认 `ppclip.exe` 已退出，否则会报文件占用（os error 5）。
4. 更换图标：替换 `src-tauri/icons/` 后需要重新 `cargo build`（图标嵌进二进制）；
   原始图源是仓库根目录的 `icon-source.png`，可用 `npx tauri icon icon-source.png` 重新生成整套。

## 发布约定

沿用 openstart 的做法：`dev` 日常开发，`main` 只接收合并并在其上打 tag
（`git tag -a vX.Y.Z` + `git push`），打包后创建 GitHub Release。
