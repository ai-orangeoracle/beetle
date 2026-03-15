import { createContext } from 'react'

const STORAGE_BASE_URL = 'pocket_crayfish_device_base_url'
const STORAGE_PAIRING_CODE = 'pocket_crayfish_pairing_code'

export function getStoredBaseUrl(): string {
  if (typeof window === 'undefined') return ''
  return window.localStorage.getItem(STORAGE_BASE_URL) ?? ''
}

export function getStoredPairingCode(): string {
  if (typeof window === 'undefined') return ''
  return window.localStorage.getItem(STORAGE_PAIRING_CODE) ?? ''
}

export function setStoredBaseUrl(value: string): void {
  if (typeof window === 'undefined') return
  if (value) window.localStorage.setItem(STORAGE_BASE_URL, value)
  else window.localStorage.removeItem(STORAGE_BASE_URL)
}

export function setStoredPairingCode(value: string): void {
  if (typeof window === 'undefined') return
  if (value) window.localStorage.setItem(STORAGE_PAIRING_CODE, value)
  else window.localStorage.removeItem(STORAGE_PAIRING_CODE)
}

export interface DeviceContextValue {
  baseUrl: string
  pairingCode: string
  setBaseUrl: (v: string) => void
  setPairingCode: (v: string) => void
}

export const DeviceContext = createContext<DeviceContextValue | null>(null)
