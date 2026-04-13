"""Export all-MiniLM-L6-v2 to ONNX with INT8 dynamic quantization.

Uses HuggingFace Optimum for clean ONNX export (avoids torch.onnx.export / onnxscript).

Usage:
    python scripts/export_model.py [--output-dir models/minilm-l6-v2]

Produces:
    models/minilm-l6-v2/model.onnx          (FP32)
    models/minilm-l6-v2/model_int8.onnx     (INT8 dynamic quantized -- optimized for Hexagon NPU)
    models/minilm-l6-v2/tokenizer.json       (HuggingFace fast tokenizer)
"""

import argparse
import shutil
from pathlib import Path

import numpy as np

MODEL_NAME = "sentence-transformers/all-MiniLM-L6-v2"


def export_onnx(output_dir: Path):
    output_dir.mkdir(parents=True, exist_ok=True)

    print(f"Loading and exporting model: {MODEL_NAME}")

    # Use optimum for clean ONNX export
    from optimum.onnxruntime import ORTModelForFeatureExtraction
    from transformers import AutoTokenizer

    # Export to ONNX via optimum (handles dynamic axes, opset, etc.)
    model = ORTModelForFeatureExtraction.from_pretrained(MODEL_NAME, export=True)
    model.save_pretrained(str(output_dir))

    # Save tokenizer alongside model
    tokenizer = AutoTokenizer.from_pretrained(MODEL_NAME)
    tokenizer.save_pretrained(str(output_dir))

    fp32_path = output_dir / "model.onnx"
    print(f"Saved FP32 model to {fp32_path} ({fp32_path.stat().st_size / 1e6:.1f} MB)")
    print(f"Saved tokenizer to {output_dir / 'tokenizer.json'}")

    # INT8 dynamic quantization -- this is what the Hexagon NPU (45 TOPS INT8) excels at
    print("Quantizing to INT8 (dynamic)...")
    from onnxruntime.quantization import QuantType, quantize_dynamic

    int8_path = output_dir / "model_int8.onnx"
    quantize_dynamic(
        str(fp32_path),
        str(int8_path),
        weight_type=QuantType.QInt8,
    )
    print(f"Saved INT8 model to {int8_path} ({int8_path.stat().st_size / 1e6:.1f} MB)")

    # Validate the export
    print("\nValidating ONNX model...")
    import onnxruntime as ort

    session = ort.InferenceSession(str(fp32_path))
    encoded = tokenizer("This is a test sentence.", return_tensors="np")
    inputs = {k: v for k, v in encoded.items() if k in ["input_ids", "attention_mask", "token_type_ids"]}
    outputs = session.run(None, inputs)
    hidden = outputs[0]  # (batch, seq_len, 384)

    # Mean pooling with attention mask
    mask = encoded["attention_mask"]
    mask_expanded = np.expand_dims(mask, -1)
    embedding = np.sum(hidden * mask_expanded, axis=1) / np.maximum(
        mask_expanded.sum(axis=1), 1e-9
    )
    # L2 normalize
    embedding = embedding / np.linalg.norm(embedding, axis=1, keepdims=True)

    print(f"Output shape: {embedding.shape}")
    print(f"Embedding dim: {embedding.shape[1]}")
    print(f"Norm check (should be ~1.0): {np.linalg.norm(embedding[0]):.6f}")
    print("Validation passed!")

    # Clean up intermediate optimum files we don't need
    for f in output_dir.iterdir():
        if f.name not in ("model.onnx", "model_int8.onnx", "tokenizer.json", "tokenizer_config.json",
                          "special_tokens_map.json", "vocab.txt"):
            if f.is_file():
                f.unlink()

    print(f"\nDone! Model files in {output_dir}/")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Export MiniLM-L6-v2 to ONNX")
    parser.add_argument(
        "--output-dir",
        type=str,
        default="models/minilm-l6-v2",
        help="Output directory for model files",
    )
    args = parser.parse_args()
    export_onnx(Path(args.output_dir))
