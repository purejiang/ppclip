/**
 * 全应用共享的线性图标集。
 *
 * 规矩（改图标先看这条）：
 * - 24 网格、`stroke-width: 1.7`、圆头圆角、`currentColor`，和界面其它手写 SVG 一个手感；
 * - 不引入图标字体 / 图标库：图标一律是组件，颜色交给 CSS（`color` 控制描边）；
 * - 尺寸由 `size` 决定，默认 18（面板头部 / 行内动作），行内小徽标传 14。
 * - **一图一义**：同一个图形只表达一件事。视觉相近但不相关的东西必须能一眼分开 ——
 *   比如「适应窗口」是四角**内收**、「原始尺寸」是对角**外扩**，别两个都用四角框。
 * - **不留未使用的图标**：这个文件是"可用词汇表"，里面躺着没人用的图形，
 *   下一个人就会顺手拿来乱用（`IconBolt`（单击即复制已下线）、`IconTerminal`
 *   （来源统一成剪贴板/截图徽章）都因此删掉了）。加图标时顺手说明用在哪。
 *
 * 图形取自 Lucide（ISC 许可）的常用几个，几个品牌图形（剪贴板出箭、悬浮球）是本项目手绘。
 */

import type { SVGProps } from "react";

type IconProps = SVGProps<SVGSVGElement> & { size?: number };

function Svg({ size = 18, children, ...rest }: IconProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.7}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
      {...rest}
    >
      {children}
    </svg>
  );
}

/** 搜索 */
export function IconSearch(props: IconProps) {
  return (
    <Svg {...props}>
      <circle cx="11" cy="11" r="7.4" />
      <path d="m20.5 20.5-4.2-4.2" />
    </Svg>
  );
}

/** 区域截图：取景框四角 */
export function IconScan(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M3 7.4V5.4A2.4 2.4 0 0 1 5.4 3h2" />
      <path d="M16.6 3h2A2.4 2.4 0 0 1 21 5.4v2" />
      <path d="M21 16.6v2a2.4 2.4 0 0 1-2.4 2.4h-2" />
      <path d="M7.4 21h-2A2.4 2.4 0 0 1 3 18.6v-2" />
      <rect x="9.4" y="9.4" width="5.2" height="5.2" rx="1.2" />
    </Svg>
  );
}

/** 全屏截图：显示器 */
export function IconMonitor(props: IconProps) {
  return (
    <Svg {...props}>
      <rect x="2.6" y="3.6" width="18.8" height="13" rx="2.2" />
      <path d="M12 16.6v3.6" />
      <path d="M8.6 20.2h6.8" />
    </Svg>
  );
}

/** 设置：齿轮 */
export function IconGear(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" />
      <circle cx="12" cy="12" r="3" />
    </Svg>
  );
}

/** 关闭 */
export function IconClose(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M18 6 6 18" />
      <path d="m6 6 12 12" />
    </Svg>
  );
}

/** 复制到剪贴板：两张纸 */
export function IconCopy(props: IconProps) {
  return (
    <Svg {...props}>
      <rect x="8.4" y="8.4" width="12" height="12" rx="2.4" />
      <path d="M4.4 15.6A2.4 2.4 0 0 1 2 13.2V5.6a2.4 2.4 0 0 1 2.4-2.4h7.6a2.4 2.4 0 0 1 2.4 2.4" />
    </Svg>
  );
}

/** 删除：垃圾桶 */
export function IconTrash(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M3.4 6.4h17.2" />
      <path d="M18.6 6.4v13a2.4 2.4 0 0 1-2.4 2.4H7.8a2.4 2.4 0 0 1-2.4-2.4v-13" />
      <path d="M8.8 6.4V4.6a2 2 0 0 1 2-2h2.4a2 2 0 0 1 2 2v1.8" />
      <path d="M10.4 11v6.4" />
      <path d="M13.6 11v6.4" />
    </Svg>
  );
}

/** 确定 */
export function IconCheck(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M20 6.8 9.4 17.4l-5.4-5.4" />
    </Svg>
  );
}

/** 剪贴板历史：列表 */
export function IconList(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M9.2 6.6h10.6" />
      <path d="M9.2 12h10.6" />
      <path d="M9.2 17.4h10.6" />
      <circle cx="4.9" cy="6.6" r="1.1" />
      <circle cx="4.9" cy="12" r="1.1" />
      <circle cx="4.9" cy="17.4" r="1.1" />
    </Svg>
  );
}

/** 隐藏悬浮球：向下收进托盘 */
export function IconHide(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M12 3.6v9.4" />
      <path d="m8.2 9.4 3.8 3.8 3.8-3.8" />
      <path d="M4.8 19.8h14.4" />
    </Svg>
  );
}

