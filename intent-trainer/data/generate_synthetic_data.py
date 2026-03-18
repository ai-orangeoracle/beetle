#!/usr/bin/env python3
# 从 data/samples/<标签>.txt 读取语料，生成 fastText 格式的 train.txt。
# 一行一句，标签由文件名决定；不包含任何写死的语料，用户可直接编辑 samples 下 txt 自训练。
# Output: __label__<name> <text>

import random
from pathlib import Path


def escape_line(text: str) -> str:
    """fastText 一行内换行改为空格，避免多行."""
    return " ".join(text.split())


def load_test_set(test_path: Path) -> set[tuple[str, str]]:
    """解析 test.txt 为 (label, text) 集合，用于从 train 中排除。"""
    out: set[tuple[str, str]] = set()
    if not test_path.exists():
        return out
    for raw in test_path.read_text(encoding="utf-8").splitlines():
        raw = raw.strip()
        if not raw or not raw.startswith("__label__"):
            continue
        rest = raw[len("__label__"):].lstrip()
        idx = rest.find(" ")
        if idx <= 0:
            continue
        label, text = rest[:idx], rest[idx + 1:].strip()
        if text:
            out.add((label, text))
    return out


def main() -> None:
    root = Path(__file__).resolve().parent
    samples_dir = root / "samples"
    train_path = root / "train.txt"
    test_path = root / "test.txt"

    if not samples_dir.is_dir():
        print(f"Missing directory: {samples_dir}", flush=True)
        return

    test_set = load_test_set(test_path)
    if test_set:
        print(f"Excluding {len(test_set)} test samples from train")

    lines: list[str] = []
    for f in sorted(samples_dir.glob("*.txt")):
        label = f.stem
        for raw in f.read_text(encoding="utf-8").splitlines():
            text = raw.strip()
            if not text:
                continue
            if (label, text) in test_set:
                continue
            lines.append(f"__label__{label} {escape_line(text)}\n")

    random.shuffle(lines)
    train_path.write_text("".join(lines), encoding="utf-8")
    print(f"Wrote {len(lines)} lines to {train_path}")


if __name__ == "__main__":
    main()
