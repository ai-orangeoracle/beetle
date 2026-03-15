import Box from "@mui/material/Box";

/**
 * Logo icon: beetle (甲虫) — wings spread diagonally, body ∧, head, antennae.
 * 24×24 viewBox; use fontSize (e.g. var(--icon-size-sm)) to scale.
 */
export function BeetleIcon({
  sx,
  ...rest
}: { sx?: object } & React.SVGAttributes<SVGSVGElement>) {
  return (
    <Box component="span" sx={{ display: "inline-flex", fontSize: "inherit", flexShrink: 0, ...sx }}>
      <svg
        xmlns="http://www.w3.org/2000/svg"
        viewBox="0 0 24 24"
        fill="currentColor"
        width="1em"
        height="1em"
        {...rest}
      >
      {/* Left wing: diagonal, soft outer curve */}
      <path d="M12 6.2 L12 18.8 L5 17 Q2 13 3.5 8.5 Q6 5 12 6.2 Z" />
      {/* Right wing: mirror */}
      <path d="M12 6.2 L12 18.8 L19 17 Q22 13 20.5 8.5 Q18 5 12 6.2 Z" />
      {/* Body: thin ∧, slight taper & rounded tip, theme color */}
      <path d="M12 6.8 L11 18.5 Q12 19.4 13 18.5 Z" fill="var(--primary)" />
      {/* Head */}
      <circle cx="12" cy="5" r="2.2" />
      {/* Antennae: slight curve, small club */}
      <path
        d="M9.6 3.6 Q8 2 6.5 1.8 M14.4 3.6 Q16 2 17.5 1.8"
        stroke="currentColor"
        strokeWidth="0.8"
        fill="none"
        strokeLinecap="round"
      />
      <circle cx="6.5" cy="1.8" r="0.6" fill="currentColor" />
      <circle cx="17.5" cy="1.8" r="0.6" fill="currentColor" />
      </svg>
    </Box>
  );
}
