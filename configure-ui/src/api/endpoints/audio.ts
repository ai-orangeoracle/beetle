import { API_ERROR, request, type ApiResult } from '../client'
import type { AudioConfig } from '../../types/audioConfig'

export async function getAudioConfig(baseUrl: string, pairingCode?: string): Promise<ApiResult<AudioConfig>> {
  if (!baseUrl?.trim()) return { ok: false, error: API_ERROR.NO_BASE_URL }
  return request<AudioConfig>(baseUrl, '/api/config/audio', {
    pairingCode: pairingCode?.trim(),
  })
}

export async function saveAudioConfig(
  baseUrl: string,
  pairingCode: string,
  body: AudioConfig,
): Promise<ApiResult<{ ok: boolean; restart_required?: boolean }>> {
  if (!baseUrl?.trim()) return { ok: false, error: API_ERROR.NO_BASE_URL }
  if (!pairingCode?.trim()) return { ok: false, error: API_ERROR.PAIRING_REQUIRED }
  return request<{ ok: boolean; restart_required?: boolean }>(baseUrl, '/api/config/audio', {
    method: 'POST',
    body,
    pairingCode: pairingCode.trim(),
  })
}
