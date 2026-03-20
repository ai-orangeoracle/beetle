import { useEffect } from 'react'
import Box from '@mui/material/Box'
import Typography from '@mui/material/Typography'
import CheckCircleOutlined from '@mui/icons-material/CheckCircleOutlined'
import ErrorOutline from '@mui/icons-material/ErrorOutline'

type Status = 'ok' | 'fail'

interface SaveFeedbackProps {
  status: Status
  message: string
  /** 成功时多少毫秒后自动消失，0 不自动消失 */
  autoDismissMs?: number
  onDismiss?: () => void
}

export function SaveFeedback({
  status,
  message,
  autoDismissMs = 3000,
  onDismiss,
}: SaveFeedbackProps) {
  const isOk = status === 'ok'

  useEffect(() => {
    if (!isOk || autoDismissMs <= 0 || !onDismiss) return
    const id = setTimeout(onDismiss, autoDismissMs)
    return () => clearTimeout(id)
  }, [isOk, autoDismissMs, onDismiss])

  return (
    <Box
      role="status"
      aria-live="polite"
      aria-atomic
      sx={{
        display: 'flex',
        alignItems: 'center',
        gap: 1,
        mt: 2,
        p: 1.5,
        borderRadius: 'var(--radius-control)',
        bgcolor: isOk
          ? 'color-mix(in srgb, var(--semantic-success) 6%, transparent)'
          : 'color-mix(in srgb, var(--semantic-danger) 6%, transparent)',
        border: '1px solid',
        borderColor: isOk
          ? 'color-mix(in srgb, var(--semantic-success) 16%, transparent)'
          : 'color-mix(in srgb, var(--semantic-danger) 12%, transparent)',
        transition: 'opacity var(--transition-duration) ease',
      }}
    >
      {isOk ? (
        <CheckCircleOutlined sx={{ fontSize: 'var(--icon-size-md)', color: 'var(--semantic-success)' }} aria-hidden />
      ) : (
        <ErrorOutline sx={{ fontSize: 'var(--icon-size-md)', color: 'var(--semantic-danger)' }} aria-hidden />
      )}
      <Typography
        variant="body2"
        sx={{
          color: isOk ? 'var(--semantic-success)' : 'var(--semantic-danger)',
          fontWeight: 500,
        }}
      >
        {message}
      </Typography>
    </Box>
  )
}
