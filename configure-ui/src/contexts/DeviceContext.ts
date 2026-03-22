import { createContext } from 'react'

const STORAGE_BASE_URL = 'pocket_crayfish_device_base_url'
const STORAGE_PAIRING_CODE = 'pocket_crayfish_pairing_code'
const STORAGE_PAIRING_CODE_TS = 'pocket_crayfish_pairing_code_ts'
const PAIRING_CODE_EXPIRE_MS = 60 * 60 * 1000 // 1 hour

export function getStoredBaseUrl(): string {
  if (typeof window === 'undefined') return ''
  return window.localStorage.getItem(STORAGE_BASE_URL) ?? ''
}

export function getStoredPairingCode(): string {
  if (typeof window === 'undefined') return ''
  const code = window.localStorage.getItem(STORAGE_PAIRING_CODE)
  const ts = window.localStorage.getItem(STORAGE_PAIRING_CODE_TS)
  if (!code || !ts) return ''
  const elapsed = Date.now() - parseInt(ts, 10)
  if (elapsed > PAIRING_CODE_EXPIRE_MS) {
    window.localStorage.removeItem(STORAGE_PAIRING_CODE)
    window.localStorage.removeItem(STORAGE_PAIRING_CODE_TS)
    return ''
  }
  return code
}

export function setStoredBaseUrl(value: string): void {
  if (typeof window === 'undefined') return
  if (value) window.localStorage.setItem(STORAGE_BASE_URL, value)
  else window.localStorage.removeItem(STORAGE_BASE_URL)
}

export function setStoredPairingCode(value: string): void {
  if (typeof window === 'undefined') return
  if (value) {
    window.localStorage.setItem(STORAGE_PAIRING_CODE, value)
    window.localStorage.setItem(STORAGE_PAIRING_CODE_TS, Date.now().toString())
  } else {
    window.localStorage.removeItem(STORAGE_PAIRING_CODE)
    window.localStorage.removeItem(STORAGE_PAIRING_CODE_TS)
  }
}

export interface DeviceContextValue {
  baseUrl: string
  pairingCode: string
  setBaseUrl: (v: string) => void
  setPairingCode: (v: string) => void
}

export const DeviceContext = createContext<DeviceContextValue | null>(null)
