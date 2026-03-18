#!/usr/bin/env python3
# 用已训练模型在指定数据文件上计算准确率。
# 用法: python eval_accuracy.py [--test data/train.txt]
# 未传 --test 时默认用 data/train.txt（仅为训练集准确率；建议保留部分数据做 test.txt 得真实泛化）

import argparse
import sys
from pathlib import Path

# numpy 2.x 与 fastText predict() 的 copy=False 不兼容，先 patch 再 import fasttext
try:
    import numpy as np
    _orig_array = np.array
    np.array = lambda obj, copy=False, **kw: _orig_array(obj, **kw) if copy else np.asarray(obj, **kw)
except Exception:
    pass
try:
    import fasttext
except ImportError:
    print("Run: pip install -r requirements.txt", file=sys.stderr)
    sys.exit(1)

SCRIPT_DIR = Path(__file__).resolve().parent
DEFAULT_MODEL = SCRIPT_DIR / "model" / "intent.bin"
DEFAULT_DATA = SCRIPT_DIR / "data" / "train.txt"


def parse_fasttext_line(line: str) -> tuple[str, str] | None:
    """返回 (label, text)，label 不含 __label__ 前缀。"""
    line = line.strip()
    if not line:
        return None
    if not line.startswith("__label__"):
        return None
    rest = line[len("__label__") :].lstrip()
    idx = rest.find(" ")
    if idx <= 0:
        return None
    label = rest[:idx]
    text = rest[idx + 1 :].strip()
    return (label, text) if text else None


def main() -> None:
    p = argparse.ArgumentParser(description="Evaluate intent model accuracy")
    p.add_argument("--model", type=Path, default=DEFAULT_MODEL, help="Path to intent.bin")
    p.add_argument("--test", type=Path, default=DEFAULT_DATA, help="Path to labeled data (fastText format)")
    args = p.parse_args()

    if not args.model.exists():
        print(f"Missing model: {args.model}", file=sys.stderr)
        sys.exit(1)
    if not args.test.exists():
        print(f"Missing data: {args.test}", file=sys.stderr)
        sys.exit(1)

    try:
        fasttext.FastText.eprint = lambda x: None
    except AttributeError:
        pass
    model = fasttext.load_model(str(args.model))

    samples = []
    for raw in args.test.read_text(encoding="utf-8").splitlines():
        parsed = parse_fasttext_line(raw)
        if parsed:
            samples.append(parsed)

    if not samples:
        print("No valid labeled lines in data file.", file=sys.stderr)
        sys.exit(1)

    correct = 0
    by_label: dict[str, list[bool]] = {}
    for true_label, text in samples:
        pred_labels, _ = model.predict(text, k=1)
        pred = pred_labels[0].replace("__label__", "")
        ok = pred == true_label
        if ok:
            correct += 1
        by_label.setdefault(true_label, []).append(ok)

    n = len(samples)
    acc = correct / n
    print(f"Total: {n}  Correct: {correct}  Accuracy: {acc:.2%}")
    if args.test == DEFAULT_DATA and "train" in str(DEFAULT_DATA):
        print("(Data is train set; for generalization use a held-out test set.)")
    print()
    print("Per-label (true label -> correct count / total):")
    for label in sorted(by_label.keys()):
        lst = by_label[label]
        c = sum(1 for x in lst if x)
        print(f"  {label}: {c}/{len(lst)} ({c/len(lst):.0%})")


if __name__ == "__main__":
    main()
