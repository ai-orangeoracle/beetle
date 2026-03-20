import Box from '@mui/material/Box'
import Stack from '@mui/material/Stack'
import Typography from '@mui/material/Typography'
import type { PropsWithChildren, ReactNode } from 'react'

interface SettingsSectionProps {
  icon: ReactNode
  label: string
  /** 区块下方、内容区上方的简短说明 */
  description?: string
  accessory?: ReactNode
  /**
   * 标题行（图标+标题+accessory）下方的全宽区域，例如保存结果。
   * Keeps the title row a single-line flex; avoids a tall right column next to the title.
   */
  belowTitleRow?: ReactNode
}

export function SettingsSection({
  icon,
  label,
  description,
  accessory,
  belowTitleRow,
  children,
}: PropsWithChildren<SettingsSectionProps>) {
  const titleRowMb = belowTitleRow ? 1 : description ? 1 : 2
  const belowRowMb = description ? 1 : 2

  return (
    <Box
      sx={{
        p: 2.5,
        borderRadius: 'var(--radius-card)',
        bgcolor: 'var(--card)',
        boxShadow: 'var(--shadow-subtle)',
        transition:
          'box-shadow var(--transition-duration) ease, transform var(--transition-duration) var(--ease-emphasized)',
        '&:hover': {
          boxShadow: 'var(--shadow-card-hover)',
          transform: 'translateY(var(--hover-lift-y, -2px))',
        },
      }}
    >
      <Stack
        direction="row"
        alignItems="center"
        justifyContent="space-between"
        flexWrap="wrap"
        gap={1.5}
        sx={{ mb: titleRowMb }}
      >
        <Stack direction="row" alignItems="center" spacing={1.5}>
          <Box sx={{ color: 'var(--muted)', display: 'flex', alignItems: 'center' }}>{icon}</Box>
          <Typography
            component="span"
            sx={{
              fontSize: 'var(--font-size-body-sm)',
              fontWeight: 700,
              letterSpacing: '0.03em',
              lineHeight: 'var(--line-height-tight)',
              color: 'var(--foreground)',
            }}
          >
            {label}
          </Typography>
        </Stack>
        {accessory}
      </Stack>
      {belowTitleRow ? <Box sx={{ mb: belowRowMb }}>{belowTitleRow}</Box> : null}
      {description && (
        <Typography
          variant="body2"
          sx={{
            color: 'var(--muted)',
            mb: 2,
            fontSize: 'var(--font-size-caption)',
            lineHeight: 'var(--line-height-normal)',
          }}
        >
          {description}
        </Typography>
      )}
      {children}
    </Box>
  )
}
