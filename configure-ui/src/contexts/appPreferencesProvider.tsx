import { useCallback, useEffect, useMemo, useState, type PropsWithChildren } from 'react'
import type { ThemeBrand, ThemeMode } from '../config/themeTokens'
import {
  AppPreferencesContext,
  defaultPreferences,
  isLanguage,
  type AppPreferencesState,
} from './appPreferencesContext'

const STORAGE_KEY = 'ai-job-market.app.preferences.v1'

function isThemeMode(value: unknown): value is ThemeMode {
  return value === 'light' || value === 'dark'
}

function isThemeBrand(value: unknown): value is ThemeBrand {
  return (
    value === 'blue' ||
    value === 'teal' ||
    value === 'orange' ||
    value === 'firmware'
  )
}

function loadPreferences(): AppPreferencesState {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return defaultPreferences
    const parsed: unknown = JSON.parse(raw)
    if (!parsed || typeof parsed !== 'object') return defaultPreferences
    const record = parsed as Record<string, unknown>
    return {
      language: isLanguage(record.language) ? record.language : defaultPreferences.language,
      themeMode: isThemeMode(record.themeMode) ? record.themeMode : defaultPreferences.themeMode,
      themeBrand: isThemeBrand(record.themeBrand) ? record.themeBrand : defaultPreferences.themeBrand,
    }
  } catch {
    return defaultPreferences
  }
}

export function AppPreferencesProvider({ children }: PropsWithChildren) {
  const [state, setState] = useState<AppPreferencesState>(loadPreferences)

  useEffect(() => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(state))
  }, [state])

  const setLanguage = useCallback((language: AppPreferencesState['language']) => {
    setState((prev) => ({ ...prev, language }))
  }, [])

  const setThemeMode = useCallback((themeMode: ThemeMode) => {
    setState((prev) => ({ ...prev, themeMode }))
  }, [])

  const setThemeBrand = useCallback((themeBrand: ThemeBrand) => {
    setState((prev) => ({ ...prev, themeBrand }))
  }, [])

  const value = useMemo(
    () => ({ ...state, setLanguage, setThemeMode, setThemeBrand }),
    [state, setLanguage, setThemeMode, setThemeBrand],
  )

  return (
    <AppPreferencesContext.Provider value={value}>
      {children}
    </AppPreferencesContext.Provider>
  )
}
