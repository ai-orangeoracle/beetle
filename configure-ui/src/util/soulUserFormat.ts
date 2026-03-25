/**
 * SOUL / USER 表单与设备端存储文本之间的序列化。
 * 保存为带版本标记的 JSON；设备原样写入 SPIFFS 并拼入 system prompt。
 */

export const SOUL_FORMAT_MARKER = '---beetle-soul-v1---'
export const USER_FORMAT_MARKER = '---beetle-user-v1---'

export const SOUL_TRAIT_KEYS = [
  'humorous',
  'serious',
  'warm',
  'concise',
  'chatty',
  'professional',
] as const
export type SoulTraitKey = (typeof SOUL_TRAIT_KEYS)[number]

export const SOUL_SKILL_KEYS = [
  'programming',
  'daily',
  'health',
  'finance',
  'creative',
  'general',
] as const
export type SoulSkillKey = (typeof SOUL_SKILL_KEYS)[number]

export type SoulTone = 'colloquial' | 'formal' | 'flex' | ''

export interface SoulFormState {
  name: string
  traits: string[]
  skills: string[]
  tone: SoulTone
  extra: string
}

export const USER_INTEREST_KEYS = [
  'tech',
  'life',
  'art',
  'sports',
  'music',
  'reading',
  'travel',
] as const
export type UserInterestKey = (typeof USER_INTEREST_KEYS)[number]

export type UserLangPref = 'zh' | 'en' | 'any'
export type UserReplyLength = 'short' | 'medium' | 'long'

export interface UserFormState {
  nickname: string
  langPref: UserLangPref
  replyLength: UserReplyLength
  occupation: string
  interests: string[]
  timezone: string
  extra: string
}

export const defaultSoulForm = (): SoulFormState => ({
  name: '',
  traits: [],
  skills: [],
  tone: '',
  extra: '',
})

export const defaultUserForm = (): UserFormState => ({
  nickname: '',
  langPref: 'any',
  replyLength: 'medium',
  occupation: '',
  interests: [],
  timezone: '',
  extra: '',
})

function isSoulTone(v: unknown): v is SoulTone {
  return v === 'colloquial' || v === 'formal' || v === 'flex' || v === ''
}

function isUserLangPref(v: unknown): v is UserLangPref {
  return v === 'zh' || v === 'en' || v === 'any'
}

function isUserReplyLength(v: unknown): v is UserReplyLength {
  return v === 'short' || v === 'medium' || v === 'long'
}

function filterStringArray(v: unknown): string[] {
  if (!Array.isArray(v)) return []
  return v.filter((x): x is string => typeof x === 'string')
}

function normalizeSoul(raw: unknown): SoulFormState {
  const d = defaultSoulForm()
  if (!raw || typeof raw !== 'object') return d
  const o = raw as Record<string, unknown>
  if (typeof o.name === 'string') d.name = o.name
  d.traits = filterStringArray(o.traits)
  d.skills = filterStringArray(o.skills)
  if (isSoulTone(o.tone)) d.tone = o.tone
  if (typeof o.extra === 'string') d.extra = o.extra
  return d
}

function normalizeUser(raw: unknown): UserFormState {
  const d = defaultUserForm()
  if (!raw || typeof raw !== 'object') return d
  const o = raw as Record<string, unknown>
  if (typeof o.nickname === 'string') d.nickname = o.nickname
  if (isUserLangPref(o.langPref)) d.langPref = o.langPref
  if (isUserReplyLength(o.replyLength)) d.replyLength = o.replyLength
  if (typeof o.occupation === 'string') d.occupation = o.occupation
  d.interests = filterStringArray(o.interests)
  if (typeof o.timezone === 'string') d.timezone = o.timezone
  if (typeof o.extra === 'string') d.extra = o.extra
  return d
}

function parseSoulStrict(raw: string): { ok: true; data: SoulFormState } | { ok: false } {
  const t = raw.trim()
  if (!t.startsWith(SOUL_FORMAT_MARKER)) return { ok: false }
  const jsonPart = t.slice(SOUL_FORMAT_MARKER.length).trim()
  try {
    const parsed = JSON.parse(jsonPart) as unknown
    return { ok: true, data: normalizeSoul(parsed) }
  } catch {
    return { ok: false }
  }
}

function parseUserStrict(raw: string): { ok: true; data: UserFormState } | { ok: false } {
  const t = raw.trim()
  if (!t.startsWith(USER_FORMAT_MARKER)) return { ok: false }
  const jsonPart = t.slice(USER_FORMAT_MARKER.length).trim()
  try {
    const parsed = JSON.parse(jsonPart) as unknown
    return { ok: true, data: normalizeUser(parsed) }
  } catch {
    return { ok: false }
  }
}

/** 空内容视为空白表单；非 v1 JSON 则失败（如历史纯文本需用户用表单重写后保存）。 */
export function parseSoul(raw: string): { ok: true; data: SoulFormState } | { ok: false } {
  if (raw.trim() === '') return { ok: true, data: defaultSoulForm() }
  return parseSoulStrict(raw)
}

export function serializeSoul(data: SoulFormState): string {
  return `${SOUL_FORMAT_MARKER}\n${JSON.stringify(data)}\n`
}

export function parseUser(raw: string): { ok: true; data: UserFormState } | { ok: false } {
  if (raw.trim() === '') return { ok: true, data: defaultUserForm() }
  return parseUserStrict(raw)
}

export function serializeUser(data: UserFormState): string {
  return `${USER_FORMAT_MARKER}\n${JSON.stringify(data)}\n`
}

export function toggleMultiValue(list: string[], key: string): string[] {
  if (list.includes(key)) return list.filter((x) => x !== key)
  return [...list, key]
}
