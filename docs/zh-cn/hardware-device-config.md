# 硬件设备配置与 LLM 驱动设计

[English](../en-us/hardware-device-config.md) | **中文**

本文档面向**需要配置板载硬件并由 Agent 按语义调用的用户与开发者**，描述 Beetle 的**里程碑式设计**：通过一份 JSON 配置（`config/hardware.json`）即可让 LLM 按语义控制板载硬件（GPIO、PWM、ADC、蜂鸣器等），**零代码、配置即用**。LLM 只看到「设备叫什么、能做什么、怎么用」，不接触引脚号与底层协议。配置方式与校验规则见 [配置 API - GET/POST /api/config/hardware](config-api.md)；配置页中可在「硬件」相关项编辑。

---

## 设计定位

- **配置驱动**：一个 JSON 列表描述「引脚接了啥、叫什么、能做什么、怎么用」；运行时根据配置生成统一的 `device_control` 工具。
- **语义隔离**：LLM 仅通过设备 ID 与自然语言描述（what/how）操作硬件，引脚与驱动细节对模型不可见，便于安全与扩展。
- **非实时**：面向开关、设值、按需读取等场景（典型 LLM 调用延迟 2–10 秒），不适合实时反馈回路或连续采集。
- **通用驱动**：覆盖 GPIO 读写、PWM、ADC、蜂鸣器等 ESP32 原生 API 即可驱动的外设；特定芯片传感器（如 DHT11、BME280）属后续「可编程设备驱动」范畴。

---

## 配置模型

配置文件路径：`config/hardware.json`（SPIFFS，与 `config/llm.json`、`config/channels.json` 同级）。

根节点为 `hardware_devices` 数组，每项为一个设备实例：

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `id` | 字符串 | 是 | 设备名称，唯一，供 LLM 在工具参数中选用；≤32 字节 |
| `device_type` | 字符串 | 是 | 驱动类型：`gpio_out` / `gpio_in` / `pwm_out` / `adc_in` / `buzzer` |
| `pins` | 对象 | 是 | 引脚映射，当前统一为 `{"pin": GPIO编号}` |
| `what` | 字符串 | 是 | 给 LLM 的一句话：设备是什么、能实现什么；≤128 字节 |
| `how` | 字符串 | 是 | 给 LLM 的用法说明：传什么参数、含义；≤256 字节 |
| `options` | 对象 | 否 | 设备相关配置（如 PWM 频率、ADC 衰减），驱动自行解读 |

**示例**（节选）：

```json
{
  "hardware_devices": [
    {
      "id": "板载LED",
      "device_type": "gpio_out",
      "pins": { "pin": 2 },
      "what": "板载指示灯，可开关",
      "how": "传 value：1=亮，0=灭"
    },
    {
      "id": "门磁",
      "device_type": "gpio_in",
      "pins": { "pin": 4 },
      "what": "门磁传感器，检测门是否关闭",
      "how": "无需参数，返回 0=关门 1=开门"
    },
    {
      "id": "台灯",
      "device_type": "pwm_out",
      "pins": { "pin": 15 },
      "what": "可调光 LED 台灯",
      "how": "传 duty：0=关，1–100=亮度百分比",
      "options": { "frequency_hz": 5000 }
    },
    {
      "id": "提醒蜂鸣器",
      "device_type": "buzzer",
      "pins": { "pin": 14 },
      "what": "无源蜂鸣器，可短鸣提醒",
      "how": "传 duration_ms 响多少毫秒（上限 3000）；或 beep=true 短鸣一次"
    }
  ]
}
```

完整示例与校验规则见 [配置 API - GET/POST /api/config/hardware](config-api.md)。

---

## 与 Agent 的关系

1. **加载**：启动时从 `config/hardware.json` 解析并校验；若文件不存在或校验失败，则不注册硬件工具（校验失败时记入 `load_errors`，避免非法配置进入运行时）。
2. **单一工具**：注册一个名为 `device_control` 的工具，**不依赖网络**（`requires_network: false`）。
3. **描述与 schema**：工具的 description 由所有设备的 `id`、`what`、`how` 拼接而成（总长截断至 2048 字节）；schema 中 `device_id` 的枚举为当前配置中全部 `id`，`params` 为可选 JSON 对象（如 `{"value": 1}`、`{"duty": 50}`），读取类设备无需 params。
4. **执行**：Agent 调用时根据 `device_id` 查表，按 `device_type` 分发到对应驱动，完成引脚操作；每次调用有速率限制（以「上次操作完成」为起点）与每设备操作锁，并记录审计日志。

---

## 设备类型一览

| 类型 | 方向 | 典型用途 | 参数 / 返回 |
|------|------|----------|-------------|
| `gpio_out` | 输出 | 继电器、LED | params: `value` 0/1；写后读回确认 |
| `gpio_in` | 输入 | 门磁、干簧管、水位开关 | 无需 params；返回 `value` 0/1；options: `pull` |
| `pwm_out` | 输出 | 调光、调速 | params: `duty` 0–100；options: `frequency_hz`；每设备独立 LEDC 定时器，频率互不影响 |
| `adc_in` | 输入 | 光敏、电池分压、土壤湿度 | 无需 params；返回 `raw` 0–4095；options: `atten`；仅 ADC1 引脚（GPIO 1–10） |
| `buzzer` | 输出 | 无源蜂鸣器 | params: `duration_ms` 或 `beep: true`；最长 3 秒，非阻塞 |

---

## 配置 API

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/config/hardware` | 返回当前 `config/hardware.json` 内容；文件不存在时返回 `{"hardware_devices":[]}` |
| POST | `/api/config/hardware` | 校验并写入 segment 到 SPIFFS；**重启后生效**。校验规则见 [配置 API](config-api.md)。 |

写操作需配对码；详见 [配置 API 契约](config-api.md)。

---

## 安全与限制

- **引脚不暴露给 LLM**：tool 的 schema 与 description 仅含 `id`/`what`/`how`，不含 `pins`。
- **引脚与数量**：禁止 strapping 引脚（ESP32-S3：0、3、45、46）；设备总数 ≤ 8，其中 `pwm_out` ≤ 4；引脚不得跨设备冲突；`adc_in` 仅允许 ADC1 引脚（GPIO 1–10）。
- **速率限制**：同一输出设备两次操作间隔 ≥ 2 秒、输入设备读取间隔 ≥ 500ms；间隔以「上次操作**完成**」到「本次操作开始」计算，防止误触与硬件损坏。
- **PWM**：每个 `pwm_out` 占用独立 LEDC 定时器（最多 4 个），可配置不同 `frequency_hz`，互不覆盖。
- **buzzer**：单次最长 3 秒，超限截断；非阻塞执行，不占用 Agent 主线程。
- **并发**：每设备操作锁，正在使用时返回「设备正忙」，不排队等待；若执行中发生 panic，锁会在析构时释放，避免设备永久占满。

---

## 参考

- [配置 API 契约](config-api.md)：GET/POST /api/config/hardware 的请求响应与校验细节。
- [硬件与资源](hardware.md)：板型、内存、排错及可配置硬件设备入口。
