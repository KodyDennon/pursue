import torch
from transformers import AutoProcessor, AutoModelForImageTextToText
from PIL import Image, ImageDraw

MODEL_ID = "stepfun-ai/GOT-OCR-2.0-hf"
device = "mps" if torch.backends.mps.is_available() else "cpu"

print(f"Loading model on {device}...")
processor = AutoProcessor.from_pretrained(MODEL_ID)
model = AutoModelForImageTextToText.from_pretrained(
    MODEL_ID,
    low_cpu_mem_usage=True,
    device_map={"": device},
    dtype=torch.float16 if device != "cpu" else torch.float32,
).eval()

# Create a dummy image with some text
img = Image.new('RGB', (400, 100), color=(255, 255, 255))
d = ImageDraw.Draw(img)
d.text((10,10), "Hello World OCR Test", fill=(0,0,0))
img_path = "dummy_test.jpg"
img.save(img_path)

print("Testing generate method for OCR without text prompt...")
inputs = processor(img, return_tensors="pt").to(device)
generate_ids = model.generate(
    **inputs,
    do_sample=False,
    tokenizer=processor.tokenizer,
    stop_strings="<|im_end|>",
    max_new_tokens=4096,
)
res = processor.decode(generate_ids[0], skip_special_tokens=True)

if "assistant\n" in res:
    res = res.split("assistant\n")[-1].strip()
elif "assistant" in res:
    res = res.split("assistant")[-1].strip()
    
print(f"Output: '{res}'")
