import type { DeviceEntry } from "../types/hardwareConfig";

/** 生成唯一设备 ID（≤32 字符），与 `taken` 中已有值不重复 */
export function generateDeviceId(taken: Set<string>): string {
  for (let attempt = 0; attempt < 64; attempt++) {
    const bytes = new Uint8Array(4);
    crypto.getRandomValues(bytes);
    const suffix = Array.from(bytes, (b) =>
      b.toString(16).padStart(2, "0"),
    ).join("");
    const id = `d_${suffix}`;
    if (!taken.has(id) && id.length <= 32) {
      return id;
    }
  }
  const fallback = `d_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 10)}`;
  return fallback.slice(0, 32);
}

/** 为缺少 id 的条目补全随机 id（用于 GET 回显与单处规范化） */
export function ensureHardwareDeviceIds(devices: DeviceEntry[]): DeviceEntry[] {
  const taken = new Set<string>();
  devices.forEach((d) => {
    if (d.id?.trim()) taken.add(d.id);
  });
  return devices.map((d) => {
    if (d.id?.trim()) return d;
    const id = generateDeviceId(taken);
    taken.add(id);
    return { ...d, id };
  });
}
