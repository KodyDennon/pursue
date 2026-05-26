#!/bin/bash
set -e

TARGET_DIR="src-tauri/assets/models"
mkdir -p "$TARGET_DIR"

DET_URL="https://github.com/GreatV/oar-ocr/releases/download/v0.3.0/pp-ocrv5_mobile_det.onnx"
REC_URL="https://github.com/GreatV/oar-ocr/releases/download/v0.3.0/pp-ocrv5_mobile_rec.onnx"
DICT_URL="https://raw.githubusercontent.com/PaddlePaddle/PaddleOCR/main/ppocr/utils/dict/ppocrv5_dict.txt"

echo "Downloading PaddleOCR v5 Detection Model..."
if [ ! -f "$TARGET_DIR/pp-ocrv5_mobile_det.onnx" ]; then
    curl -L -o "$TARGET_DIR/pp-ocrv5_mobile_det.onnx" "$DET_URL"
else
    echo "Detection model already exists."
fi

echo "Downloading PaddleOCR v5 Recognition Model..."
if [ ! -f "$TARGET_DIR/pp-ocrv5_mobile_rec.onnx" ]; then
    curl -L -o "$TARGET_DIR/pp-ocrv5_mobile_rec.onnx" "$REC_URL"
else
    echo "Recognition model already exists."
fi

echo "Downloading PaddleOCR v5 Chinese/English Dictionary..."
if [ ! -f "$TARGET_DIR/ppocrv5_dict.txt" ]; then
    curl -L -o "$TARGET_DIR/ppocrv5_dict.txt" "$DICT_URL"
else
    echo "Dictionary already exists."
fi

echo "OCR models setup complete."
