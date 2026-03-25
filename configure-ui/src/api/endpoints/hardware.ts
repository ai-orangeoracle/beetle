import { API_ERROR, request, type ApiResult } from '../client'
import type { HardwareSegment } from '../../types/hardwareConfig'

export async function getHardwareConfig(
  baseUrl: string,
): Promise<ApiResult<HardwareSegment>> {
  if (!baseUrl?.trim()) return { ok: false, error: API_ERROR.NO_BASE_URL }
  return request<HardwareSegment>(baseUrl, '/api/config/hardware')
}

export async function saveHardwareConfig(
  baseUrl: string,
  pairingCode: string,
  body: HardwareSegment,
): Promise<ApiResult<{ ok: boolean }>> {
  if (!baseUrl?.trim()) return { ok: false, error: API_ERROR.NO_BASE_URL }
  if (!pairingCode?.trim()) return { ok: false, error: API_ERROR.PAIRING_REQUIRED }
  return request<{ ok: boolean }>(baseUrl, '/api/config/hardware', {
    method: 'POST',
    body,
    pairingCode: pairingCode.trim(),
  })
}
