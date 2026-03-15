import Stack from '@mui/material/Stack'
import type { ReactNode } from 'react'

/** 表单项纵向排列，统一间距（theme spacing 2）。 */
export function FormFieldStack({ children }: { children: ReactNode }) {
  return (
    <Stack spacing={2} sx={{ '& .MuiTextField-root': { minWidth: 0 } }}>
      {children}
    </Stack>
  )
}
