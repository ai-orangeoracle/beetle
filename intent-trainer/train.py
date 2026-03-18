#!/usr/bin/env python3
# 训练意图分类器：读 data/train.txt（fastText 格式），输出 model/intent.bin。

import argparse
import sys
from pathlib import Path

try:
    import fasttext
except ImportError:
    print("Run: pip install -r requirements.txt", file=sys.stderr)
    sys.exit(1)

SCRIPT_DIR = Path(__file__).resolve().parent
DEFAULT_TRAIN = SCRIPT_DIR / "data" / "train.txt"
DEFAULT_MODEL_DIR = SCRIPT_DIR / "model"


def train(
    train_path: Path = DEFAULT_TRAIN,
    model_dir: Path = DEFAULT_MODEL_DIR,
    epoch: int = 25,
    lr: float = 0.5,
    word_ngrams: int = 2,
    bucket: int = 11_000,
    dim: int = 14,
    quantize: bool = True,
) -> Path:
    if not train_path.exists():
        print(f"Missing {train_path}. Run: python data/generate_synthetic_data.py", file=sys.stderr)
        sys.exit(1)
    model_dir.mkdir(parents=True, exist_ok=True)
    out_bin = model_dir / "intent.bin"

    # 略大一点：bucket=11k、dim=14，quantize 后约 100KB 左右
    model = fasttext.train_supervised(
        str(train_path),
        epoch=epoch,
        lr=lr,
        wordNgrams=word_ngrams,
        bucket=bucket,
        dim=dim,
        verbose=1,
    )
    if quantize:
        model.quantize(input=str(train_path), retrain=False)
    model.save_model(str(out_bin))
    print(f"Saved {out_bin}")
    return out_bin


def run_predict(model_path: Path) -> None:
    if not model_path.exists():
        print(f"Missing {model_path}. Run: python train.py", file=sys.stderr)
        sys.exit(1)
    # 抑制 fastText 0.9.x 的 load_model 兼容性警告（C 侧 eprint，warnings 拦不住）
    try:
        fasttext.FastText.eprint = lambda x: None
    except AttributeError:
        pass
    model = fasttext.load_model(str(model_path))
    print("Enter a line to predict intent (Ctrl+D/Ctrl+C exit):")
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        labels, probs = model.predict(line, k=1)
        print(f"  -> {labels[0]} ({probs[0]:.3f})")


def main() -> None:
    p = argparse.ArgumentParser(description="Train intent classifier (fastText)")
    p.add_argument("--train", type=Path, default=DEFAULT_TRAIN, help="Path to train.txt")
    p.add_argument("--model-dir", type=Path, default=DEFAULT_MODEL_DIR, help="Output dir for intent.bin")
    p.add_argument("--epoch", type=int, default=25, help="Training epochs")
    p.add_argument("--lr", type=float, default=0.5, help="Learning rate")
    p.add_argument("--word-ngrams", type=int, default=2, help="Word n-grams")
    p.add_argument("--bucket", type=int, default=11_000, help="Bucket count (11k; 10k for minimal size)")
    p.add_argument("--dim", type=int, default=14, help="Word vector dimension (14; 12 for minimal)")
    p.add_argument("--no-quantize", action="store_true", help="Skip quantize (larger .bin)")
    p.add_argument("--predict", action="store_true", help="Interactive predict using existing model")
    args = p.parse_args()

    if args.predict:
        run_predict(args.model_dir / "intent.bin")
    else:
        train(
            train_path=args.train,
            model_dir=args.model_dir,
            epoch=args.epoch,
            lr=args.lr,
            word_ngrams=args.word_ngrams,
            bucket=args.bucket,
            dim=args.dim,
            quantize=not args.no_quantize,
        )


if __name__ == "__main__":
    main()