/** 品牌图形：剪贴板 + 出箭头（PPClip 干的就是「把剪贴板里的东西拿出来用」） */
export function IconClipboardArrow(props: IconProps) {
  return (
    <Svg {...props}>
      <rect x="3.4" y="5.2" width="12.4" height="15" rx="2.6" />
      <path d="M7.6 3.4h4a.8.8 0 0 1 .8.8v1.6H6.8V4.2a.8.8 0 0 1 .8-.8Z" />
      <path d="M6.4 11.2h6.4" />
      <path d="M16.6 12.7h3.8" />
      <path d="m18.3 10.6 2.1 2.1-2.1 2.1" />
    </Svg>
  );
}

/** 文本条目 */
export function IconText(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M20.4 6.4H3.6" />
      <path d="M15.4 12H3.6" />
      <path d="M17.4 17.6H3.6" />
    </Svg>
  );
}

/** 图片条目 */
export function IconImage(props: IconProps) {
  return (
    <Svg {...props}>
      <rect x="3.2" y="3.2" width="17.6" height="17.6" rx="2.6" />
      <circle cx="9" cy="9" r="1.7" />
      <path d="m20.8 15.4-3.5-3.5a2 2 0 0 0-2.8 0L6 20.4" />
    </Svg>
  );
}

/** 文件条目（单个文件：折角纸） */
export function IconFile(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M13.6 3.2H7.4a2.4 2.4 0 0 0-2.4 2.4v12.8a2.4 2.4 0 0 0 2.4 2.4h9.2a2.4 2.4 0 0 0 2.4-2.4V8.8z" />
      <path d="M13.6 3.2v5.6h5.4" />
    </Svg>
  );
}

/** 数据目录 */
export function IconFolder(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M20 19.4a2 2 0 0 0 2-2V8.6a2 2 0 0 0-2-2h-7.6a2 2 0 0 1-1.7-.9l-.8-1.3a2 2 0 0 0-1.7-.9H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2Z" />
    </Svg>
  );
}

/** 容量（数据库） */
export function IconDatabase(props: IconProps) {
  return (
    <Svg {...props}>
      <ellipse cx="12" cy="5.4" rx="8.4" ry="2.8" />
      <path d="M3.6 5.4v13.2c0 1.55 3.76 2.8 8.4 2.8s8.4-1.25 8.4-2.8V5.4" />
      <path d="M3.6 12c0 1.55 3.76 2.8 8.4 2.8s8.4-1.25 8.4-2.8" />
    </Svg>
  );
}

/** 悬浮球 */
export function IconBall(props: IconProps) {
  return (
    <Svg {...props}>
      <circle cx="12" cy="12" r="8.4" />
      <circle cx="12" cy="12" r="2.6" fill="currentColor" stroke="none" />
    </Svg>
  );
}

/** 退出 */
export function IconPower(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M12 3.2v8.4" />
      <path d="M18.4 7.2a9 9 0 1 1-12.8 0" />
    </Svg>
  );
}

/** 关于 / 版本信息：关于页头部、设置页「关于 PPClip」卡片 */
export function IconInfo(props: IconProps) {
  return (
    <Svg {...props}>
      <circle cx="12" cy="12" r="8.6" />
      <path d="M12 11v5.4" />
      <circle cx="12" cy="7.8" r="0.9" fill="currentColor" stroke="none" />
    </Svg>
  );
}

/** 检查更新 / 重新检查（圆弧箭头）；检查中给按钮加旋转动画 */
export function IconRefresh(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M20.4 11.2a8.4 8.4 0 0 0-14.9-4.4L3.6 9" />
      <path d="M3.6 4.4V9h4.6" />
      <path d="M3.6 12.8a8.4 8.4 0 0 0 14.9 4.4l1.9-2.2" />
      <path d="M20.4 19.6V15h-4.6" />
    </Svg>
  );
}

/**
 * 打开外部页面（关于页「前往下载」）：方框 + 右上角向外箭头。
 *
 * **别用「箭头进托盘」的下载图形**：那和 `IconHide`（隐藏悬浮球）几乎一样，
 * 一眼分不出来。下载动作发生在浏览器里，所以用「打开外链」语义更准确。
 */
export function IconExternalLink(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M14.4 4.4h5.2v5.2" />
      <path d="M19.6 4.4 11 13" />
      <path d="M18.4 14v4.4a1.8 1.8 0 0 1-1.8 1.8H5.8A1.8 1.8 0 0 1 4 18.4V7.6a1.8 1.8 0 0 1 1.8-1.8H10" />
    </Svg>
  );
}

/**
 * 贴到桌面：画中画（大框右下开个口 + 里面一张小窗）。
 *
 * 表达的是「把这条内容贴成一张浮在桌面上的小窗」。**别用图钉**：图钉读作「钉住 / 固定」，
 * 而这一行左边就是「文件」的折角纸图标，再来一张纸（便签 / 文件）会更分不清。
 */
