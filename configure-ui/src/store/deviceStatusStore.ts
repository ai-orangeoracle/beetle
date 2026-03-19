/**
 * 设备状态单一数据源：连接状态 + 是否激活（设备端是否已设配对码）+ 重启闭环状态。
 * 仅由 DeviceProvider 写入：baseUrl 变化时检测一次，并有定时复检（约 30s），
 * 便于设备断线后更新侧栏/横幅；各页通过 useDeviceStatus() / useDeviceConnected() 消费。
 * 重启闭环：指令发出→仍可达=正在重启→不可达=重启中→再次可达=重启完成；超时 1 分钟=设备可能异常。
 */

import { useSyncExternalStore } from 'react'

export type ConnectionStatus = 'none' | 'checking' | 'reachable' | 'unreachable'

export interface DeviceStatus {
  connectionStatus: ConnectionStatus
  /** 设备端是否已设配对码（GET /api/pairing_code 的 code_set）；null = 未请求或不可达 */
  activated: boolean | null
}

/** 重启闭环阶段：idle=无重启流程，pending=指令已发设备仍可达，restarting=设备已掉线等待上线 */
export type RestartPhase = 'idle' | 'pending' | 'restarting'

const RESTART_TIMEOUT_MS = 60_000

let status: DeviceStatus = {
  connectionStatus: 'none',
  activated: null,
}
let restartPending = false
let restartDropTime: number | null = null
let reconnectedAfterRestart = false
let restartTimeout = false
const listeners = new Set<() => void>()
const restartListeners = new Set<() => void>()

function getSnapshot(): DeviceStatus {
  return status
}

function subscribe(callback: () => void): () => void {
  listeners.add(callback)
  return () => listeners.delete(callback)
}

function emitChange(): void {
  listeners.forEach((l) => l())
  restartListeners.forEach((l) => l())
}

/** 设置设备状态。仅由 DeviceProvider 调用。 */
export function setDeviceStatus(connectionStatus: ConnectionStatus, activated: boolean | null): void {
  const next: DeviceStatus = { connectionStatus, activated }
  if (status.connectionStatus === next.connectionStatus && status.activated === next.activated) return
  status = next
  emitChange()
}

/** 重启指令已发出，由 TopBar 在 POST /api/restart 成功后调用。 */
export function setRestartPending(): void {
  restartPending = true
  restartDropTime = null
  emitChange()
}

/**
 * 轮询得到新连接状态后调用，用于推进重启闭环：不可达时记 dropTime，再次可达则置「重启完成」并清 pending；
 * 自 drop 起超过 1 分钟仍不可达则置「设备可能异常」并清 pending。
 */
export function updateRestartState(connectionStatus: ConnectionStatus): void {
  if (!restartPending) return
  if (connectionStatus === 'unreachable') {
    if (restartDropTime === null) restartDropTime = Date.now()
    else if (Date.now() - restartDropTime > RESTART_TIMEOUT_MS) {
      restartPending = false
      restartDropTime = null
      restartTimeout = true
      emitChange()
    }
    return
  }
  if (connectionStatus === 'reachable' && restartDropTime !== null) {
    restartPending = false
    restartDropTime = null
    reconnectedAfterRestart = true
    emitChange()
  }
}

/** 当前重启阶段，供横幅等展示。 */
export function getRestartPhase(): RestartPhase {
  if (!restartPending) return 'idle'
  return restartDropTime === null ? 'pending' : 'restarting'
}

/** 一次性：重启后已重新连接，消费后清除。用于弹 toast「重启完成」。 */
export function consumeReconnectedAfterRestart(): boolean {
  const v = reconnectedAfterRestart
  if (v) {
    reconnectedAfterRestart = false
    emitChange()
  }
  return v
}

/** 一次性：重启流程超时，消费后清除。用于弹 toast「设备可能异常」。 */
export function consumeRestartTimeout(): boolean {
  const v = restartTimeout
  if (v) {
    restartTimeout = false
    emitChange()
  }
  return v
}

function subscribeRestart(cb: () => void): () => void {
  restartListeners.add(cb)
  return () => restartListeners.delete(cb)
}
/** 订阅重启阶段变化（phase）；单次事件由上层用 consume* 消费。getSnapshot 必须返回稳定引用，否则会触发 React #185 无限重渲染。 */
export function useRestartPhase(): RestartPhase {
  return useSyncExternalStore(subscribeRestart, getRestartPhase, getRestartPhase)
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
