/**
 * 请求设备 API。baseUrl 不含末尾斜杠，path 如 '/api/config'。
 * 配对码仅通过 Header X-Pairing-Code 传递，避免 query 导致预检 URL 不匹配而 CORS 失败。
 */
function buildUrl(baseUrl: string, path: string): string {
  return `${baseUrl.replace(/\/$/, '')}${path}`
}

export const API_ERROR = {
  NO_BASE_URL: 'NO_BASE_URL',
  PAIRING_REQUIRED: 'PAIRING_REQUIRED',
} as const

export interface ApiRequestOptions {
  method?: 'GET' | 'POST' | 'DELETE'
  body?: string | object
  pairingCode?: string
}

export interface ApiResult<T = unknown> {
  ok: boolean
  data?: T
  error?: string
}

export async function request<T = unknown>(
  baseUrl: string,
  path: string,
  options: ApiRequestOptions = {},
): Promise<ApiResult<T>> {
  const { method = 'GET', body, pairingCode } = options
  const url = buildUrl(baseUrl, path)
  const headers: Record<string, string> = {
    Accept: 'application/json',
  }
  if (pairingCode?.trim()) headers['X-Pairing-Code'] = pairingCode.trim()
  if (body !== undefined) {
    headers['Content-Type'] = 'application/json'
  }

  try {
    const res = await fetch(url, {
      method,
      headers,
      body: typeof body === 'object' ? JSON.stringify(body) : body,
    })
    const text = await res.text()
    let data: unknown
    try {
      data = text ? JSON.parse(text) : null
    } catch {
      data = text
    }

    if (!res.ok) {
      const err = typeof data === 'object' && data !== null && 'error' in data
        ? String((data as { error: unknown }).error)
        : res.statusText
      return { ok: false, error: err } as ApiResult<T>
    }
    return { ok: true, data: data as T }
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : 'Network error' } as ApiResult<T>
  }
}