export function IconFloatCard(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M21 9V6.2A2.2 2.2 0 0 0 18.8 4H5.2A2.2 2.2 0 0 0 3 6.2v11.6a2.2 2.2 0 0 0 2.2 2.2h3.2" />
      <rect x="12" y="12" width="9" height="9" rx="2.2" />
    </Svg>
  );
}

/** 面板桌面置顶：把窗口推到最上层（顶部横线 + 向上箭头） */
export function IconKeepOnTop(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M4.4 3.8h15.2" />
      <path d="M12 20.4V8.4" />
      <path d="m7.6 12.8 4.4-4.4 4.4 4.4" />
    </Svg>
  );
}

/**
 * 清空全部历史（橡皮擦，擦掉整叠记录）。
 *
 * **别用扫帚**（旧版）：扫帚偏向「日常打扫」，语义糊；也**别用垃圾桶**——那是
 * 「删除这一条」的图形（`IconTrash`），清空全部和删一条必须一眼分开。
 */
export function IconEraser(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="m7 21-4.3-4.3c-1-1-1-2.5 0-3.4l9.6-9.6c1-1 2.5-1 3.4 0l5.6 5.6c1 1 1 2.5 0 3.4L13 21" />
      <path d="M22 21H7" />
      <path d="m5 11 9 9" />
    </Svg>
  );
}

/** 历史条目上限（时钟回溯） */
export function IconHistory(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M3.6 12a8.4 8.4 0 1 0 2.6-6.1" />
      <path d="M3.4 4.6v4.2h4.2" />
      <path d="M12 8.2V12l2.9 1.7" />
    </Svg>
  );
}

/** 返回 */
export function IconArrowBack(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M19.4 12H4.8" />
      <path d="m11 5.4-6.2 6.6 6.2 6.6" />
    </Svg>
  );
}

/** 时间（meta 行里的小时钟） */
export function IconSchedule(props: IconProps) {
  return (
    <Svg {...props}>
      <circle cx="12" cy="12" r="8.6" />
      <path d="M12 7.4V12l3 1.8" />
    </Svg>
  );
}

/**
 * 来源：剪贴板（普通剪贴板，不带箭头）。
 *
 * 查看窗口 meta 条上的来源徽章用它，**不要用地球 / IconPublic**：地球是「网络 / 公开」，
 * 拿来表示「这条是从别的程序复制进来的」属于乱用。
 * 品牌图形 `IconClipboardArrow`（带出箭头）只给品牌位用，两者别混。
 */
export function IconClipboard(props: IconProps) {
  return (
    <Svg {...props}>
      <rect x="5" y="5" width="14" height="16.2" rx="2.4" />
      <path d="M9.2 2.8h5.6a1.2 1.2 0 0 1 1.2 1.2v1.4H8V4a1.2 1.2 0 0 1 1.2-1.2Z" />
    </Svg>
  );
}

/** 放大 */
export function IconZoomIn(props: IconProps) {
  return (
    <Svg {...props}>
      <circle cx="11" cy="11" r="7" />
      <path d="M11 8.4v5.2" />
      <path d="M8.4 11h5.2" />
      <path d="m16.4 16.4 4.1 4.1" />
    </Svg>
  );
}

/** 缩小 */
export function IconZoomOut(props: IconProps) {
  return (
    <Svg {...props}>
      <circle cx="11" cy="11" r="7" />
      <path d="M8.4 11h5.2" />
      <path d="m16.4 16.4 4.1 4.1" />
    </Svg>
  );
}

/** 适应窗口 */
export function IconFitScreen(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M3.6 9V5.4A1.8 1.8 0 0 1 5.4 3.6H9" />
      <path d="M15 3.6h3.6a1.8 1.8 0 0 1 1.8 1.8V9" />
      <path d="M20.4 15v3.6a1.8 1.8 0 0 1-1.8 1.8H15" />
      <path d="M9 20.4H5.4a1.8 1.8 0 0 1-1.8-1.8V15" />
    </Svg>
  );
}

/**
 * 原始尺寸 / 100%（对角外扩箭头）。
 *
 * 和 `IconFitScreen`（四角**内收**＝缩进窗口里）配套：一个往外撑到实际大小、
 * 一个往里收到窗口内。**别再用取景框**（旧版是两个交叉的框）—— 它和
 * `IconScan`（区域截图的取景框）在同一个小缩放条上根本分不出来。
 */
export function IconActualSize(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="m4.2 4.2 5.6 5.6" />
      <path d="M4.2 9.8V4.2h5.6" />
      <path d="m19.8 19.8-5.6-5.6" />
      <path d="M19.8 14.2v5.6h-5.6" />
    </Svg>
  );
}

/** 全选文本 */
export function IconSelectAll(props: IconProps) {
  return (
    <Svg {...props}>
      <rect x="3.6" y="3.6" width="16.8" height="16.8" rx="2.4" strokeDasharray="3.4 2.6" />
      <path d="M8.6 12.2 11 14.6l4.4-4.6" />
    </Svg>
  );
}
