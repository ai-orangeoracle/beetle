import { useCallback, useState } from 'react'

/** 聚焦时明文、失焦时密码（仅 focus/blur，不含 hover）。返回当前是否明文及要绑到 input 的事件。 */
export function useRevealedPassword() {
  const [revealed, setRevealed] = useState(false)

  const onFocus = useCallback(() => setRevealed(true), [])
  const onBlur = useCallback(() => setRevealed(false), [])

  return {
    type: revealed ? 'text' : 'password' as const,
    inputProps: { onFocus, onBlur },
  }
}

/** 多字段共用：按 key 记录当前处于「明文」的字段（仅聚焦时显示、失焦时隐藏，不含 hover）。 */
export function useRevealedPasswordFields() {
  const [revealed, setRevealed] = useState<Set<string>>(new Set())

  const add = useCallback((key: string) => {
    setRevealed((prev) => (prev.has(key) ? prev : new Set(prev).add(key)))
  }, [])
  const remove = useCallback((key: string) => {
    setRevealed((prev) => {
      if (!prev.has(key)) return prev
      const next = new Set(prev)
      next.delete(key)
      return next
    })
  }, [])

  const isRevealed = useCallback((key: string) => revealed.has(key), [revealed])
  const getRevealHandlers = useCallback(
    (key: string) => ({
      onFocus: () => add(key),
      onBlur: () => remove(key),
    }),
    [add, remove],
  )

  return { isRevealed, getRevealHandlers }
}
