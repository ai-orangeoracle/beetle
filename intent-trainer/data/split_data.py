#!/usr/bin/env python3
# 从 samples/ 做分层抽样，写出固定测试集 data/test.txt（约 15%），仅运行一次或需重划时再跑。
# 之后 generate_synthetic_data.py 会排除 test.txt 中的样本生成 train.txt，保证训练/测试不重叠。

import random
from pathlib import Path

TEST_RATIO = 0.15
SEED = 42


def escape_line(text: str) -> str:
    return " ".join(text.split())


def main() -> None:
    root = Path(__file__).resolve().parent
    samples_dir = root / "samples"
    test_path = root / "test.txt"

    if not samples_dir.is_dir():
        print(f"Missing directory: {samples_dir}", flush=True)
        return

    rng = random.Random(SEED)
    by_label: dict[str, list[str]] = {}
    for f in sorted(samples_dir.glob("*.txt")):
        label = f.stem
        for raw in f.read_text(encoding="utf-8").splitlines():
            text = raw.strip()
            if text:
                by_label.setdefault(label, []).append(text)

    test_pairs: list[tuple[str, str]] = []
    for label, texts in by_label.items():
        n = len(texts)
        k = max(1, int(round(n * TEST_RATIO)))
        chosen = set(rng.sample(range(n), k))
        for i, t in enumerate(texts):
            if i in chosen:
                test_pairs.append((label, t))

    rng.shuffle(test_pairs)
    lines = [f"__label__{label} {escape_line(text)}\n" for label, text in test_pairs]
    test_path.write_text("".join(lines), encoding="utf-8")
    total = sum(len(v) for v in by_label.values())
    print(f"test.txt: {len(test_pairs)} / {total} ({100*len(test_pairs)/total:.0f}%), stratified by label")
    print(f"Wrote {test_path}")


if __name__ == "__main__":
    main()
