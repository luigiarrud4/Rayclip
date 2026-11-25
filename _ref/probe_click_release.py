"""Clique de verdade (SendInput) na build instalada muda a seleção?

Sem CDP: foto da janela antes/depois do clique e comparação de pixels. A linha
selecionada muda de fundo e o painel de detalhes troca de conteúdo, então
qualquer mudança confirma que a seleção se moveu; zero mudança confirma o bug.
"""

import ctypes
import os
import subprocess
import cdp_env
import time
from ctypes import wintypes as w

import PIL.Image
import PIL.ImageChops

import entrada  # noqa: F401  (garante sizeof(INPUT) == 40)
import harness_compare as h
import shot_px

user32 = ctypes.windll.user32
user32.SetForegroundWindow.argtypes = [w.HWND]
user32.GetWindowRect.argtypes = [w.HWND, ctypes.POINTER(w.RECT)]

EXE = os.environ.get(
    "RAYCLIP_EXE", "C:\\Users\\Lugia\\AppData\\Local\\Rayclip\\rayclip.exe"
)
TMP = "C:/Users/Lugia/AppData/Local/Temp"


def mostrar():
    """Sinaliza a instância viva para a janela aparecer.

    A janela se esconde ao perder o foco, e qualquer console que o teste abra já
    tira o foco dela — por isso mostrar é o primeiro passo.
    """
    # `cmd /c start` abre um console novo, que rouba o foco e faz o app se
    # esconder antes do teste. `DETACHED_PROCESS | CREATE_NO_WINDOW` sinaliza a
    # instância viva sem janela nenhuma.
    subprocess.Popen(
        [EXE],
        creationflags=0x00000008 | 0x08000000,  # DETACHED_PROCESS | CREATE_NO_WINDOW
        env=cdp_env.env_cdp(),
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    for _ in range(12):
        time.sleep(0.5)
        hwnd = h.find_window("rayclip.exe")
        if hwnd:
            return hwnd
    return None


def retangulo(hwnd):
    r = w.RECT()
    user32.GetWindowRect(hwnd, ctypes.byref(r))
    return r


def clicar(x, y):
    """Clique esquerdo absoluto (uniao INPUT correta, ver `entrada.py`)."""
    from entrada import clicar as _clicar

    _clicar(x, y)


def foto(hwnd, rect, destino):
    shot_px.foto(hwnd, rect, destino)
    return PIL.Image.open(destino).convert("RGB")


def diff(a, b):
    d = PIL.ImageChops.difference(a, b).convert("L")
    return sum(1 for p in d.get_flattened_data() if p > 12)


if __name__ == "__main__":
    hwnd = mostrar()
    if not hwnd:
        print("janela nao encontrada")
        raise SystemExit(1)
    rect = retangulo(hwnd)
    print("janela:", hwnd, (rect.left, rect.top, rect.right, rect.bottom))
    user32.SetForegroundWindow(hwnd)
    time.sleep(0.6)

    atual = foto(hwnd, rect, f"{TMP}/sel_a.png")
    for rotulo, offset in (("4a linha", 199), ("5a linha", 239), ("3a linha", 159)):
        clicar(rect.left + 150, rect.top + offset)
        time.sleep(0.8)
        nova = foto(hwnd, rect, f"{TMP}/sel_b.png")
        print(f"{rotulo}: pixels mudados = {diff(atual, nova)}")
        atual = nova
