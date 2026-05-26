import os
import torch
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel
from transformers import AutoProcessor, AutoModelForImageTextToText
from PIL import Image
import uvicorn
import logging
import sys
import fitz  # PyMuPDF
import threading
import time
import gc
import traceback
import warnings

# Disable HF telemetry and warnings early
os.environ["HF_HUB_DISABLE_TELEMETRY"] = "1"
os.environ["TRANSFORMERS_NO_ADVISORY_WARNINGS"] = "1"

# Suppress specific library warnings
warnings.filterwarnings("ignore", category=UserWarning)
warnings.filterwarnings("ignore", message=".*torch_dtype.*")
warnings.filterwarnings("ignore", message=".*clean_up_tokenization_spaces.*")

# Configure logging
LOG_FORMAT = "%(asctime)s [%(levelname)s] [%(name)s] %(message)s"
logging.basicConfig(
    level=logging.INFO,
    format=LOG_FORMAT,
    handlers=[logging.StreamHandler(sys.stdout)],
    force=True
)

logger = logging.getLogger("got-ocr")

# Optional: File logging if we have write access
try:
    log_dir = os.path.join(os.getcwd(), "logs")
    if not os.path.exists(log_dir):
        os.makedirs(log_dir, exist_ok=True)
    file_handler = logging.FileHandler(os.path.join(log_dir, "vision_engine.log"))
    file_handler.setFormatter(logging.Formatter(LOG_FORMAT))
    logging.getLogger().addHandler(file_handler)
    logger.info(f"File logging initialized at {log_dir}/vision_engine.log")
except Exception as e:
    logger.warning(f"Could not initialize file logging: {e}")

app = FastAPI(title="PURSUE Vision Engine (GOT-OCR-2.0)")

# Model configuration
MODEL_ID = "stepfun-ai/GOT-OCR-2.0-hf"
device = "cpu"

if torch.cuda.is_available():
    device = "cuda"
elif torch.backends.mps.is_available():
    device = "mps"

logger.info(f"Hardware detected: {device.upper()}")

if device == "cpu" and sys.platform == "win32":
    # Help debug why CUDA might be missing
    try:
        import subprocess
        subprocess.check_output(["nvidia-smi"], stderr=subprocess.STDOUT)
        logger.warning("NVIDIA GPU detected by system, but Torch is running on CPU. "
                       "This usually means the CPU-only version of PyTorch was installed.")
    except Exception:
        # nvidia-smi not found, probably no NVIDIA GPU or drivers
        pass

# Global model and processor
model = None
processor = None
model_lock = threading.Lock()
cancel_requested = False
load_error = None

def load_model():
    global model, processor, load_error
    with model_lock:
        if model is not None:
            return
        try:
            logger.info(f"Initializing Neural Engine with model: {MODEL_ID}")
            start_time = time.time()
            
            processor = AutoProcessor.from_pretrained(
                MODEL_ID, 
                trust_remote_code=True,
                clean_up_tokenization_spaces=False
            )
            
            # Determine optimal dtype
            target_dtype = torch.float32
            if device != "cpu":
                target_dtype = torch.float16
            elif hasattr(torch, "bfloat16"):
                target_dtype = torch.bfloat16
            
            logger.info(f"Using dtype: {target_dtype}")
            
            load_params = {
                "low_cpu_mem_usage": True,
                "trust_remote_code": True,
                "torch_dtype": target_dtype, # Use standard key
            }

            if device == "cuda":
                logger.info("Loading model on CUDA...")
                load_params["device_map"] = "cuda:0" # Explicitly target first GPU
                model = AutoModelForImageTextToText.from_pretrained(MODEL_ID, **load_params).eval()
            elif device == "mps":
                logger.info("Loading model on Apple Silicon (MPS)...")
                # MPS doesn't support device_map="auto" well in all Transformers versions
                model = AutoModelForImageTextToText.from_pretrained(MODEL_ID, **load_params).eval()
                model = model.to("mps")
            else:
                logger.info("Loading model on CPU (approx 6-12GB RAM required)...")
                model = AutoModelForImageTextToText.from_pretrained(MODEL_ID, **load_params).eval()
                
            elapsed = time.time() - start_time
            logger.info(f"Neural Engine ready. Load time: {elapsed:.2f}s")
            load_error = None
        except Exception as e:
            load_error = str(e)
            logger.error(f"CRITICAL: Failed to load neural engine: {e}")
            logger.error(traceback.format_exc())
            raise e

