import { createContext } from 'react'

export interface UnsavedContextValue {
  dirty: boolean
  setDirty: (value: boolean) => void
}

export const UnsavedContext = createContext<UnsavedContextValue>({
  dirty: false,
  setDirty: () => {},
})
