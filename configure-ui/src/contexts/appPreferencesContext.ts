import { createContext } from 'react'
import type { ThemeBrand, ThemeMode } from '../config/themeTokens'

export type AppLanguage = 'zh-CN' | 'en-US'

export function isLanguage(value: unknown): value is AppLanguage {
  return value === 'zh-CN' || value === 'en-US'
}

export interface AppPreferencesState {
  language: AppLanguage
  themeMode: ThemeMode
  themeBrand: ThemeBrand
}

export interface AppPreferencesContextValue extends AppPreferencesState {
  setLanguage: (language: AppLanguage) => void
  setThemeMode: (mode: ThemeMode) => void
  setThemeBrand: (brand: ThemeBrand) => void
}

export const defaultPreferences: AppPreferencesState = {
  language: 'zh-CN',
  themeMode: 'dark',
  themeBrand: 'firmware',
}

export const AppPreferencesContext = createContext<AppPreferencesContextValue | null>(null)
