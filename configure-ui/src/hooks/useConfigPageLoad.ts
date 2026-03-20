import { useEffect, useRef } from 'react'

export function useConfigPageLoad(params: {
  hasConfig: boolean
  loading: boolean
  loadConfig: () => Promise<void>
}) {
  const { hasConfig, loading, loadConfig } = params
  const loadAttemptedRef = useRef(false)

  useEffect(() => {
    if (hasConfig) {
      loadAttemptedRef.current = false
      return
    }
    if (loading || loadAttemptedRef.current) return
    loadAttemptedRef.current = true
    loadConfig()
  }, [hasConfig, loading, loadConfig])
}
