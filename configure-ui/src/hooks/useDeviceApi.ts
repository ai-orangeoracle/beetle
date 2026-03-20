import { useMemo } from 'react'
import { useDeviceStatus, useDeviceConnected } from '../store/deviceStatusStore'
import { useDevice } from './useDevice'
import { API_ERROR } from '../api/client'
import { request } from '../api/client'
import * as configApi from '../api/endpoints/config'
import * as soulUserApi from '../api/endpoints/soulUser'
import * as skillsApi from '../api/endpoints/skills'
import * as systemApi from '../api/endpoints/system'
import type {
  LlmConfigSegment,
  ChannelsConfigSegment,
  SystemConfigSegment,
} from '../types/appConfig'
import type { SkillItem } from '../api/endpoints/skills'

export { API_ERROR }

/** 与 Rust API 一致：无地址 / 设备未激活 / 未填本站配对码 时展示横幅 */
export type DeviceHintReason = 'no_device' | 'device_not_activated' | 'no_pairing' | null

export function useDeviceApi() {
  const { baseUrl, pairingCode } = useDevice()
  const { connectionStatus, activated } = useDeviceStatus()

  const api = useMemo(
    () => ({
      config: {
        get: () => configApi.getConfig(baseUrl ?? ''),
        saveLlm: (body: LlmConfigSegment) =>
          configApi.saveLlm(baseUrl ?? '', (pairingCode ?? '').trim(), body),
        saveChannels: (body: ChannelsConfigSegment) =>
          configApi.saveChannels(baseUrl ?? '', (pairingCode ?? '').trim(), body),
        saveSystem: (body: SystemConfigSegment) =>
          configApi.saveSystem(baseUrl ?? '', (pairingCode ?? '').trim(), body),
      },
      soul: {
        get: () => soulUserApi.getSoul(baseUrl ?? ''),
        save: (content: string) => soulUserApi.saveSoul(baseUrl ?? '', (pairingCode ?? '').trim(), content),
      },
      user: {
        get: () => soulUserApi.getUser(baseUrl ?? ''),
        save: (content: string) => soulUserApi.saveUser(baseUrl ?? '', (pairingCode ?? '').trim(), content),
      },
      skills: {
        list: () => skillsApi.listSkills(baseUrl ?? '', pairingCode ?? undefined),
        getContent: (name: string) => skillsApi.getSkillContent(baseUrl ?? '', name, pairingCode ?? undefined),
        post: (body: { name: string; enabled?: boolean; content?: string }) =>
          skillsApi.postSkill(baseUrl ?? '', (pairingCode ?? '').trim(), body),
        delete: (name: string) => skillsApi.deleteSkill(baseUrl ?? '', (pairingCode ?? '').trim(), name),
        import: (url: string, name: string) =>
          skillsApi.importSkill(baseUrl ?? '', (pairingCode ?? '').trim(), url, name),
      },
      system: {
        health: () => systemApi.getHealth(baseUrl ?? ''),
        diagnose: () => systemApi.getDiagnose(baseUrl ?? ''),
        wifiScan: () => systemApi.getWifiScan(baseUrl ?? ''),
        info: () => systemApi.getSystemInfo(baseUrl ?? '', pairingCode ?? undefined),
        channelConnectivity: () =>
          systemApi.getChannelConnectivity(baseUrl ?? '', pairingCode ?? undefined),
      },
      device: {
        probe: (targetBaseUrl?: string) => request(targetBaseUrl ?? baseUrl ?? '', '/'),
      },
    }),
    [baseUrl, pairingCode],
  )

  const ready = !!baseUrl?.trim()
  const deviceConnected = useDeviceConnected()
  const hasPairing = !!pairingCode?.trim()
  /** 仅当设备可达时根据 activated / hasPairing 决定横幅；checking / unreachable 不展示横幅 */
  const deviceHintReason: DeviceHintReason =
    !ready
      ? 'no_device'
      : connectionStatus !== 'reachable'
        ? null
        : activated === false
          ? 'device_not_activated'
          : activated === true && !hasPairing
            ? 'no_pairing'
            : null
  const needDeviceHint = deviceHintReason !== null
  const connectionChecking = connectionStatus === 'checking'

  return { api, ready, deviceConnected, hasPairing, needDeviceHint, deviceHintReason, connectionChecking }
}

export type { SkillItem }
