import { useId } from "react";
import Box from "@mui/material/Box";

/**
 * 主内容区装饰：电路图式走线、原理图符号与 QFP 封装 CPU 轮廓。
 * 使用 SVG pattern + currentColor，随主题明暗与主色变化；不参与交互与焦点。
 */
export function PcbDecorOverlay() {
  const uid = useId().replace(/:/g, "");
  const tracePat = `pcb-t-${uid}`;
  const chipPat = `pcb-c-${uid}`;

  return (
    <Box
      aria-hidden
      sx={{
        position: "absolute",
        inset: 0,
        zIndex: 0,
        pointerEvents: "none",
        overflow: "hidden",
        "& svg": { display: "block" },
      }}
    >
      {/* 走线 / 原理图：偏 foreground */}
      <svg
        width="100%"
        height="100%"
        xmlns="http://www.w3.org/2000/svg"
        style={{
          position: "absolute",
          inset: 0,
          color:
            "color-mix(in srgb, var(--foreground) 11%, transparent)",
          opacity: 0.92,
        }}
      >
        <defs>
          <pattern
            id={tracePat}
            width={320}
            height={320}
            patternUnits="userSpaceOnUse"
          >
            <g
              fill="none"
              stroke="currentColor"
              strokeWidth={1}
              strokeLinecap="square"
              strokeLinejoin="miter"
            >
              <path d="M 0 88 L 96 88 L 96 152 L 168 152 L 168 56 L 248 56" />
              <path d="M 320 72 L 220 72 L 220 200 L 140 200" />
              <path d="M 52 320 L 52 248 L 128 248 L 128 188" />
              <path d="M 260 320 L 260 260 L 320 260" />
              <circle cx={196} cy={124} r={2.2} fill="currentColor" />
              <path d="M 196 124 L 196 108" />
              <path d="M 196 124 L 212 124" />
              <path
                d="M 72 228 h 6 l 4 5 l -6 5 l 6 5 l -4 5 h 6"
                strokeWidth={0.9}
              />
              <path d="M 270 140 v 18 M 282 140 v 18 M 270 149 h 12" />
            </g>
          </pattern>
        </defs>
        <rect width="100%" height="100%" fill={`url(#${tracePat})`} />
      </svg>
      {/* 芯片封装：偏 primary，略淡 */}
      <svg
        width="100%"
        height="100%"
        xmlns="http://www.w3.org/2000/svg"
        style={{
          position: "absolute",
          inset: 0,
          color: "color-mix(in srgb, var(--primary) 16%, transparent)",
          opacity: 0.85,
        }}
      >
        <defs>
          <pattern
            id={chipPat}
            width={320}
            height={320}
            patternUnits="userSpaceOnUse"
          >
            <g
              fill="none"
              stroke="currentColor"
              strokeWidth={1.1}
              strokeLinecap="square"
            >
              <rect x={116} y={116} width={88} height={88} rx={5} />
              <rect
                x={132}
                y={132}
                width={56}
                height={56}
                rx={2}
                strokeWidth={0.75}
                opacity={0.65}
              />
              <circle cx={128} cy={128} r={2.5} fill="currentColor" />
              {[
                124, 136, 148, 160, 172, 184, 196,
              ].map((y) => (
                <line key={`L${y}`} x1={108} y1={y} x2={116} y2={y} />
              ))}
              {[
                124, 136, 148, 160, 172, 184, 196,
              ].map((y) => (
                <line key={`R${y}`} x1={204} y1={y} x2={212} y2={y} />
              ))}
              {[
                124, 136, 148, 160, 172, 184, 196,
              ].map((x) => (
                <line key={`T${x}`} x1={x} y1={108} x2={x} y2={116} />
              ))}
              {[
                124, 136, 148, 160, 172, 184, 196,
              ].map((x) => (
                <line key={`B${x}`} x1={x} y1={204} x2={x} y2={212} />
              ))}
            </g>
          </pattern>
        </defs>
        <rect width="100%" height="100%" fill={`url(#${chipPat})`} />
      </svg>
    </Box>
  );
}
