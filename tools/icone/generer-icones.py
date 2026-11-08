"""Icône 1.5.4 : SVG MAÎTRE (vectoriel) + rendu de chaque taille avec ajustement à la grille de pixels.
python3 svg.py -> maitre.svg, maitre-pause.svg, png/<n>.png, png/pause-<n>.png, klf.ico, klf-pause.ico"""
import os, subprocess, io
from fontTools.ttLib import TTFont
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.boundsPen import BoundsPen
from PIL import Image
ICI = os.path.dirname(os.path.abspath(__file__))
NAVY, TURQ, GREY1, GREY2 = '#1F3A5F', '#00B4C8', '#666666', '#AAAAAA'
FA = '/usr/share/fonts/truetype/dejavu/DejaVuSansCondensed-Bold.ttf'
FH = '/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf'

def glyphe(police, ch):
    f = TTFont(police); gs = f.getGlyphSet(); nom = f.getBestCmap()[ord(ch)]
    bp = BoundsPen(gs); gs[nom].draw(bp); sp = SVGPathPen(gs); gs[nom].draw(sp)
    return sp.getCommands(), bp.bounds   # coordonnées police (y vers le haut)

def place(path, b, cx, cy, h, snap=None, wmax=None):
    """transform SVG qui met le glyphe à la hauteur h (largeur au plus wmax), centré en (cx, cy) ;
    snap = pas de la grille de pixels."""
    x0, y0, x1, y1 = b; k = h / (y1 - y0)
    if wmax and (x1 - x0) * k > wmax:
        k = wmax / (x1 - x0)
    tx, ty = cx - (x0 + x1) / 2 * k, cy + (y0 + y1) / 2 * k
    if snap:  # bas du glyphe et bord gauche sur des pixels entiers
        bas = ty - y0 * k; ty += round(bas / snap) * snap - bas
        gauche = tx + x0 * k; tx += round(gauche / snap) * snap - gauche
    return f'<path transform="translate({tx:.3f} {ty:.3f}) scale({k:.5f} {-k:.5f})" d="{path}" fill="#FFFFFF"/>'

PA, BA = glyphe(FA, 'A'); PH, BH = glyphe(FH, 'א')

def svg(n=256, pause=False):
    """Choix du 09/11 (variante OpenAI 1) : carré bleu plein, A blanc et aleph turquoise
    côte à côte, très gras. Unités de 256 ; ajustement à la grille de pixels si n <= 48."""
    c1, c2 = (GREY1, '#D8D8D8') if pause else (NAVY, TURQ)
    px = 256 / n; snap = px if n <= 48 else None
    r = lambda v: round(v / px) * px if snap else v
    m = 0 if n <= 24 else r(256 * 0.03); rad = 256 * (0.17 if n <= 24 else 0.19)
    h = r(256 * (0.72 if n <= 20 else 0.66))      # hauteur des lettres (plus grandes en tout petit)
    cy = 256 * 0.52
    s = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" width="{n}" height="{n}">',
         f'<rect x="{m}" y="{m}" width="{256 - 2 * m}" height="{256 - 2 * m}" rx="{rad:.2f}" fill="{c1}"/>',
         place(PA, BA, 256 * 0.275, cy, h, snap, wmax=256 * 0.44),
         place(PH, BH, 256 * 0.725, cy, h * 0.97, snap, wmax=256 * 0.42).replace('fill="#FFFFFF"', f'fill="{c2}"'), '</svg>']
    return '\n'.join(s)

def rendu(n, pause=False):
    out = subprocess.run(['rsvg-convert', '-w', str(n), '-h', str(n)], input=svg(n, pause).encode(), capture_output=True, check=True).stdout
    return Image.open(io.BytesIO(out)).convert('RGBA')

if __name__ == '__main__':
    open(os.path.join(ICI, 'maitre.svg'), 'w').write(svg())
    open(os.path.join(ICI, 'maitre-pause.svg'), 'w').write(svg(pause=True))
    os.makedirs(os.path.join(ICI, 'png'), exist_ok=True)
    TA = [16, 20, 24, 32, 40, 48, 64, 128, 256]
    for p in (False, True):
        ims = [rendu(n, p) for n in TA]
        for n, im in zip(TA, ims): im.save(os.path.join(ICI, 'png', ('pause-' if p else '') + f'{n}.png'))
        ims[-1].save(os.path.join(ICI, 'klf-pause.ico' if p else 'klf.ico'), sizes=[(n, n) for n in TA], append_images=ims[:-1])
    print('ok')
