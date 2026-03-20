import type { AppLanguage } from '../contexts/appPreferencesContext'

function normalizeLanguageTag(raw: string): string {
  return raw.trim().toLowerCase()
}

export function resolveLanguageFromBrowser(
  fallback: AppLanguage = 'zh-CN',
): AppLanguage {
  if (typeof navigator === 'undefined') return fallback

  const candidates = [
    ...(Array.isArray(navigator.languages) ? navigator.languages : []),
    navigator.language,
  ].filter((value): value is string => Boolean(value))

  for (const candidate of candidates) {
    const normalized = normalizeLanguageTag(candidate)
    if (normalized.startsWith('zh')) return 'zh-CN'
    if (normalized.startsWith('en')) return 'en-US'
  }

  return fallback
}
