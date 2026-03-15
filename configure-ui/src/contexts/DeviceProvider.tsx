import { useCallback, useEffect, useMemo, useState } from 'react'
import { getPairingCode } from '../api/endpoints/pairingCode'
import { setDeviceStatus } from '../store/deviceStatusStore'
import {
  DeviceContext,
  getStoredBaseUrl,
  getStoredPairingCode,
  setStoredBaseUrl,
  setStoredPairingCode,
} from './DeviceContext'

export function DeviceProvider({ children }: { children: React.ReactNode }) {
  const [baseUrl, setBaseUrlState] = useState(getStoredBaseUrl)
  const [pairingCode, setPairingCodeState] = useState(getStoredPairingCode)

  const setBaseUrl = useCallback((v: string) => {
    setBaseUrlState(v)
    setStoredBaseUrl(v)
  }, [])

  const setPairingCode = useCallback((v: string) => {
    setPairingCodeState(v)
    setStoredPairingCode(v)
  }, [])

  useEffect(() => {
    const url = baseUrl?.trim()
    if (!url) {
      setDeviceStatus('none', null)
      return
    }
    setDeviceStatus('checking', null)
    let cancelled = false
    getPairingCode(url).then((res) => {
      if (cancelled) return
      if (res.ok && res.data != null) {
        setDeviceStatus('reachable', res.data.code_set)
      } else {
        setDeviceStatus('unreachable', null)
      }
    })
    return () => {
      cancelled = true
    }
  }, [baseUrl])

  const value = useMemo(
    () => ({ baseUrl, pairingCode, setBaseUrl, setPairingCode }),
    [baseUrl, pairingCode, setBaseUrl, setPairingCode],
  )

  return <DeviceContext.Provider value={value}>{children}</DeviceContext.Provider>
}
