import torch
from transformers import AutoProcessor, AutoModelForImageTextToText
from PIL import Image, ImageDraw
import time

MODEL_ID = "stepfun-ai/GOT-OCR-2.0-hf"
device = "mps" if torch.backends.mps.is_available() else "cuda" if torch.cuda.is_available() else "cpu"

print(f"Loading model on {device}...")
start_load = time.time()
processor = AutoProcessor.from_pretrained(MODEL_ID, trust_remote_code=True)
model = AutoModelForImageTextToText.from_pretrained(
    MODEL_ID,
    low_cpu_mem_usage=True,
    device_map={"": device} if device != "mps" else None,
    trust_remote_code=True,
    torch_dtype=torch.float16 if device != "cpu" else torch.float32,
).eval()

if device == "mps":
    model = model.to("mps")

print(f"Model loaded in {time.time() - start_load:.2f}s")

# Create a dummy image with some text
img = Image.new('RGB', (400, 100), color=(255, 255, 255))
d = ImageDraw.Draw(img)
d.text((10,10), "Hello World OCR Test", fill=(0,0,0))
img_path = "dummy_test.jpg"
img.save(img_path)

print("Testing generate method for OCR...")
start_inf = time.time()
inputs = processor(img, return_tensors="pt").to(device)
with torch.no_grad():
    generate_ids = model.generate(
        **inputs,
        do_sample=False,
        tokenizer=processor.tokenizer,
        stop_strings="<|im_end|>",
        max_new_tokens=1024,
    )
res = processor.decode(generate_ids[0], skip_special_tokens=True)

if "assistant\n" in res:
    res = res.split("assistant\n")[-1].strip()
elif "assistant" in res:
    res = res.split("assistant")[-1].strip()
    
print(f"Output: '{res}'")
print(f"Inference time: {time.time() - start_inf:.2f}s")
