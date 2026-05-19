import os
import torch
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel
from transformers import AutoProcessor, AutoModelForImageTextToText
from PIL import Image
import uvicorn
import logging
import sys

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
    handlers=[logging.StreamHandler(sys.stdout)]
)
logger = logging.getLogger("got-ocr-sidecar")

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

def load_model():
    global model, processor
    try:
        logger.info(f"Loading {MODEL_ID} in FP16 precision...")
        processor = AutoProcessor.from_pretrained(MODEL_ID)
        model = AutoModelForImageTextToText.from_pretrained(
            MODEL_ID,
            low_cpu_mem_usage=True,
            device_map={"": device},
            torch_dtype=torch.float16 if device != "cpu" else torch.float32
        ).eval()
        logger.info("Neural Engine ready.")
    except Exception as e:
        logger.error(f"Failed to load neural engine: {e}")
        raise e

@app.on_event("startup")
async def startup_event():
    # We load the model on startup to ensure readiness
    load_model()

class OCRRequest(BaseModel):
    image_path: str

@app.get("/health")
async def health():
    if model is not None:
        return {"status": "ready", "device": device, "model": MODEL_ID}
    return {"status": "loading"}

@app.post("/ocr")
async def ocr(request: OCRRequest):
    if model is None:
        raise HTTPException(status_code=503, detail="Model not loaded")
    
    if not os.path.exists(request.image_path):
        raise HTTPException(status_code=404, detail=f"Image not found: {request.image_path}")

    try:
        logger.info(f"Processing neural vision task: {request.image_path}")
        
        image = Image.open(request.image_path).convert("RGB")
        inputs = processor(image, return_tensors="pt").to(device)

        # Generate OCR text
        generate_ids = model.generate(
            **inputs,
            do_sample=False,
            tokenizer=processor.tokenizer,
            stop_strings="<|im_end|>",
            max_new_tokens=4096,
        )

        # Decode output
        res = processor.decode(generate_ids[0], skip_special_tokens=True)
        
        return {"text": res}
    except Exception as e:
        logger.error(f"OCR processing failed: {e}")
        raise HTTPException(status_code=500, detail=str(e))

if __name__ == "__main__":
    port = int(os.environ.get("PORT", 8374))
    logger.info(f"Starting GOT-OCR Sidecar on port {port}...")
    uvicorn.run(app, host="127.0.0.1", port=port)
