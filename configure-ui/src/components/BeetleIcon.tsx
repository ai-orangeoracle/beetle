import type { ImgHTMLAttributes } from "react";
import Box from "@mui/material/Box";
import type { SxProps, Theme } from "@mui/material/styles";

/**
 * App logo: uses `public/logo.png` (served as `/logo.png`).
 * 应用 Logo：使用 `public/logo.png`（通过 `/logo.png` 访问）。
 */
export function BeetleIcon({
  sx,
  ...rest
}: {
  sx?: SxProps<Theme>;
} & Omit<ImgHTMLAttributes<HTMLImageElement>, "src" | "alt">) {
  return (
    <Box
      component="img"
      src="/logo.png"
      alt=""
      decoding="async"
      draggable={false}
      sx={{
        display: "block",
        width: "var(--icon-container-sm)",
        height: "var(--icon-container-sm)",
        objectFit: "contain",
        flexShrink: 0,
        ...sx,
      }}
      {...rest}
    />
  );
}
