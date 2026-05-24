import os
import sys

try:
    from PIL import Image, ImageDraw
except ImportError:
    import subprocess
    subprocess.check_call([sys.executable, "-m", "pip", "install", "Pillow"])
    from PIL import Image, ImageDraw

def generate_header(logo_path, out_path):
    # 150x57 dark background
    bg_color = (25, 25, 25) # Dark gray/black
    header = Image.new("RGB", (150, 57), bg_color)
    
    # Load and resize logo to fit
    logo = Image.open(logo_path).convert("RGBA")
    logo.thumbnail((45, 45), Image.Resampling.LANCZOS)
    
    # Paste logo on right side
    offset = (150 - 45 - 6, 6) # right padded
    header.paste(logo, offset, logo)
    
    header.save(out_path, format="BMP")
    print(f"Generated {out_path}")

def generate_sidebar(logo_path, out_path):
    # 164x314 dark background
    bg_color = (20, 20, 20)
    sidebar = Image.new("RGB", (164, 314), bg_color)
    
    # Load and resize logo
    logo = Image.open(logo_path).convert("RGBA")
    logo.thumbnail((120, 120), Image.Resampling.LANCZOS)
    
    # Paste logo in top center
    offset = ((164 - logo.width) // 2, 40)
    sidebar.paste(logo, offset, logo)
    
    sidebar.save(out_path, format="BMP")
    print(f"Generated {out_path}")

if __name__ == "__main__":
    base_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    logo_path = os.path.join(base_dir, "src-tauri", "icons", "128x128.png")
    header_path = os.path.join(base_dir, "src-tauri", "icons", "header.bmp")
    sidebar_path = os.path.join(base_dir, "src-tauri", "icons", "sidebar.bmp")
    
    generate_header(logo_path, header_path)
    generate_sidebar(logo_path, sidebar_path)
