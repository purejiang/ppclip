# PPClip

轻量剪贴板 + 截图小工具：常驻一个桌面悬浮球，自动攒剪贴板历史（文本 + 图片），
支持区域/全屏截图，能把内容「钉」在桌面上随时查看。

Tauri 2 + React 19，安装包约 8MB，常驻内存约 100MB 量级（Rust 进程 + WebView2）。

## 功能（v1）

| 能力 | 说明 |
|---|---|
| 剪贴板历史 | 文本 + 图片；内容哈希去重（重复复制只置顶不新增）；搜索；按类型筛选；单条删除 / 全部清空；默认上限 300 条 + 图片 200MB |
| 截图 | 区域截图（拖拽选区、实时显示像素尺寸、Esc 取消）、全屏截图（光标所在显示器）；结果自动写入剪贴板并进历史 |
| 悬浮球 | 56×56 圆形，置顶可拖动，位置记忆；左键开合面板，右键原生菜单（区域截图 / 打开面板 / 隐藏悬浮球 / 退出） |
| 历史面板 | 列表（文本预览 / 图片缩略图）；点击即复制并收起面板；右键原生菜单（复制 / 贴到桌面 / 删除）；↑↓ 选择、Enter 复制、Esc 收起 |
| 桌面贴图 | 把历史条目钉在桌面，可拖动、滚轮缩放、关闭；重启后按上次位置与尺寸恢复 |
| 其他 | 单实例、文件日志、JSON 设置、隐藏悬浮球后靠「再次启动 PPClip」恢复入口 |

## 交互速查

- 点悬浮球 → 面板出现/收起
- 面板里点条目 → 复制到剪贴板并收起面板（接着去目标窗口 Ctrl+V）
- 面板里右键条目 → 复制 / 贴到桌面 / 删除
- 面板右上角：`截` 区域截图、`全` 全屏截图、`⚙` 设置、`✕` 收起
- 贴图上滚轮缩放、右键菜单、右上角 ✕ 关闭
- 截图时拖拽选区即完成，Esc 或右键取消

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

打包产物：`src-tauri/target/release/bundle/nsis/PPClip_0.1.0_x64-setup.exe`

## 数据位置

`%APPDATA%\com.ppclip.app\`

| 路径 | 内容 |
|---|---|
| `ppclip.db` | SQLite（clips / pins 两张表） |
| `images/`、`thumbs/` | 图片原图与缩略图，文件名 = 内容 sha256 |
| `cache/overlay.png` | 截图时的冻结图（用完即删） |
| `settings.json` | 历史上限、图片上限、悬浮球/面板位置 |
| `logs/ppclip.log` | 运行日志 |

## 已知限制（v1 有意不做）

- 没有托盘图标、没有开机自启、没有全局热键 —— 入口是悬浮球；隐藏悬浮球后，再启动一次 PPClip 即可恢复。
- 截图不支持标注（矩形/箭头/马赛克/文字）、OCR、窗口截图、滚动长截图。
- 只记录文本与图片，不记录复制的文件列表，不保留富文本/HTML 格式。
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
