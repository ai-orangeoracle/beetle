# 训练数据说明

- **数据与代码分离**：所有语料放在 `samples/` 下，按意图分文件，**不写在代码里**。
- **格式**：`samples/<标签>.txt`，一行一句用户话术；文件名即标签（与固件工具名一致）。
- **中英文**：同一文件内可混合中英文，一行一句。
- **自训练**：直接编辑或新增 `samples/<标签>.txt`，保存后运行：
  - `python data/generate_synthetic_data.py` → 生成 `train.txt`
  - `python train.py` → 产出 `model/intent.bin`

## 标签与文件对应

| 标签 | 含义 |
|------|------|
| get_time | 查当前时间 |
| board_info | 板子/设备/芯片信息 |
| system_stats | WiFi、存储、系统状态 |
| gpio_read | 读 GPIO 电平 |
| gpio_write | 写 GPIO / 开灯关灯 |
| web_search | 网页搜索 |
| files | 列出/读取存储文件 |
| remind_at | 定时提醒 |
| cron | 解析 cron 下次触发 |
| fetch_url | 拉取 URL 内容 |
| kv_store | 键值存储 get/set/list |
| fallback | 走 LLM（闲聊、复杂意图） |

新增意图：在 `samples/` 下新建 `<新标签>.txt`，每行一句示例即可。

## 规范训练与测试集

- **固定测试集**：运行 `python data/split_data.py` 会从当前 samples 分层抽样约 15% 写出 `data/test.txt`，仅创建或重划时执行一次。
- **训练集**：`python data/generate_synthetic_data.py` 会从 samples 生成 `data/train.txt`，并**自动排除**出现在 `data/test.txt` 里的 (标签, 句子)，保证训练/测试不重叠。
- 评估泛化：`python eval_accuracy.py --test data/test.txt`。
