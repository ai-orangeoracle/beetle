import { useCallback, useMemo, useState } from 'react'
import { API_ERROR } from '../api/client'
import type {
  AppConfig,
  LlmConfigSegment,
  ChannelsConfigSegment,
  SystemConfigSegment,
} from '../types/appConfig'
import { ConfigContext } from './ConfigContext'
import { useDeviceApi } from '../hooks/useDeviceApi'

/** i18n keys for config load errors; 与顶栏横幅重复的配对/设备类不展示，由 DeviceBanner 处理 */
const ERROR_KEY_NO_BASE = 'device.bannerNeedDevice'
const ERROR_KEY_LOAD_FAILED = 'config.errorLoadFailed'

/** API 返回的原始配对/设备类文案，子页不重复展示 */
function isDeviceOrPairingHint(err: string | undefined): boolean {
  if (!err) return false
  const s = err.trim()
  return (
    s === '请先设置配对码' ||
    s === '请先填写设备地址' ||
    s === '配对码错误' ||
    s === 'Please set pairing code first' ||
    s === 'Please enter device URL' ||
    s === 'Wrong pairing code'
  )
}

export function ConfigProvider({ children }: { children: React.ReactNode }) {
  const { api, ready } = useDeviceApi()
  const [config, setConfig] = useState<AppConfig | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const clearCachedConfig = useCallback(() => {
    setConfig(null)
    setError(null)
  }, [])

  const loadConfig = useCallback(async () => {
    if (!ready) {
      setError(ERROR_KEY_NO_BASE)
      return
    }
    setLoading(true)
    setError(null)
    const res = await api.config.get()
    setLoading(false)
    if (res.ok && res.data != null && typeof res.data === 'object') {
      setConfig(res.data as AppConfig)
    } else {
      setError(
        res.error === API_ERROR.NO_BASE_URL
          ? ERROR_KEY_NO_BASE
          : isDeviceOrPairingHint(res.error ?? '')
            ? null
            : ERROR_KEY_LOAD_FAILED,
      )
      setConfig(null)
    }
  }, [api.config, ready])

  const saveLlm = useCallback(
    async (body: LlmConfigSegment): Promise<{ ok: boolean; error?: string }> => {
      const res = await api.config.saveLlm(body)
      if (res.ok)
        setConfig((prev) =>
          prev ? { ...prev, ...body } : null,
        )
      const err =
        res.error === API_ERROR.PAIRING_REQUIRED ? 'device.pairingCodeRequired' : res.error
      return { ok: res.ok ?? false, error: err }
    },
    [api.config],
  )

  const saveChannels = useCallback(
    async (body: ChannelsConfigSegment): Promise<{ ok: boolean; error?: string }> => {
      const res = await api.config.saveChannels(body)
      if (res.ok)
        setConfig((prev) =>
          prev ? { ...prev, ...body } : null,
        )
      const err =
        res.error === API_ERROR.PAIRING_REQUIRED ? 'device.pairingCodeRequired' : res.error
      return { ok: res.ok ?? false, error: err }
    },
    [api.config],
  )

  const saveSystem = useCallback(
    async (body: SystemConfigSegment): Promise<{ ok: boolean; error?: string }> => {
      const res = await api.config.saveSystem(body)
      if (res.ok)
        setConfig((prev) =>
          prev ? { ...prev, ...body } : null,
        )
      const err =
        res.error === API_ERROR.PAIRING_REQUIRED ? 'device.pairingCodeRequired' : res.error
      return { ok: res.ok ?? false, error: err }
    },
    [api.config],
  )

  const value = useMemo(
    () => ({
      config,
      loading,
      error,
      loadConfig,
      clearCachedConfig,
      saveLlm,
      saveChannels,
      saveSystem,
    }),
    [config, loading, error, loadConfig, clearCachedConfig, saveLlm, saveChannels, saveSystem],
  )

  return <ConfigContext.Provider value={value}>{children}</ConfigContext.Provider>
}
