/**
 * 固件 / 设备配置类面板：全宽、扁平、细边框（挂载模块感）。
 * SettingsSection、个性配置等共用，保证风格一致。
 */
export const CONFIG_PANEL_SX = {
  borderRadius: "var(--radius-card)",
  bgcolor: "var(--card)",
  border: "1px solid color-mix(in srgb, var(--border) 28%, transparent)",
  boxShadow: "none",
} as const
