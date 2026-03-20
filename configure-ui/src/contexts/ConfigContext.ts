import { createContext } from 'react'
import type {
  AppConfig,
  LlmConfigSegment,
  ChannelsConfigSegment,
  SystemConfigSegment,
} from '../types/appConfig'

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
}

export const ConfigContext = createContext<ConfigContextValue | null>(null)
