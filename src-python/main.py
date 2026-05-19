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
        logger.info(f"Loading {MODEL_ID}...")
        processor = AutoProcessor.from_pretrained(MODEL_ID)
        if device == "mps":
            model = AutoModelForImageTextToText.from_pretrained(
                MODEL_ID,
                low_cpu_mem_usage=True,
                torch_dtype=torch.float16,
            ).eval()
            model = model.to("mps")
        else:
            model = AutoModelForImageTextToText.from_pretrained(
                MODEL_ID,
                low_cpu_mem_usage=True,
                device_map={"": device},
                torch_dtype=torch.float16 if device != "cpu" else torch.float32,
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

def process_image(image: Image.Image) -> str:
    inputs = processor(image, return_tensors="pt").to(device)
    generate_ids = model.generate(
        **inputs,
        do_sample=False,
        tokenizer=processor.tokenizer,
        stop_strings="<|im_end|>",
        max_new_tokens=4096,
    )
    res = processor.decode(generate_ids[0], skip_special_tokens=True)
    
    # Parse out the system prompt and return just the assistant's reply
    if "assistant\n" in res:
        return res.split("assistant\n")[-1].strip()
    elif "assistant" in res:
        return res.split("assistant")[-1].strip()
    return res.strip()

@app.post("/ocr")
async def ocr(request: OCRRequest):
    if model is None:
        raise HTTPException(status_code=503, detail="Model not loaded")
    
    if not os.path.exists(request.image_path):
        raise HTTPException(status_code=404, detail=f"Image not found: {request.image_path}")

    try:
        logger.info(f"Processing neural vision task: {request.image_path}")
        
        # Check if PDF
        if request.image_path.lower().endswith(".pdf"):
            import gc
            full_text = []
            doc = fitz.open(request.image_path)
            
            # WORKAROUND: Remove broken StructTreeRoot to prevent get_pixmap hang on corrupt PDFs
            try:
                cat = doc.pdf_catalog()
                doc.xref_set_key(cat, "StructTreeRoot", "null")
            except Exception as e:
                logger.warning(f"Could not remove StructTreeRoot: {e}")

            for page_num in range(len(doc)):
                logger.info(f"Rendering PDF page {page_num + 1}/{len(doc)}")
                page = doc.load_page(page_num)
                # 150 DPI is optimal for GOT-OCR-2.0 (it resizes to 1024x1024 anyway)
                pix = page.get_pixmap(dpi=150)
                image = Image.frombytes("RGB", [pix.width, pix.height], pix.samples)
                text = process_image(image)
                full_text.append(text)
                full_text.append("\n--- PAGE BREAK ---\n")
                
                # Proactively clean up memory after each page
                del pix
                del image
                gc.collect()
                if torch.cuda.is_available():
                    torch.cuda.empty_cache()
                elif torch.backends.mps.is_available():
                    try:
                        torch.mps.empty_cache()
                    except:
                        pass
            doc.close()
            return {"text": "".join(full_text)}
        else:
            image = Image.open(request.image_path).convert("RGB")
            text = process_image(image)
            return {"text": text}
    except Exception as e:
        logger.error(f"OCR processing failed: {e}")
        raise HTTPException(status_code=500, detail=str(e))

if __name__ == "__main__":
    port = int(os.environ.get("PORT", 8374))
    logger.info(f"Starting GOT-OCR Sidecar on port {port}...")
    uvicorn.run(app, host="127.0.0.1", port=port)