def monitor_parent_lifecycle():
    """
    Terminates the process if the parent process (Tauri) dies.
    This prevents orphaned sidecar processes from hogging system resources.
    """
    def check_parent():
        parent_pid = os.getppid()
        while True:
            if sys.platform == "win32":
                import ctypes
                PROCESS_QUERY_INFORMATION = 0x0400
                SYNCHRONIZE = 0x0010
                handle = ctypes.windll.kernel32.OpenProcess(PROCESS_QUERY_INFORMATION | SYNCHRONIZE, False, parent_pid)
                if not handle:
                    logger.warning("Parent process handle lost, terminating...")
                    os._exit(0)
                ctypes.windll.kernel32.CloseHandle(handle)
            else:
                if os.getppid() == 1:
                    logger.warning("Parent process reaped by init, terminating...")
                    os._exit(0)

            time.sleep(2)

    thread = threading.Thread(target=check_parent, daemon=True)
    thread.start()

@app.on_event("startup")
async def startup_event():
    monitor_parent_lifecycle()
    threading.Thread(target=load_model, daemon=True).start()

class OCRRequest(BaseModel):
    image_path: str

@app.get("/health")
async def health():
    if model is not None:
        return {"status": "ready", "device": device, "model": MODEL_ID}
    if load_error:
        return {"status": "failed", "error": load_error}
    return {"status": "loading"}

@app.get("/status")
async def status():
    stats = {
        "status": "ready" if model is not None else "loading",
        "device": device,
        "model": MODEL_ID,
        "memory": {}
    }
    if device == "cuda":
        stats["memory"]["cuda"] = torch.cuda.memory_summary()
    elif device == "mps":
        stats["memory"]["mps"] = "active"
    return stats

@app.post("/cancel")
async def cancel():
    global cancel_requested
    cancel_requested = True
    logger.info("Cancellation signal received")
    return {"status": "cancelling"}

def process_image(image: Image.Image) -> str:
    if image.mode != "RGB":
        image = image.convert("RGB")

    inputs = processor(image, return_tensors="pt").to(device)
    
    with torch.no_grad():
        generate_ids = model.generate(
            **inputs,
            do_sample=False,
            tokenizer=processor.tokenizer,
            stop_strings="<|im_end|>",
            max_new_tokens=2048,
            pad_token_id=processor.tokenizer.pad_token_id,
            eos_token_id=processor.tokenizer.eos_token_id,
        )
    
    res = processor.decode(generate_ids[0], skip_special_tokens=True)
    if "assistant\n" in res:
        res = res.split("assistant\n")[-1]
    elif "assistant" in res:
        res = res.split("assistant")[-1]
    return res.strip()

@app.post("/ocr")
async def ocr(request: OCRRequest):
    global cancel_requested
    cancel_requested = False
    
    if model is None:
        load_model()
    
    if not os.path.exists(request.image_path):
        logger.error(f"File not found: {request.image_path}")
        raise HTTPException(status_code=404, detail=f"Image not found: {request.image_path}")

    try:
        logger.info(f"Processing neural vision task: {request.image_path}")
        start_time = time.time()
        
        if request.image_path.lower().endswith(".pdf"):
            full_text = []
            doc = fitz.open(request.image_path)
            try:
                cat = doc.pdf_catalog()
                doc.xref_set_key(cat, "StructTreeRoot", "null")
            except Exception:
                pass

            total_pages = len(doc)
            for page_num in range(total_pages):
                if cancel_requested:
                    logger.info("OCR task cancelled by user during PDF processing")
                    doc.close()
                    raise HTTPException(status_code=499, detail="Processing cancelled")

                logger.info(f"Rendering PDF page {page_num + 1}/{total_pages}")
                page = doc.load_page(page_num)
                pix = page.get_pixmap(dpi=150)
                image = Image.frombytes("RGB", [pix.width, pix.height], pix.samples)
                
                text = process_image(image)
                full_text.append(text)
                full_text.append("\n--- PAGE BREAK ---\n")
                
                del pix
                del image
                gc.collect()
                if device == "cuda":
                    torch.cuda.empty_cache()
                elif device == "mps":
                    try:
                        torch.mps.empty_cache()
                    except:
                        pass
            
            doc.close()
            final_text = "".join(full_text)
            elapsed = time.time() - start_time
            logger.info(f"PDF processed successfully in {elapsed:.2f}s")
            return {"text": final_text}
        else:
            image = Image.open(request.image_path).convert("RGB")
            text = process_image(image)
            elapsed = time.time() - start_time
            logger.info(f"Image processed successfully in {elapsed:.2f}s")
            return {"text": text}
            
    except Exception as e:
        logger.error(f"OCR processing failed for {request.image_path}: {e}")
        logger.error(traceback.format_exc())
        raise HTTPException(status_code=500, detail=str(e))

if __name__ == "__main__":
    port = int(os.environ.get("PORT", 8374))
    logger.info(f"Starting GOT-OCR Sidecar on port {port}...")
    uvicorn.run(app, host="127.0.0.1", port=port, log_level="info", timeout_keep_alive=120)
