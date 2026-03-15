/**
 * 设备状态单一数据源：连接状态 + 是否激活（设备端是否已设配对码）。
 * 仅由 DeviceProvider 在 baseUrl 变化或 GET /api/pairing_code 结果时写入；
 * 各页通过 useDeviceStatus() / useDeviceConnected() 消费。
 */

import { useSyncExternalStore } from 'react'

export type ConnectionStatus = 'none' | 'checking' | 'reachable' | 'unreachable'

export interface DeviceStatus {
  connectionStatus: ConnectionStatus
  /** 设备端是否已设配对码（GET /api/pairing_code 的 code_set）；null = 未请求或不可达 */
  activated: boolean | null
}

let status: DeviceStatus = {
  connectionStatus: 'none',
  activated: null,
}
const listeners = new Set<() => void>()

function getSnapshot(): DeviceStatus {
  return status
}

function subscribe(callback: () => void): () => void {
  listeners.add(callback)
  return () => listeners.delete(callback)
}

function emitChange(): void {
  listeners.forEach((l) => l())
}

/** 设置设备状态。仅由 DeviceProvider 调用。 */
export function setDeviceStatus(connectionStatus: ConnectionStatus, activated: boolean | null): void {
  const next: DeviceStatus = { connectionStatus, activated }
  if (status.connectionStatus === next.connectionStatus && status.activated === next.activated) return
  status = next
  emitChange()
}

/** 在组件中订阅完整设备状态。 */
export function useDeviceStatus(): DeviceStatus {
  return useSyncExternalStore(subscribe, getSnapshot, getSnapshot)
}

/** 设备是否已连接（可达且已拿到 pairing_code 响应）。 */
export function useDeviceConnected(): boolean {
  return useDeviceStatus().connectionStatus === 'reachable'
}

/** 非 React 环境获取当前是否已连接。 */
export function getDeviceConnected(): boolean {
  return status.connectionStatus === 'reachable'
}
