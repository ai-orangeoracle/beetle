#!/usr/bin/env python3
# 自进化循环：生成 train -> 训练 -> 在 probe 上测试 -> 错题加入对应 samples -> 重复。
# 用法: python evolve.py [--rounds N]  默认跑满 MAX_ROUNDS 或全对即停；--rounds 5 则固定跑 5 轮。

import argparse
import subprocess
import sys
from pathlib import Path

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
DATA_DIR = SCRIPT_DIR / "data"
SAMPLES_DIR = DATA_DIR / "samples"
PROBE_PATH = DATA_DIR / "probe.txt"
MODEL_PATH = SCRIPT_DIR / "model" / "intent.bin"
MAX_ROUNDS = 50


def parse_probe(path: Path) -> list[tuple[str, str]]:
    out = []
    for raw in path.read_text(encoding="utf-8").splitlines():
        raw = raw.strip()
        if not raw or not raw.startswith("__label__"):
            continue
        rest = raw[len("__label__"):].lstrip()
        idx = rest.find(" ")
        if idx <= 0:
            continue
        label, text = rest[:idx], rest[idx + 1:].strip()
        if text:
            out.append((label, text))
    return out


def run_cmd(cmd: list[str], cwd: Path) -> bool:
    r = subprocess.run(cmd, cwd=str(cwd), capture_output=True, text=True, timeout=120)
    if r.returncode != 0:
        print(r.stderr or r.stdout, file=sys.stderr)
    return r.returncode == 0


def add_failed_to_samples(failed: list[tuple[str, str]]) -> None:
    """按标签汇总后，每个文件只读一次、写一次，避免循环里多次追加卡死。"""
    by_label: dict[str, set[str]] = {}
    for label, text in failed:
        t = text.strip()
        if t:
            by_label.setdefault(label, set()).add(t)
    for label, new_texts in by_label.items():
        f = SAMPLES_DIR / f"{label}.txt"
        f.parent.mkdir(parents=True, exist_ok=True)
        existing_lines = []
        if f.exists():
            existing_lines = [
                line for line in f.read_text(encoding="utf-8").splitlines()
                if line.strip()
            ]
        existing_set = set(existing_lines)
        to_append = [t for t in new_texts if t not in existing_set]
        if not to_append:
            continue
        with f.open("w", encoding="utf-8") as fp:
            if existing_lines:
                fp.write("\n".join(existing_lines) + "\n")
            fp.write("\n".join(to_append) + "\n")


def main() -> None:
    p = argparse.ArgumentParser(description="Train -> probe test -> add failures to samples, repeat.")
    p.add_argument("--rounds", type=int, default=None, help="Fixed number of rounds (default: until 100%% or MAX_ROUNDS)")
    args = p.parse_args()
    max_rounds = args.rounds if args.rounds is not None else MAX_ROUNDS

    if not PROBE_PATH.exists():
        print(f"Missing {PROBE_PATH}", file=sys.stderr)
        sys.exit(1)
    probe = parse_probe(PROBE_PATH)
    if not probe:
        print("No valid lines in probe.txt", file=sys.stderr)
        sys.exit(1)

    try:
        fasttext.FastText.eprint = lambda x: None
    except AttributeError:
        pass

    for round_no in range(1, max_rounds + 1):
        print(f"\n=== Round {round_no}/{max_rounds} ===")
        if not run_cmd([sys.executable, "data/generate_synthetic_data.py"], SCRIPT_DIR):
            sys.exit(1)
        if not run_cmd([sys.executable, "train.py"], SCRIPT_DIR):
            sys.exit(1)
        if not MODEL_PATH.exists():
            print("Model not produced", file=sys.stderr)
            sys.exit(1)

        model = fasttext.load_model(str(MODEL_PATH))
        failed = []
        for expected, text in probe:
            pred_list, _ = model.predict(text, k=1)
            pred = pred_list[0].replace("__label__", "")
            if pred != expected:
                failed.append((expected, text))
        if failed:
            add_failed_to_samples(failed)

        correct = len(probe) - len(failed)
        acc = correct / len(probe)
        print(f"Probe accuracy: {correct}/{len(probe)} ({acc:.1%})")
        if failed:
            for exp, txt in failed[:10]:
                pred_list, _ = model.predict(txt, k=1)
                pred = pred_list[0].replace("__label__", "")
                print(f"  - [{exp}] \"{txt[:40]}\" -> {pred} (added to samples)")
            if len(failed) > 10:
                print(f"  ... and {len(failed) - 10} more")
        if not failed and args.rounds is None:
            print("All probe passed. Stopping.")
            break
    else:
        if args.rounds is not None:
            print(f"Finished {max_rounds} rounds.")
        else:
            print(f"Reached {MAX_ROUNDS} rounds.")


if __name__ == "__main__":
    main()
