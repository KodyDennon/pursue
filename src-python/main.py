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
import errno
import warnings
import gc
import traceback

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

# Global model and processor
model = None
processor = None
model_lock = threading.Lock()

def load_model():
    global model, processor
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
            # bfloat16 is preferred on modern GPUs, float16 as fallback
            target_dtype = torch.float32
            if device != "cpu":
                target_dtype = torch.float16 # Standard for GOT-OCR-2.0 on MPS/CUDA
            
            load_params = {
                "low_cpu_mem_usage": True,
                "trust_remote_code": True,
                "torch_dtype": target_dtype,
            }

            if device == "cuda":
                load_params["device_map"] = "auto"
                model = AutoModelForImageTextToText.from_pretrained(MODEL_ID, **load_params).eval()
            elif device == "mps":
                model = AutoModelForImageTextToText.from_pretrained(MODEL_ID, **load_params).eval()
                model = model.to("mps")
            else:
                model = AutoModelForImageTextToText.from_pretrained(MODEL_ID, **load_params).eval()
                
            elapsed = time.time() - start_time
            logger.info(f"Neural Engine ready. Load time: {elapsed:.2f}s")
        except Exception as e:
            logger.error(f"CRITICAL: Failed to load neural engine: {e}")
            logger.error(traceback.format_exc())
            raise e

def monitor_parent_lifecycle():
    parent_pid = os.getppid()
    if parent_pid <= 1:
        return

    def poll_parent():
        logger.info(f"Parent process monitor active for PID: {parent_pid}")
        while True:
            if os.getppid() == 1:
                logger.info("Parent process terminated (re-parented to init). Exiting...")
                os._exit(0)
            
            try:
                os.kill(parent_pid, 0)
            except OSError as e:
                if e.errno == errno.ESRCH:
                    logger.info("Parent process has ceased to exist. Exiting...")
                    os._exit(0)
            
            time.sleep(2)

    thread = threading.Thread(target=poll_parent, daemon=True)
    thread.start()

@app.on_event("startup")
async def startup_event():
    monitor_parent_lifecycle()
    # Loading on startup ensures the first request doesn't timeout
    try:
        load_model()
    except:
        pass # Errors logged in load_model

class OCRRequest(BaseModel):
    image_path: str

@app.get("/health")
async def health():
    if model is not None:
        return {"status": "ready", "device": device, "model": MODEL_ID}
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
        # MPS memory summary is limited in torch, but we can check if it's initialized
        stats["memory"]["mps"] = "active"
    
    return stats

def process_image(image: Image.Image) -> str:
    # GOT-OCR-2.0 performs best with 1024x1024 input
    # The processor usually handles this, but we ensure it's RGB
    if image.mode != "RGB":
        image = image.convert("RGB")

    inputs = processor(image, return_tensors="pt").to(device)
    
    # Optimization: GOT-OCR-2.0 has specific generation config
    with torch.no_grad():
        generate_ids = model.generate(
            **inputs,
            do_sample=False,
            tokenizer=processor.tokenizer,
            stop_strings="<|im_end|>",
            max_new_tokens=2048, # Reduced from 4096 to prevent runaway generation
            pad_token_id=processor.tokenizer.pad_token_id,
            eos_token_id=processor.tokenizer.eos_token_id,
        )
    
    res = processor.decode(generate_ids[0], skip_special_tokens=True)
    
    # Clean up output
    if "assistant\n" in res:
        res = res.split("assistant\n")[-1]
    elif "assistant" in res:
        res = res.split("assistant")[-1]
    
    return res.strip()

@app.post("/ocr")
async def ocr(request: OCRRequest):
    if model is None:
        load_model()
    
    if not os.path.exists(request.image_path):
        logger.error(f"File not found: {request.image_path}")
        raise HTTPException(status_code=404, detail=f"Image not found: {request.image_path}")

    try:
        logger.info(f"Processing neural vision task: {request.image_path}")
        start_time = time.time()
        
        # Check if PDF
        if request.image_path.lower().endswith(".pdf"):
            full_text = []
            doc = fitz.open(request.image_path)
            
            # WORKAROUND: Remove broken StructTreeRoot to prevent get_pixmap hang on corrupt PDFs
            try:
                cat = doc.pdf_catalog()
                doc.xref_set_key(cat, "StructTreeRoot", "null")
            except Exception:
                pass

            total_pages = len(doc)
            for page_num in range(total_pages):
                logger.info(f"Rendering PDF page {page_num + 1}/{total_pages}")
                page = doc.load_page(page_num)
                
                # 150 DPI is optimal (resizes to 1024x1024 anyway)
                pix = page.get_pixmap(dpi=150)
                image = Image.frombytes("RGB", [pix.width, pix.height], pix.samples)
                
                text = process_image(image)
                full_text.append(text)
                full_text.append("\n--- PAGE BREAK ---\n")
                
                # Proactive cleanup to prevent thermal throttling
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
    # Increase timeout for large model loading
    uvicorn.run(app, host="127.0.0.1", port=port, log_level="info", timeout_keep_alive=120)
