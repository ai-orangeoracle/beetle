import { createContext } from 'react'
import type {
  AppConfig,
  LlmConfigSegment,
  ChannelsConfigSegment,
  SystemConfigSegment,
} from '../types/appConfig'
import type { DisplayConfig } from '../types/displayConfig'

export interface ConfigContextValue {
  config: AppConfig | null
  loading: boolean
  error: string | null
  loadConfig: () => Promise<void>
  refreshCachedConfig: () => Promise<{ ok: boolean; error?: string }>
  clearCachedConfig: () => void
  saveLlm: (body: LlmConfigSegment) => Promise<{ ok: boolean; error?: string }>
  saveChannels: (body: ChannelsConfigSegment) => Promise<{ ok: boolean; error?: string }>
  saveSystem: (body: SystemConfigSegment) => Promise<{ ok: boolean; error?: string }>
  displayConfig: DisplayConfig | null
  displayLoading: boolean
  displayError: string | null
  loadDisplayConfig: () => Promise<void>
  saveDisplayConfig: (
    body: DisplayConfig,
  ) => Promise<{ ok: boolean; error?: string; restartRequired?: boolean }>
}

export const ConfigContext = createContext<ConfigContextValue | null>(null)
