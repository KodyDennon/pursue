#!/usr/bin/env python3
import argparse
import json
import sys
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

MODEL = None
PROCESSOR = None
MODEL_PATH = None
DEVICE = "cpu"
LOCK = threading.Lock()


def _json_response(handler, status, payload):
    data = json.dumps(payload).encode("utf-8")
    handler.send_response(status)
    handler.send_header("Content-Type", "application/json")
    handler.send_header("Content-Length", str(len(data)))
    handler.end_headers()
    handler.wfile.write(data)


def _select_device(torch):
    if torch.cuda.is_available():
        return "cuda"
    if hasattr(torch.backends, "mps") and torch.backends.mps.is_available():
        return "mps"
    return "cpu"


def _load_model(model_path):
    global MODEL, PROCESSOR, MODEL_PATH, DEVICE
    with LOCK:
        if MODEL is not None and MODEL_PATH == model_path:
            return

        from transformers import AutoModelForImageTextToText, AutoProcessor
        import torch

        DEVICE = _select_device(torch)
        PROCESSOR = AutoProcessor.from_pretrained(model_path, local_files_only=True, trust_remote_code=True)
        dtype = torch.float16 if DEVICE in ("cuda", "mps") else torch.float32
        MODEL = AutoModelForImageTextToText.from_pretrained(
            model_path,
            local_files_only=True,
            torch_dtype=dtype,
            trust_remote_code=True,
        )
        MODEL.to(DEVICE)
        MODEL.eval()
        MODEL_PATH = model_path


def _extract_json(text):
    start = text.find("{")
    end = text.rfind("}")
    if start >= 0 and end >= start:
        try:
            return json.loads(text[start : end + 1])
        except Exception:
            return None
    return None


def _run_audit(payload):
    model_path = str(Path(payload["model_path"]).expanduser())
    image_paths = [Path(p).expanduser() for p in payload.get("images", [])]
    prompt = payload.get("text", "")

    if not image_paths:
        raise RuntimeError("no image paths supplied for visual audit")
    for image_path in image_paths:
        if not image_path.exists():
            raise RuntimeError(f"image not found: {image_path}")

    _load_model(model_path)

    import torch
    from PIL import Image

    images = [Image.open(path).convert("RGB") for path in image_paths[:6]]
    content = [{"type": "image", "image": image} for image in images]
    content.append({"type": "text", "text": prompt})
    messages = [{"role": "user", "content": content}]

    try:
        inputs = PROCESSOR.apply_chat_template(
            messages,
            add_generation_prompt=True,
            tokenize=True,
            return_dict=True,
            return_tensors="pt",
        )
    except Exception:
        inputs = PROCESSOR(text=prompt, images=images, return_tensors="pt")

    inputs = {k: v.to(DEVICE) if hasattr(v, "to") else v for k, v in inputs.items()}
    with torch.inference_mode():
        output_ids = MODEL.generate(**inputs, max_new_tokens=1024, do_sample=False)

    if "input_ids" in inputs:
        generated_ids = output_ids[:, inputs["input_ids"].shape[-1] :]
    else:
        generated_ids = output_ids
    raw = PROCESSOR.batch_decode(generated_ids, skip_special_tokens=True)[0].strip()
    parsed = _extract_json(raw)
    if parsed is None:
        return {
            "ok": False,
            "model_id": Path(model_path).name,
            "device": DEVICE,
            "raw_response": raw,
            "error": "model did not return valid JSON",
        }
    return {
        "ok": True,
        "model_id": Path(model_path).name,
        "device": DEVICE,
        "raw_response": raw,
        "response_json": parsed,
    }


class Handler(BaseHTTPRequestHandler):
    def log_message(self, _format, *_args):
        return

    def do_GET(self):
        if self.path != "/health":
            _json_response(self, 404, {"ok": False, "error": "not found"})
            return
        try:
            import transformers  # noqa: F401
            import torch  # noqa: F401
            from PIL import Image  # noqa: F401

            _json_response(self, 200, {"ok": True, "device": DEVICE})
        except Exception as exc:
            _json_response(self, 503, {"ok": False, "error": str(exc)})

    def do_POST(self):
        if self.path != "/audit":
            _json_response(self, 404, {"ok": False, "error": "not found"})
            return
        try:
            length = int(self.headers.get("Content-Length", "0"))
            payload = json.loads(self.rfile.read(length).decode("utf-8"))
            _json_response(self, 200, _run_audit(payload))
        except Exception as exc:
            _json_response(self, 500, {"ok": False, "error": str(exc)})


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", default=8374, type=int)
    args = parser.parse_args()
    server = ThreadingHTTPServer((args.host, args.port), Handler)
    server.serve_forever()


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        sys.exit(0)
