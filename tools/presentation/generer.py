"""Fabrique les images de présentation du README (docs/cover.png et docs/demo.gif) à partir de nos
propres éléments (logo, couleurs, textes). Chromium (Playwright) dessine, Pillow assemble le GIF.
python3 tools/presentation/generer.py"""
import base64, io, os
from playwright.sync_api import sync_playwright
from PIL import Image

ICI = os.path.dirname(os.path.abspath(__file__))
DOCS = os.path.join(ICI, '..', '..', 'docs')
LOGO = 'data:image/png;base64,' + base64.b64encode(open(os.path.join(DOCS, 'logo.png'), 'rb').read()).decode()
FONT = "'Segoe UI','DejaVu Sans',sans-serif"

COVER = f'''<html><body style="margin:0;width:1280px;height:640px;background:linear-gradient(135deg,#0f2a4f,#174a8c);font-family:{FONT};color:#fff;display:flex;align-items:center;gap:56px;padding:0 80px;box-sizing:border-box">
<div style="display:flex;flex-direction:column;gap:22px;flex:1">
  <div style="display:flex;align-items:center;gap:18px"><img src="{LOGO}" width="84" height="84"><span style="font-size:34px;font-weight:700">Keyboard Layout Fixer</span></div>
  <div style="font-size:54px;font-weight:700;line-height:1.15">Typed <span style="color:#7fe3d8">akuo</span>?<br>Press <span style="display:inline-block;border:2px solid #fff;border-bottom-width:6px;border-radius:12px;padding:0 16px">F10</span></div>
  <div style="font-size:26px;color:#c9d8ef">Hebrew &#8644; English layout mistakes, fixed with one key.<br>Windows 10 &amp; 11 · free and open source</div>
</div>
<div style="width:420px;background:#fff;border-radius:16px;box-shadow:0 20px 50px rgba(0,0,0,.35);overflow:hidden;color:#17202e">
  <div style="background:#e9eef6;padding:10px 16px;font-size:16px;color:#4b5a70">Message</div>
  <div style="padding:26px 22px;font-size:40px;direction:rtl;text-align:right">שלום, מה שלומך?</div>
  <div style="margin:0 22px 24px;background:#0f2a4f;color:#fff;border-radius:10px;padding:10px 16px;font-size:24px;display:flex;justify-content:space-between"><span>akuo</span><span>&#8594;</span><span>שלום</span></div>
</div></body></html>'''

def frame_html(text, rtl, key_down, bubble, caption):
    k = 'background:#13a89e;color:#fff;transform:translateY(3px);border-bottom-width:2px' if key_down else 'background:#e9eef6;color:#17202e'
    b = f'<div style="position:absolute;top:18px;left:50%;transform:translateX(-50%);background:#0f2a4f;color:#fff;border-radius:10px;padding:8px 18px;font-size:22px">{bubble}</div>' if bubble else ''
    caret = '<span style="display:inline-block;width:3px;height:42px;background:#17202e;vertical-align:middle;margin:0 2px"></span>'
    return f'''<html><body style="margin:0;width:800px;height:340px;background:#f4f7fb;font-family:{FONT};position:relative">
{b}
<div style="position:absolute;top:78px;left:60px;width:680px;background:#fff;border:1px solid #d9e2ee;border-radius:12px;box-shadow:0 8px 24px rgba(15,42,79,.12)">
  <div style="background:#e9eef6;border-radius:12px 12px 0 0;padding:8px 14px;font-size:15px;color:#4b5a70">Notepad</div>
  <div style="padding:22px 24px;font-size:40px;min-height:52px;direction:{'rtl' if rtl else 'ltr'};text-align:{'right' if rtl else 'left'};color:#17202e">{text}{caret}</div>
</div>
<div style="position:absolute;top:262px;left:60px;display:flex;align-items:center;gap:16px;font-size:20px;color:#4b5a70">
  <span style="display:inline-flex;min-width:70px;height:46px;align-items:center;justify-content:center;border:1px solid #c5d0df;border-bottom-width:5px;border-radius:9px;font-weight:700;font-size:22px;{k}">F10</span>{caption}</div>
</body></html>'''

def main():
    frames = []
    seq = []
    typed = ''
    for c in 'akuo':
        typed += c
        seq.append((typed, False, False, '', 'Oops, wrong keyboard layout…', 280))
    seq.append(('akuo', False, False, '', 'Oops, wrong keyboard layout…', 900))
    seq.append(('akuo', False, True, '', 'Press F10', 350))
    seq.append(('שלום', True, False, 'akuo &#8594; שלום', 'Fixed, and the keyboard is now in Hebrew', 2400))
    with sync_playwright() as p:
        b = p.chromium.launch()
        pg = b.new_page(viewport={'width': 1280, 'height': 640})
        pg.set_content(COVER); pg.wait_for_timeout(300)
        pg.screenshot(path=os.path.join(DOCS, 'cover.png'))
        pg = b.new_page(viewport={'width': 800, 'height': 340})
        for text, rtl, kd, bub, cap, ms in seq:
            pg.set_content(frame_html(text, rtl, kd, bub, cap)); pg.wait_for_timeout(80)
            frames.append((Image.open(io.BytesIO(pg.screenshot())).convert('RGB'), ms))
        b.close()
    pal = [f.quantize(colors=128, method=Image.Quantize.MEDIANCUT) for f, _ in frames]
    pal[0].save(os.path.join(DOCS, 'demo.gif'), save_all=True, append_images=pal[1:], duration=[ms for _, ms in frames], loop=0, optimize=True)
    for n in ('cover.png', 'demo.gif'):
        print(n, os.path.getsize(os.path.join(DOCS, n)), 'octets')

main()
