import { useCallback, useState } from 'react'
import type { SaveStatus } from '../types/asyncState'

function mapError(error: string | undefined, t: (key: string) => string): string {
  if (!error) return ''
  if (error.startsWith('device.') || error.startsWith('config.')) return t(error)
  return error
}

export function useSaveFeedback(t: (key: string) => string) {
  const [status, setStatus] = useState<SaveStatus>('idle')
  const [error, setError] = useState('')

  const begin = useCallback(() => {
    setStatus('saving')
    setError('')
  }, [])

  const fail = useCallback((message: string) => {
    setStatus('fail')
    setError(message)
  }, [])

  const finishFromResult = useCallback(
    (result: { ok: boolean; error?: string }) => {
      setStatus(result.ok ? 'ok' : 'fail')
      setError(mapError(result.error, t))
    },
    [t],
  )

  const dismiss = useCallback(() => {
    setStatus('idle')
    setError('')
  }, [])

  return { status, error, begin, fail, finishFromResult, dismiss }
}
