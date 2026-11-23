"""GIF no clipboard (formato cru "GIF", como um navegador publica): anima?

Compara os dois apps no caso em que a imagem NAO e um arquivo: os bytes do GIF vao
para a area de transferencia no formato registrado "GIF" + primeiro quadro em
CF_DIB. Mede se o preview anima (pixels que mudam de verdade, diferenca > 90).
"""
import ctypes
import json
import os
import struct
import subprocess
import sys
import time
import uuid
from ctypes import wintypes as w

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import harness_compare as h  # noqa: E402
from PIL import Image, ImageChops, ImageDraw, ImageGrab  # noqa: E402
from test_img import cdp  # noqa: E402
from test_pm_raycast import janelas  # noqa: E402

SAIDA = r"C:\Windows\System32\_rayclip_tmp"
TEMP = os.environ["TEMP"]

user32 = ctypes.WinDLL("user32", use_last_error=True)
kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
user32.OpenClipboard.argtypes = [w.HWND]
user32.SetClipboardData.argtypes = [w.UINT, w.HANDLE]
user32.SetClipboardData.restype = w.HANDLE
user32.RegisterClipboardFormatW.argtypes = [w.LPCWSTR]
user32.RegisterClipboardFormatW.restype = w.UINT
kernel32.GlobalAlloc.argtypes = [w.UINT, ctypes.c_size_t]
kernel32.GlobalAlloc.restype = w.HGLOBAL
kernel32.GlobalLock.argtypes = [w.HGLOBAL]
kernel32.GlobalLock.restype = ctypes.c_void_p
kernel32.GlobalUnlock.argtypes = [w.HGLOBAL]


def _global(dados):
    hmem = kernel32.GlobalAlloc(0x0002, len(dados))
    p = kernel32.GlobalLock(hmem)
    ctypes.memmove(p, dados, len(dados))
    kernel32.GlobalUnlock(hmem)
    return hmem


def dib_de(im):
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
    return struct.pack("<IiiHHIIiiII", 40, larg, alt, 1, 24, 0, len(pixels), 2835, 2835, 0, 0) + pixels


def criar_gif():
    pasta = os.path.join(TEMP, "imgtest")
    os.makedirs(pasta, exist_ok=True)
    caminho = os.path.join(pasta, f"clip{uuid.uuid4().hex[:6]}.gif")
    quadros = []
    # marca aleatoria: conteudo identico e deduplicado pelos dois apps
    marca = [__import__("random").randrange(1, 255) for _ in range(3)]
    for i, cor in enumerate([(230, 30, 30), (30, 210, 80), (40, 80, 240)]):
        im = Image.new("RGB", (240, 240), cor)
        d = ImageDraw.Draw(im)
        d.ellipse([60 + i * 20, 60, 140 + i * 20, 140], fill=(255, 255, 255))
        d.point((marca[0], marca[1] + marca[2] % 200), fill=(marca[2], marca[1], marca[0]))
        quadros.append(im)
    quadros[0].save(caminho, save_all=True, append_images=quadros[1:], duration=200, loop=0)
    return caminho, quadros[0]


def copiar_gif(caminho, primeiro):
    dados = open(caminho, "rb").read()
    fmt = user32.RegisterClipboardFormatW("GIF") or user32.RegisterClipboardFormatW("image/gif")
    if not user32.OpenClipboard(None):
        raise RuntimeError("clipboard ocupado")
    user32.EmptyClipboard()
    user32.SetClipboardData(fmt, _global(dados))
    user32.SetClipboardData(8, _global(dib_de(primeiro)))  # CF_DIB, como os navegadores
    user32.CloseClipboard()


def rect(app):
    pid = int(subprocess.run(["powershell", "-NoProfile", "-Command",
                              f"(Get-Process {app}).Id"],
                             capture_output=True, text=True).stdout.split()[0])
    for _ in range(25):
        cands = [c for c in janelas(pid) if c[0] > 20000]
        if cands:
            return cands[0][2]
        time.sleep(0.1)
    return None


def analisa(a, b):
    ia = Image.open(a).convert("RGB")
    ib = Image.open(b).convert("RGB")
    d = ImageChops.difference(ia, ib)
    px = list(d.getdata())
    fortes = [(i % d.size[0], i // d.size[0]) for i, p in enumerate(px) if sum(p) > 90]
    if not fortes:
        return 0, None
    xs = [p[0] for p in fortes]
    ys = [p[1] for p in fortes]
    return len(fortes), (min(xs), min(ys), max(xs), max(ys))


def selecionar_gif(alvo_id):
    """Anda com a seta ate a linha do GIF ficar selecionada (id conhecido)."""
    for _ in range(12):
        sel = cdp("JSON.stringify([...document.querySelectorAll('.row')]"
                  ".filter(r => r.dataset.selected === 'true').map(r => r.dataset.id))")
        atual = json.loads(sel or "[]")
        if atual and atual[0] == alvo_id:
            return True
        h.send_keys("down")
        time.sleep(0.25)
    return False


def main():
    gif, primeiro = criar_gif()
    print("gif:", gif)

    # ---------- Rayclip ----------
    try:
        copiar_gif(gif, primeiro)
        time.sleep(1.8)
        novo = cdp("(async () => { const {invoke} = window.__TAURI_INTERNALS__;"
                   " const l = await invoke('list_entries', { offset: 0, limit: 3 });"
                   " return JSON.stringify(l.map(e => ({id: e.id, kind: e.kind, gif: e.gif}))); })()")
        print("rayclip novos:", novo)
        alvo = json.loads(novo)[0]["id"]
        h.send_keys("esc")
        time.sleep(0.4)
        h.send_keys("alt", "space")
        time.sleep(1.6)
        ok = selecionar_gif(alvo)
        print("rayclip selecionou o gif:", ok)
        r = rect("rayclip")
        if r:
            a = os.path.join(SAIDA, "gif2_rayclip_1.png")
            b = os.path.join(SAIDA, "gif2_rayclip_2.png")
            ImageGrab.grab(bbox=r).save(a)
            time.sleep(0.40)
            ImageGrab.grab(bbox=r).save(b)
            print("rayclip:", analisa(a, b))
    except Exception as e:
        print("rayclip falhou:", e)

    # ---------- Raycast ----------
    try:
        copiar_gif(gif, primeiro)
        time.sleep(2.0)
        h.send_keys("esc")
        time.sleep(0.5)
        h.send_keys("ctrl", "shift", "v")
        time.sleep(1.8)
        r = rect("raycast")
        if r:
            a = os.path.join(SAIDA, "gif2_raycast_1.png")
            b = os.path.join(SAIDA, "gif2_raycast_2.png")
            ImageGrab.grab(bbox=r).save(a)
            time.sleep(0.40)
            ImageGrab.grab(bbox=r).save(b)
            print("raycast:", analisa(a, b))
    except Exception as e:
        print("raycast falhou:", e)


if __name__ == "__main__":
    main()
