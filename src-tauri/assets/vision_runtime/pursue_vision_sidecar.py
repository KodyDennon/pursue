#!/usr/bin/env python3
import argparse
import json
import os
import sys
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

MODEL = None
PROCESSOR = None
MODEL_PATH = None
DEVICE = "cpu"
DEVICE_DETAIL = "CPU"
LOCK = threading.Lock()


def _json_response(handler, status, payload):
    data = json.dumps(payload).encode("utf-8")
    handler.send_response(status)
    handler.send_header("Content-Type", "application/json")
    handler.send_header("Content-Length", str(len(data)))
    handler.end_headers()
    handler.wfile.write(data)


def _select_device(torch):
    requested = os.environ.get("PURSUE_VISION_DEVICE") or os.environ.get("PURSUE_ACCELERATION") or "auto"
    requested = requested.lower()
    if requested in ("cpu", "off", "disabled"):
        return "cpu"
    if requested in ("cuda", "nvidia"):
        return "cuda" if torch.cuda.is_available() else "cpu"
    if requested in ("mps", "metal", "apple"):
        return "mps" if hasattr(torch.backends, "mps") and torch.backends.mps.is_available() else "cpu"
    if torch.cuda.is_available():
        return "cuda"
    if hasattr(torch.backends, "mps") and torch.backends.mps.is_available():
        return "mps"
    return "cpu"


def _cuda_max_memory(torch):
    if not torch.cuda.is_available():
        return None
    props = torch.cuda.get_device_properties(0)
    usable_gib = max(1, int((props.total_memory * 0.86) // (1024**3)))
    return {0: f"{usable_gib}GiB", "cpu": "48GiB"}


def _offload_dir(model_path):
    base = os.environ.get("PURSUE_VISION_OFFLOAD_DIR")
    if base:
        path = Path(base).expanduser()
    else:
        path = Path.home() / ".pursue" / "vision-offload" / Path(model_path).name
    path.mkdir(parents=True, exist_ok=True)
    return str(path)


def _clear_accelerator_cache(torch):
    if torch.cuda.is_available():
        torch.cuda.empty_cache()
    if hasattr(torch, "mps") and hasattr(torch.mps, "empty_cache"):
        torch.mps.empty_cache()


def _release_model():
    global MODEL, PROCESSOR, MODEL_PATH, DEVICE, DEVICE_DETAIL
    try:
        import torch
    except Exception:
        torch = None
    MODEL = None
    PROCESSOR = None
    MODEL_PATH = None
    DEVICE = "cpu"
    DEVICE_DETAIL = "CPU"
    if torch is not None:
        _clear_accelerator_cache(torch)


def _load_model_on_device(model_path, device):
    global MODEL, PROCESSOR, MODEL_PATH, DEVICE, DEVICE_DETAIL
    from transformers import AutoModelForImageTextToText, AutoProcessor
    import torch

    PROCESSOR = AutoProcessor.from_pretrained(model_path, local_files_only=True, trust_remote_code=True)

    if device == "cuda":
        dtype = torch.float16
        MODEL = AutoModelForImageTextToText.from_pretrained(
            model_path,
            local_files_only=True,
            torch_dtype=dtype,
            trust_remote_code=True,
            low_cpu_mem_usage=True,
            device_map="auto",
            max_memory=_cuda_max_memory(torch),
            offload_folder=_offload_dir(model_path),
            offload_state_dict=True,
        )
        DEVICE = "cuda"
        DEVICE_DETAIL = "CUDA device_map=auto with CPU/disk offload"
    elif device == "mps":
        dtype = torch.float16
        MODEL = AutoModelForImageTextToText.from_pretrained(
            model_path,
            local_files_only=True,
            torch_dtype=dtype,
            trust_remote_code=True,
            low_cpu_mem_usage=True,
        )
        MODEL.to("mps")
        DEVICE = "mps"
        DEVICE_DETAIL = "Apple MPS full-model load with CPU operator fallback"
    else:
        MODEL = AutoModelForImageTextToText.from_pretrained(
            model_path,
            local_files_only=True,
            torch_dtype=torch.float32,
            trust_remote_code=True,
            low_cpu_mem_usage=True,
        )
        MODEL.to("cpu")
        DEVICE = "cpu"
        DEVICE_DETAIL = "CPU offload/fallback"

    MODEL.eval()
    MODEL_PATH = model_path


def _load_model(model_path):
    global MODEL, MODEL_PATH
    with LOCK:
        if MODEL is not None and MODEL_PATH == model_path:
            return

        import torch

        preferred = _select_device(torch)
        candidates = []
        if preferred == "cuda":
            candidates = ["cuda", "cpu"]
        elif preferred == "mps":
            candidates = ["mps", "cpu"]
        else:
            candidates = ["cpu"]

        last_error = None
        for candidate in candidates:
            try:
                _release_model()
                _load_model_on_device(model_path, candidate)
                return
            except Exception as exc:
                last_error = exc
                _release_model()
        raise RuntimeError(f"model load failed on {candidates}: {last_error}")


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

    input_device = "cuda" if DEVICE == "cuda" else DEVICE
    inputs = {k: v.to(input_device) if hasattr(v, "to") else v for k, v in inputs.items()}
    try:
        with torch.inference_mode():
            output_ids = MODEL.generate(**inputs, max_new_tokens=1024, do_sample=False)
    except Exception as exc:
        if DEVICE == "cpu":
            raise
        failed_device = DEVICE_DETAIL
        _release_model()
        _load_model_on_device(model_path, "cpu")
        inputs = {k: v.to("cpu") if hasattr(v, "to") else v for k, v in inputs.items()}
        with torch.inference_mode():
            output_ids = MODEL.generate(**inputs, max_new_tokens=1024, do_sample=False)
        failed_device = f"{failed_device}; generation retried on CPU after: {exc}"
    else:
        failed_device = None

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
            "device": DEVICE_DETAIL,
            "raw_response": raw,
            "error": "model did not return valid JSON",
        }
    if failed_device:
        parsed.setdefault("caveats", [])
        if isinstance(parsed["caveats"], list):
            parsed["caveats"].append(f"Vision runtime fell back from {failed_device}.")
    return {
        "ok": True,
        "model_id": Path(model_path).name,
        "device": DEVICE_DETAIL,
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

            _json_response(
                self,
                200,
                {
                    "ok": True,
                    "device": DEVICE,
                    "device_detail": DEVICE_DETAIL,
                    "cuda_available": torch.cuda.is_available(),
                    "mps_available": hasattr(torch.backends, "mps") and torch.backends.mps.is_available(),
                },
            )
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
