import { createContext, useContext } from 'react'

export type ToastVariant = 'success' | 'warning' | 'error'

export type ToastPosition = 'top-center' | 'top-right' | 'top-left'

export interface ToastOptions {
  variant?: ToastVariant
  position?: ToastPosition
  autoHideDuration?: number
}

export interface ToastContextValue {
  showToast: (message: string, options?: ToastOptions) => void
}

export const ToastContext = createContext<ToastContextValue | null>(null)

export function useToastContext(): ToastContextValue {
  const ctx = useContext(ToastContext)
  if (!ctx) throw new Error('useToast must be used within ToastProvider')
  return ctx
}
