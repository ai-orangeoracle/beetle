# 意图分类本地训练 (Intent Trainer)

基于 fastText 的「用户语句 → 工具名」分类器，用于固件侧减少 LLM API 调用（高置信度时直接走工具）。

## 环境

- Python 3.8+
- 训练在本地/PC 运行；产出的 `.bin` 模型后续可量化或转 C/Rust 推理上固件。

## 数据与代码分离

- **语料**：全部放在 `data/samples/`，按意图分文件：`<标签>.txt`，**一行一句**（中英文均可）。
- **不把文本写在代码里**：用户可直接编辑、追加或新增 `data/samples/<标签>.txt` 做自训练。
- 标签与固件 `ToolRegistry` 工具名一致；`fallback` 表示走 LLM。详见 `data/README.md`。

## 用法

```bash
cd intent-trainer
pip install -r requirements.txt

# 1. 从 data/samples/*.txt 生成 train.txt（用户可先编辑 samples 再运行）
python data/generate_synthetic_data.py

# 2. 训练（读 data/train.txt，写 model/intent.bin）
python train.py

# 3. 交互式试预测（可选）
python train.py --predict
```

## 规范训练（固定测试集）

需要**留出测试集、只看泛化准确率**时：

```bash
# 1. 仅首次或重划时：从 samples 分层抽样 15% 写出 data/test.txt（固定，不参与训练）
python data/split_data.py

# 2. 生成 train.txt（自动排除 test.txt 中的样本，避免泄漏）
python data/generate_synthetic_data.py

# 3. 训练
python train.py

# 4. 在测试集上评估（真实泛化）
python eval_accuracy.py --test data/test.txt
```

- `data/test.txt` 一旦生成即固定，后续只增 samples 时不必再跑 `split_data.py`；`generate_synthetic_data.py` 会继续排除 test 再生成 train。
- evolve 仍用 `data/probe.txt` 做错题回收，与 test 互不干扰。

## 自训练

1. 编辑或新增 `data/samples/<标签>.txt`，每行一句用户话术。
2. 运行 `python data/generate_synthetic_data.py` 再 `python train.py` 即可。

## 产出与体积（嵌入式）

- `model/intent.bin`：默认 **bucket=11k、dim=14、quantize**，约 **105 KB**，测试集泛化略好于 10k/12（约 89 KB）。严格 ≤100KB 时用 `--bucket 10000 --dim 12`。
- 若准确率不够再酌情调大 `--bucket`、`--dim`；`--no-quantize` 会明显增大体积。
