import Box from '@mui/material/Box'
import Typography from '@mui/material/Typography'
import type { ReactNode } from 'react'

/** 表单内子区块标题：左侧主色竖条 + 标题，与下方表单项间距统一。 */
export function FormSectionSub({
  title,
  children,
}: {
  title: string
  children: ReactNode
}) {
  return (
    <Box
      sx={{
        '&:not(:first-of-type)': { mt: 3 },
      }}
    >
      <Box
        sx={{
          display: 'flex',
          alignItems: 'center',
          gap: 1.5,
          mb: 1.5,
        }}
      >
        <Box
          sx={{
            width: 'var(--accent-line-width, 3px)',
            height: 16,
            borderRadius: 1,
            bgcolor: 'var(--primary)',
            opacity: 0.5,
          }}
        />
        <Typography
          variant="caption"
          sx={{
            fontWeight: 700,
            letterSpacing: 'var(--letter-spacing-label)',
            color: 'var(--muted)',
            textTransform: 'uppercase',
            fontSize: 'var(--font-size-overline)',
          }}
        >
          {title}
        </Typography>
      </Box>
      <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2 }}>{children}</Box>
    </Box>
  )
}
