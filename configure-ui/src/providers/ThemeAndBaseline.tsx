import { useEffect, useMemo, type PropsWithChildren } from "react";
import Box from "@mui/material/Box";
import CssBaseline from "@mui/material/CssBaseline";
import { ThemeProvider } from "@mui/material/styles";
import { createAppTheme } from "../theme/appTheme";
import { useAppPreferences } from "../hooks/useAppPreferences";
import i18n from "../i18n";

// 右上角主色渐变 + 两处光晕，几何层次柔和、偏职业感
const gradientBackground = {
  position: "fixed" as const,
  inset: 0,
  zIndex: 0,
  pointerEvents: "none" as const,
  backgroundColor: "var(--background)",
  backgroundImage: [
    "linear-gradient(225deg, color-mix(in srgb, var(--primary) 32%, transparent) 0%, color-mix(in srgb, var(--primary) 10%, transparent) 38%, transparent 72%)",
    "radial-gradient(ellipse 95% 75% at 108% -15%, color-mix(in srgb, var(--primary) 22%, transparent), transparent 52%)",
    "radial-gradient(ellipse 65% 55% at 98% 98%, color-mix(in srgb, var(--accent) 16%, transparent), transparent 48%)",
  ].join(", "),
  backgroundSize: "100% 100%, 100% 100%, 100% 100%",
  backgroundPosition: "0 0, 0 0, 0 0",
  backgroundRepeat: "no-repeat",
};

const shapeBase = {
  position: "fixed" as const,
  zIndex: 0,
  pointerEvents: "none" as const,
};

export function ThemeAndBaseline({ children }: PropsWithChildren) {
  const { language, themeMode, themeBrand } = useAppPreferences();
  const theme = useMemo(
    () => createAppTheme(themeMode, themeBrand),
    [themeMode, themeBrand],
  );

  useEffect(() => {
    void i18n.changeLanguage(language);
  }, [language]);

  useEffect(() => {
    document.documentElement.setAttribute("data-theme-mode", themeMode);
    document.documentElement.setAttribute("data-theme-brand", themeBrand);
  }, [themeMode, themeBrand]);

  return (
    <ThemeProvider theme={theme}>
      <CssBaseline />
      <Box aria-hidden sx={gradientBackground} />
      {/* 重叠几何：圆形 */}
      <Box
        aria-hidden
        sx={{
          ...shapeBase,
          width: 260,
          height: 260,
          borderRadius: "50%",
          top: -70,
          right: -50,
          backgroundColor: "color-mix(in srgb, var(--primary) 8%, transparent)",
        }}
      />
      <Box
        aria-hidden
        sx={{
          ...shapeBase,
          width: 150,
          height: 150,
          borderRadius: "50%",
          top: "42%",
          left: -35,
          backgroundColor: "color-mix(in srgb, var(--accent) 7%, transparent)",
        }}
      />
      <Box
        aria-hidden
        sx={{
          ...shapeBase,
          width: 100,
          height: 100,
          borderRadius: "50%",
          bottom: "18%",
          right: "22%",
          backgroundColor: "color-mix(in srgb, var(--primary) 6%, transparent)",
        }}
      />
      <Box
        aria-hidden
        sx={{
          ...shapeBase,
          width: 0,
          height: 0,
          borderLeft: "100px solid transparent",
          borderRight: "100px solid transparent",
          borderBottom:
            "168px solid color-mix(in srgb, var(--primary) 5%, transparent)",
          top: "26%",
          right: "18%",
          transform: "rotate(22deg)",
        }}
      />
      <Box
        aria-hidden
        sx={{
          ...shapeBase,
          width: 0,
          height: 0,
          borderLeft: "70px solid transparent",
          borderRight: "70px solid transparent",
          borderBottom:
            "120px solid color-mix(in srgb, var(--accent) 5%, transparent)",
          bottom: "22%",
          left: "12%",
          transform: "rotate(-12deg)",
        }}
      />
      <Box sx={{ position: "relative", zIndex: 1 }}>{children}</Box>
    </ThemeProvider>
  );
}
