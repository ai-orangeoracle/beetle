# Hardware device config & LLM-driven control design

**English** | [中文](../zh-cn/hardware-device-config.md) | [Doc index](../README.md)

For describing on-board peripherals in **JSON** and exposing them to the Agent through one tool. `config/hardware.json` lists device IDs, types, pins, and natural-language hints (`what`/`how`); the runtime builds the **`device_control`** tool so the model does not see raw pin maps. Read/write and validation: [config-api](config-api.md). The config UI “Hardware” section edits the same shape.

---

## Design position

- **Config-driven**: One JSON list describes “what’s wired to which pin, what it’s called, what it can do, and how to use it.” At runtime this yields a single `device_control` tool.
- **Semantic isolation**: The LLM operates hardware only by device ID and natural-language descriptions (what/how); pins and driver details are invisible to the model, improving safety and extensibility.
- **Non–real-time**: Aimed at on/off, set-value, and on-demand read scenarios (typical LLM call latency 2–10 s); not suitable for real-time feedback loops or continuous sampling.
- **Generic drivers**: Covers peripherals that ESP32’s native APIs can drive: GPIO read/write, PWM, ADC, buzzer. Chip-specific sensors (e.g. DHT11, BME280) belong to a later “programmable device driver” layer.

---

## Config model

Config file path: `config/hardware.json` (SPIFFS, same level as `config/llm.json` and `config/channels.json`).

Root key is `hardware_devices`, an array; each element is one device instance:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | Yes | Device name, unique; used by the LLM in tool arguments; ≤32 bytes |
| `device_type` | string | Yes | Driver type: `gpio_out` / `gpio_in` / `pwm_out` / `adc_in` / `buzzer` |
| `pins` | object | Yes | Pin mapping; currently `{"pin": GPIO_number}` |
| `what` | string | Yes | One sentence for the LLM: what the device is and what it does; ≤128 bytes |
| `how` | string | Yes | Usage for the LLM: what params to pass and meaning; ≤256 bytes |
| `options` | object | No | Device-specific options (e.g. PWM frequency, ADC attenuation); driver-defined |

**Example** (excerpt):

```json
{
  "hardware_devices": [
    {
      "id": "onboard_led",
      "device_type": "gpio_out",
      "pins": { "pin": 2 },
      "what": "Onboard LED indicator, toggleable",
      "how": "Pass value: 1=on, 0=off"
    },
    {
      "id": "door_sensor",
      "device_type": "gpio_in",
      "pins": { "pin": 4 },
      "what": "Door contact sensor, reports open/closed",
      "how": "No params; returns 0=closed 1=open"
    },
    {
      "id": "desk_lamp",
      "device_type": "pwm_out",
      "pins": { "pin": 15 },
      "what": "Dimmable LED desk lamp",
      "how": "Pass duty: 0=off, 1–100=brightness percent",
      "options": { "frequency_hz": 5000 }
    },
    {
      "id": "reminder_buzzer",
      "device_type": "buzzer",
      "pins": { "pin": 14 },
      "what": "Passive buzzer for short alerts",
      "how": "Pass duration_ms (max 3000) or beep=true for a short beep"
    }
  ]
}
```

Full example and validation rules: [Config API – GET/POST /api/config/hardware](config-api.md).

---

## Relation to the Agent

1. **Load**: On boot, the firmware parses and validates `config/hardware.json`. If the file is missing or validation fails, no hardware tool is registered (failed validation is recorded in `load_errors` so invalid config never enters runtime).
2. **Single tool**: One tool named `device_control` is registered; **no network** (`requires_network: false`).
3. **Description and schema**: The tool’s description is built from every device’s `id`, `what`, and `how` (total length capped at 2048 bytes). The schema’s `device_id` enum lists all configured `id`s; `params` is an optional JSON object (e.g. `{"value": 1}`, `{"duty": 50}`). Read-only devices need no params.
4. **Execution**: When the Agent calls the tool, the firmware looks up the device by `device_id`, dispatches by `device_type` to the right driver, and performs the pin operation. Each call is rate-limited (interval measured from **completion** of the previous operation) and under a per-device lock; audit logs are written.

---

## Device types at a glance

| Type | Direction | Typical use | Params / return |
|------|-----------|-------------|-----------------|
| `gpio_out` | Output | Relay, LED | params: `value` 0/1; read-back after write |
| `gpio_in` | Input | Door contact, reed switch, level switch | No params; returns `value` 0/1; options: `pull` |
| `pwm_out` | Output | Dimming, fan speed | params: `duty` 0–100; options: `frequency_hz`; each device has its own LEDC timer so frequency is independent |
| `adc_in` | Input | Light sensor, battery divider, soil moisture | No params; returns `raw` 0–4095; options: `atten`; ADC1 pins only (GPIO 1–10) |
| `buzzer` | Output | Passive buzzer | params: `duration_ms` or `beep: true`; max 3 s, non-blocking |

---

## Config API

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/config/hardware` | Returns current `config/hardware.json`; if missing, `{"hardware_devices":[]}` |
| POST | `/api/config/hardware` | Validates and writes segment to SPIFFS; **takes effect after reboot**. Validation rules: [Config API](config-api.md). |

Write operations require the pairing code; see [Config API contract](config-api.md).

---

## Safety and limits

- **Pins not exposed to LLM**: The tool’s schema and description contain only `id`/`what`/`how`, not `pins`.
- **Pins and counts**: Strapping pins are forbidden (ESP32-S3: 0, 3, 45, 46). Total devices ≤ 8, of which `pwm_out` ≤ 4. No pin may be shared across devices. `adc_in` only on ADC1 pins (GPIO 1–10).
- **Rate limits**: Same output device ≥ 2 s between operations, input device reads ≥ 500 ms apart; the interval is from **completion** of the previous operation to the start of the next, to avoid accidental hammering and hardware damage.
- **PWM**: Each `pwm_out` uses a dedicated LEDC timer (up to 4), so `frequency_hz` can differ per device without overlap.
- **Buzzer**: Max 3 s per beep; excess is clamped; runs non-blocking so the Agent thread is not held.
- **Concurrency**: One lock per device; if busy, returns “device is busy” instead of queuing. If execution panics, the lock is released on drop so the device does not stay busy indefinitely.

---

## See also

- [Config API contract](config-api.md): Request/response and validation details for GET/POST /api/config/hardware.
- [Hardware & resources](hardware.md): Boards, memory, troubleshooting, and the configurable hardware entry.
