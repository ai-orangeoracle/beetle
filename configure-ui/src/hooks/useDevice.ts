import { useContext } from 'react'
import { DeviceContext } from '../contexts/DeviceContext'

export function useDevice() {
  const ctx = useContext(DeviceContext)
  if (!ctx) throw new Error('useDevice must be used within DeviceProvider')
  return ctx
}
