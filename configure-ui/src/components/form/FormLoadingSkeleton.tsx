import Box from '@mui/material/Box'
import Skeleton from '@mui/material/Skeleton'

const radius = 'var(--radius-control)'

/** 单行「标签 + 输入框」骨架，模拟表单项。 */
function FieldRowSkeleton() {
  return (
    <Box sx={{ display: 'flex', flexDirection: 'column', gap: 1 }}>
      <Skeleton
        variant="rounded"
        height={14}
        width={80}
        animation="wave"
        sx={{ borderRadius: 1 }}
      />
      <Skeleton
        variant="rounded"
        height={40}
        animation="wave"
        sx={{ borderRadius: radius }}
      />
    </Box>
  )
}

/** 配置页整页加载时的占位骨架，模拟多列表单布局。 */
export function FormLoadingSkeleton() {
  return (
    <Box sx={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
      <FieldRowSkeleton />
      <FieldRowSkeleton />
      <FieldRowSkeleton />
      <Box sx={{ display: 'flex', flexDirection: 'column', gap: 1 }}>
        <Skeleton variant="rounded" height={14} width={120} animation="wave" sx={{ borderRadius: 1 }} />
        <Skeleton variant="rounded" height={96} animation="wave" sx={{ borderRadius: radius }} />
      </Box>
    </Box>
  )
}

/** 区块内加载占位（如 Soul/User 文本框区、技能列表、系统日志），替代纯文字「加载中…」。 */
export function SectionLoadingSkeleton() {
  return (
    <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
      <Skeleton variant="rounded" height={14} width="40%" animation="wave" sx={{ borderRadius: 1 }} />
      <Skeleton variant="rounded" height={80} animation="wave" sx={{ borderRadius: radius }} />
      <Skeleton variant="rounded" height={14} width="60%" animation="wave" sx={{ borderRadius: 1 }} />
      <Skeleton variant="rounded" height={48} animation="wave" sx={{ borderRadius: radius }} />
    </Box>
  )
}
