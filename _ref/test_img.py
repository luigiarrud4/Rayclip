"""Latencia de captura de IMAGEM e animacao de GIF: Rayclip x Raycast.

Copia um PNG pela area de transferencia (CF_DIB, como um navegador faz) e mede
quanto tempo cada app leva para ter o item no historico:

  raycast  -> aparece arquivo novo em %LOCALAPPDATA%\\Raycast\\clipboard
  rayclip  -> um id novo aparece em list_entries (sondagem dentro da pagina,
              disparada antes da copia e lida depois, sem bloquear o CDP)
"""
import ctypes
import json
import os
import struct
import sys
import time
import uuid
from ctypes import wintypes as w

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from PIL import Image, ImageDraw  # noqa: E402

TEMP = os.environ["TEMP"]
RAYCAST_CLIP = os.path.join(os.environ["LOCALAPPDATA"], "Raycast", "clipboard")

user32 = ctypes.WinDLL("user32", use_last_error=True)
kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
user32.OpenClipboard.argtypes = [w.HWND]
user32.SetClipboardData.argtypes = [w.UINT, w.HANDLE]
user32.SetClipboardData.restype = w.HANDLE
kernel32.GlobalAlloc.argtypes = [w.UINT, ctypes.c_size_t]
kernel32.GlobalAlloc.restype = w.HGLOBAL
kernel32.GlobalLock.argtypes = [w.HGLOBAL]
kernel32.GlobalLock.restype = ctypes.c_void_p
kernel32.GlobalUnlock.argtypes = [w.HGLOBAL]


def _global(dados):
    hmem = kernel32.GlobalAlloc(0x0002, len(dados))  # GMEM_MOVEABLE
    p = kernel32.GlobalLock(hmem)
    ctypes.memmove(p, dados, len(dados))
    kernel32.GlobalUnlock(hmem)
    return hmem


def png_para_dib(caminho):
    """BITMAPINFOHEADER + pixels BGR de baixo para cima = CF_DIB."""
    im = Image.open(caminho).convert("RGB")
    larg, alt = im.size
    partes = []
    for y in range(alt - 1, -1, -1):
        linha = bytearray()
        for x in range(larg):
            r, g, b = im.getpixel((x, y))
            linha += bytes((b, g, r))
        while len(linha) % 4:
            linha.append(0)
        partes.append(bytes(linha))
    pixels = b"".join(partes)
    header = struct.pack("<IiiHHIIiiII", 40, larg, alt, 1, 24, 0, len(pixels), 2835, 2835, 0, 0)
    return header + pixels


def copiar_imagem(caminho):
    dib = png_para_dib(caminho)
    if not user32.OpenClipboard(None):
        raise RuntimeError("nao abriu o clipboard")
    user32.EmptyClipboard()
    user32.SetClipboardData(8, _global(dib))  # CF_DIB
    user32.CloseClipboard()
    return time.time() * 1000.0


def arquivos_raycast():
    try:
        return {n for n in os.listdir(RAYCAST_CLIP) if n.startswith("file-")}
    except OSError:
        return set()


def latencia_raycast(caminho, limite=6.0):
    antes = arquivos_raycast()
    t0 = copiar_imagem(caminho)
    while (time.time() * 1000.0) - t0 < limite * 1000:
        if arquivos_raycast() - antes:
            return (time.time() * 1000.0) - t0
        time.sleep(0.004)
    return None


def cdp(expr, esperar=True):
    import urllib.request

    import websocket
    with urllib.request.urlopen("http://127.0.0.1:9222/json", timeout=5) as r:
        url = [t for t in json.load(r) if t.get("type") == "page"][0]["webSocketDebuggerUrl"]
    ws = websocket.create_connection(url, timeout=40)
    ws.send(json.dumps({"id": 1, "method": "Runtime.evaluate",
                        "params": {"expression": expr, "awaitPromise": esperar,
                                   "returnByValue": True}}))
    while True:
        m = json.loads(ws.recv())
        if m.get("id") == 1:
            ws.close()
            if "exceptionDetails" in m["result"]:
                raise RuntimeError(json.dumps(m["result"]["exceptionDetails"])[:300])
            res = m["result"]["result"]
            if not esperar:
                return res.get("description") or res.get("value")
            return res.get("value")


JS_ARMAR = """
(async () => {
  const { invoke } = window.__TAURI_INTERNALS__;
  const l = await invoke('list_entries', { offset: 0, limit: 40 });
  const antes = new Set(l.map(e => e.id));
  window.__novo = null;
  const fim = performance.now() + 8000;
  while (performance.now() < fim) {
    const a = await invoke('list_entries', { offset: 0, limit: 6 });
    const n = a.find(e => !antes.has(e.id));
    if (n) { window.__novo = performance.timeOrigin + performance.now(); return 'ok'; }
    await new Promise(r => setTimeout(r, 8));
  }
  return 'timeout';
})()
"""


def latencia_rayclip(caminho):
    """Arma a sondagem (sem bloquear), copia e depois le o carimbo da pagina."""
    cdp("window.__novo = null; void 0")
    cdp(JS_ARMAR, esperar=False)          # roda em segundo plano na pagina
    time.sleep(0.25)                      # deixa o baseline ser tirado
    t0 = copiar_imagem(caminho)
    for _ in range(80):
        time.sleep(0.1)
        v = cdp("window.__novo")
        if v:
            return float(v) - t0
    return None


def criar_png(pasta):
    png = os.path.join(pasta, "latencia.png")
    im = Image.new("RGB", (1280, 800))
    d = ImageDraw.Draw(im)
    for i in range(0, 1280, 40):
        d.line([(i, 0), (1280 - i, 800)], fill=(i % 255, 90, 200 - i % 200), width=3)
    d.text((40, 40), "rayclip " + uuid.uuid4().hex, fill=(255, 255, 255))
    im.save(png)
    return png


def main():
    modo = sys.argv[1] if len(sys.argv) > 1 else "raycast"
    pasta = os.path.join(TEMP, "imgtest")
    os.makedirs(pasta, exist_ok=True)
    f = latencia_raycast if modo == "raycast" else latencia_rayclip
    vals = []
    for i in range(5):
        # PNG novo a cada rodada: conteudo identico seria deduplicado
        png = criar_png(pasta)
        v = f(png)
        vals.append(v)
        print(f"  {modo} #{i+1}: " + (f"{v:.0f} ms" if v else "NAO CAPTUROU"), flush=True)
        time.sleep(0.9)
    ok = sorted(v for v in vals if v)
    print(f"{modo}: n={len(ok)}/{len(vals)} mediana=" + (f"{ok[len(ok)//2]:.0f} ms" if ok else "—")
          + (f" min={ok[0]:.0f} max={ok[-1]:.0f}" if ok else ""))


if __name__ == "__main__":
    main()
