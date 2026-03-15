import { request, API_ERROR } from '../client'
import type {
  AppConfig,
  LlmConfigSegment,
  ChannelsConfigSegment,
  SystemConfigSegment,
} from '../../types/appConfig'
import type { ApiResult } from '../client'

export async function getConfig(baseUrl: string): Promise<ApiResult<AppConfig>> {
  if (!baseUrl?.trim()) return { ok: false, error: API_ERROR.NO_BASE_URL }
  return request<AppConfig>(baseUrl, '/api/config')
}

export async function saveLlm(
  baseUrl: string,
  pairingCode: string,
  body: LlmConfigSegment,
): Promise<ApiResult<void>> {
  if (!baseUrl?.trim()) return { ok: false, error: API_ERROR.NO_BASE_URL }
  if (!pairingCode?.trim()) return { ok: false, error: API_ERROR.PAIRING_REQUIRED }
  return request<void>(baseUrl, '/api/config/llm', {
    method: 'POST',
    body,
    pairingCode: pairingCode.trim(),
  })
}

export async function saveChannels(
  baseUrl: string,
  pairingCode: string,
  body: ChannelsConfigSegment,
): Promise<ApiResult<void>> {
  if (!baseUrl?.trim()) return { ok: false, error: API_ERROR.NO_BASE_URL }
  if (!pairingCode?.trim()) return { ok: false, error: API_ERROR.PAIRING_REQUIRED }
  return request<void>(baseUrl, '/api/config/channels', {
    method: 'POST',
    body,
    pairingCode: pairingCode.trim(),
  })
}

export async function saveSystem(
  baseUrl: string,
  pairingCode: string,
  body: SystemConfigSegment,
): Promise<ApiResult<void>> {
  if (!baseUrl?.trim()) return { ok: false, error: API_ERROR.NO_BASE_URL }
  if (!pairingCode?.trim()) return { ok: false, error: API_ERROR.PAIRING_REQUIRED }
  return request<void>(baseUrl, '/api/config/system', {
    method: 'POST',
    body,
    pairingCode: pairingCode.trim(),
  })
}
