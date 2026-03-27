/**
 * 主内容区在 `var(--surface)` 上叠 PCB 丝印：正交细线网格 + 双层焊盘点阵 + 稀疏主色格点 + 对角走线；
 * 金属丝印感斜向高光层随 keyframes 缓慢移动；焊盘等层同步漂移（网格固定）。
 * 电路图 / CPU 轮廓见 [components/PcbDecorOverlay.tsx]（SVG + currentColor）。
 * `prefers-reduced-motion: reduce` 下关闭动画。
 */
export const MAIN_SURFACE_PCB_SX = {
  "@keyframes pcbMainGridDrift": {
    "0%": {
      backgroundPosition:
        "0px 0px, 0px 0px, 0px 0px, 8px 8px, 0px 0px, 0px 0px, 0% 0%",
    },
    "100%": {
      backgroundPosition:
        "0px 0px, 0px 0px, 32px 32px, 24px 24px, 128px 128px, 64px 64px, 100% 100%",
    },
  },
  backgroundColor: "var(--surface)",
  backgroundImage: [
    // 丝印竖线（中心略提亮，金属边缘感）
    "repeating-linear-gradient(90deg, color-mix(in srgb, var(--foreground) 5%, transparent) 0, color-mix(in srgb, white 12%, var(--foreground) 9%, transparent) 0.7px, color-mix(in srgb, var(--foreground) 6%, transparent) 1.4px, transparent 2px, transparent 32px)",
    // 丝印横线
    "repeating-linear-gradient(0deg, color-mix(in srgb, var(--foreground) 5%, transparent) 0, color-mix(in srgb, white 12%, var(--foreground) 9%, transparent) 0.7px, color-mix(in srgb, var(--foreground) 6%, transparent) 1.4px, transparent 2px, transparent 32px)",
    // 主焊盘 32px
    "radial-gradient(circle, color-mix(in srgb, var(--foreground) 14%, transparent) 1.5px, transparent 2px)",
    // 细过孔 16px
    "radial-gradient(circle, color-mix(in srgb, var(--foreground) 7%, transparent) 0.85px, transparent 1.25px)",
    // 大格主色点缀
    "radial-gradient(circle at 50% 50%, color-mix(in srgb, var(--primary) 14%, transparent) 2px, transparent 5px)",
    // 极淡对角走线
    "repeating-linear-gradient(135deg, color-mix(in srgb, var(--foreground) 3%, transparent) 0 1px, transparent 1px 48px)",
    // 金属高光刷痕（慢移）
    "linear-gradient(122deg, transparent 0%, color-mix(in srgb, var(--foreground) 2%, transparent) 42%, color-mix(in srgb, white 22%, var(--foreground) 9%) 50%, color-mix(in srgb, var(--foreground) 2.5%, transparent) 58%, transparent 100%)",
  ].join(", "),
  backgroundSize:
    "32px 32px, 32px 32px, 32px 32px, 16px 16px, 128px 128px, 48px 48px, 220% 220%",
  backgroundPosition:
    "0px 0px, 0px 0px, 0px 0px, 8px 8px, 0px 0px, 0px 0px, 0% 0%",
  animation: "pcbMainGridDrift 40s linear infinite",
  "@media (prefers-reduced-motion: reduce)": {
    animation: "none",
  },
} as const;

/**
 * 侧栏选中项丝印：用绝对定位子节点绘制（不用 ::before，避免与 MUI ButtonBase/ListItem 伪元素冲突）。
 * 对比度刻意抬高，否则叠在主色浅底上几乎不可见。
 */
export function sidebarNavSelectedPcbOverlaySx() {
  return {
    backgroundImage: [
      "repeating-linear-gradient(90deg, color-mix(in srgb, var(--foreground) 22%, transparent) 0, color-mix(in srgb, white 14%, var(--foreground) 18%, transparent) 0.8px, color-mix(in srgb, var(--foreground) 14%, transparent) 1.45px, transparent 2px, transparent 24px)",
      "repeating-linear-gradient(0deg, color-mix(in srgb, var(--foreground) 22%, transparent) 0, color-mix(in srgb, white 14%, var(--foreground) 18%, transparent) 0.8px, color-mix(in srgb, var(--foreground) 14%, transparent) 1.45px, transparent 2px, transparent 24px)",
      "radial-gradient(circle, color-mix(in srgb, var(--primary) 55%, transparent) 1.5px, transparent 2.4px)",
      "radial-gradient(circle, color-mix(in srgb, var(--foreground) 18%, transparent) 0.7px, transparent 1.1px)",
      "repeating-linear-gradient(135deg, color-mix(in srgb, var(--foreground) 14%, transparent) 0 1px, transparent 1px 32px)",
    ].join(", "),
    backgroundSize: "24px 24px, 24px 24px, 24px 24px, 12px 12px, 32px 32px",
    backgroundPosition: "0 0, 0 0, 0 0, 6px 6px, 0 0",
    backgroundRepeat: "repeat, repeat, repeat, repeat, repeat",
  } as const;
}
