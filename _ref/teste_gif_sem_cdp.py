"""O caso real do usuário: build **sem CDP** (nada de `RAYCLIP_CDP`).

O WebView2 cria o ambiente uma vez por processo com os argumentos da primeira
janela; com `RAYCLIP_CDP` ligado a janela principal ganha argumentos extras e a
segunda janela precisa repetir os mesmos. Aqui, sem CDP nenhum, a janela
escondida tem de nascer com os padrões do wry — é a configuração que o usuário
roda de verdade.

Fluxo: copia o link do Klipy, abre o app, digita "klipy" no filtro (a entrada
vira a única linha e é selecionada sozinha), espera a capa e confere o log.
"""

import ctypes
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import entrada  # noqa: E402
import harness_compare as h  # noqa: E402
import keysafe  # noqa: E402

# `?v=2` só para ser uma URL nova (a de antes já tem capa gravada)
URL = "https://klipy.com/gifs/lamar-chop?v=13"
LOG = os.path.join(os.environ["TEMP"], "rayclip.log")
VK = {"k": 0x4B, "l": 0x4C, "i": 0x49, "p": 0x50, "y": 0x59}


def diz(m):
    print(m, flush=True)


def texto(t):
    u = ctypes.windll.user32
    k = ctypes.windll.kernel32
    u.OpenClipboard.argtypes = [ctypes.c_void_p]
    u.OpenClipboard.restype = ctypes.c_int
    u.EmptyClipboard.argtypes = []
    u.CloseClipboard.argtypes = []
    u.SetClipboardData.argtypes = [ctypes.c_uint, ctypes.c_void_p]
    u.SetClipboardData.restype = ctypes.c_void_p
    k.GlobalAlloc.argtypes = [ctypes.c_uint, ctypes.c_size_t]
    k.GlobalAlloc.restype = ctypes.c_void_p
    k.GlobalLock.argtypes = [ctypes.c_void_p]
    k.GlobalLock.restype = ctypes.c_void_p
    k.GlobalUnlock.argtypes = [ctypes.c_void_p]
    blob = (t + chr(0)).encode("utf-16-le")
    u.OpenClipboard(None)
    try:
        u.EmptyClipboard()
        hmem = k.GlobalAlloc(0x0002, len(blob))
        p = k.GlobalLock(hmem)
        ctypes.memmove(p, blob, len(blob))
        k.GlobalUnlock(hmem)
        u.SetClipboardData(13, hmem)
    finally:
        u.CloseClipboard()


def main():
    texto(URL)
    time.sleep(0.6)

    exe = os.path.join(
        os.environ["LOCALAPPDATA"], "Rayclip", "rayclip.exe"
    )
    env = dict(os.environ)
    env.pop("RAYCLIP_CDP", None)
    env["RAYCLIP_LOG"] = "1"  # release só escreve log com isto
    import subprocess

    subprocess.Popen([exe, "--open"], env=env, close_fds=True)
    time.sleep(6)

    hwnd = h.find_window("rayclip.exe")
    if not hwnd:
        diz("app nao subiu")
        return 1
    keysafe.focar(hwnd)
    time.sleep(0.4)
    for letra in "klipy":
        entrada.tecla(VK[letra])
        time.sleep(0.06)
    diz("filtro digitado")
    COVERS = os.path.join(os.environ["APPDATA"], "com.rayclip.app", "clipboard", "media", "covers")
    antes = set(os.listdir(COVERS)) if os.path.isdir(COVERS) else set()
    # clica na primeira linha (as coordenadas das linhas são fixas em relação à
    # janela; sem CDP não há como perguntar ao DOM)
    import ctypes as ct
    from ctypes import wintypes as wt

    r = wt.RECT()
    ct.windll.user32.GetWindowRect.argtypes = [wt.HWND, ct.POINTER(wt.RECT)]
    ct.windll.user32.GetWindowRect(hwnd, ct.byref(r))
    x = r.left + 150
    y = r.top + 179
    entrada.clicar(x, y)
    diz(f"clique em ({x},{y}) janela=({r.left},{r.top})-({r.right},{r.bottom})")

    achou = None
    for _ in range(30):
        time.sleep(1.0)
        try:
            with open(LOG, "r", encoding="utf-8", errors="replace") as f:
                linhas = [l for l in f.read().splitlines() if "webmeta:" in l]
        except OSError:
            linhas = []
        if any("capa de" in l for l in linhas):
            achou = linhas[-3:]
            break
    if achou:
        for l in achou:
            diz(l)
        diz("RESULTADO: OK")
        return 0

    # Build de release não escreve log (`cfg!(debug_assertions)`): a prova fica
    # no disco — uma capa nova em `media/covers/`, com bytes de vídeo mp4/webm.
    novas = sorted(set(os.listdir(COVERS)) - antes)
    for nome in novas:
        caminho = os.path.join(COVERS, nome)
        with open(caminho, "rb") as f:
            cabeca = f.read(12)
        vid = cabeca[4:8] == b"ftyp" or cabeca[8:12] == b"WEBM"
        diz(f"capa nova: {nome} {os.path.getsize(caminho)} B video={vid}")
        if vid and os.path.getsize(caminho) > 200_000:
            diz("RESULTADO: OK")
            return 0
    diz("sem webmeta no log (sem CDP)")
    diz("RESULTADO: FALHOU")
    return 1


if __name__ == "__main__":
    sys.exit(main())
