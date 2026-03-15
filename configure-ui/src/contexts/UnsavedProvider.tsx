import { useCallback, useState } from 'react'
import { UnsavedContext } from './UnsavedContext'

export function UnsavedProvider({ children }: { children: React.ReactNode }) {
  const [dirty, setDirty] = useState(false)
  const value = { dirty, setDirty: useCallback((v: boolean) => setDirty(v), []) }
  return (
    <UnsavedContext.Provider value={value}>
      {children}
    </UnsavedContext.Provider>
  )
}
