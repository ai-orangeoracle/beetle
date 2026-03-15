import { useContext } from 'react'
import type { UnsavedContextValue } from '../contexts/UnsavedContext'
import { UnsavedContext } from '../contexts/UnsavedContext'

export function useUnsaved(): UnsavedContextValue {
  return useContext(UnsavedContext)
}
