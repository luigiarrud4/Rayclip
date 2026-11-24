"""Quem rouba o foco do app durante o teste? (SetWinEventHook no foco)

Imprime, com carimbo de tempo, cada troca de janela em primeiro plano e cada
mudança de visibilidade da janela do app enquanto o roteiro foca e clica.
"""

import ctypes
import io
import os
import sys
import time
from ctypes import wintypes as w

import harness_compare as h
import keysafe as k
import probe_click_release as p

u32 = ctypes.windll.user32
u32.GetWindowThreadProcessId.argtypes = [w.HWND, ctypes.POINTER(w.DWORD)]
u32.GetClassNameW.argtypes = [w.HWND, ctypes.c_wchar_p, ctypes.c_int]

SAIDA = os.path.join(os.environ["TEMP"], "foco_trace.txt")
linhas = []
t0 = time.time()
WINEVENTPROC = ctypes.WINFUNCTYPE(
    None, w.HANDLE, w.DWORD, w.HWND, w.LONG, w.LONG, w.DWORD, w.DWORD
)


def diz(msg):
    linhas.append(f"{time.time() - t0:7.3f} {msg}")
    if sys.stdout:
        print(linhas[-1])


def descreve(hwnd):
    if not hwnd:
        return "-"
    pid = w.DWORD(0)
    u32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
    buf = ctypes.create_unicode_buffer(128)
    u32.GetClassNameW(hwnd, buf, 128)
    try:
        nome = h.proc_name(pid.value)
    except Exception:
        nome = "?"
    return f"{nome}:{buf.value}"


def main():
    hwnd = p.mostrar()
    if not hwnd:
        diz("sem janela")
        return
    rect = p.retangulo(hwnd)

    def ao_mudar(_hook, _ev, h2, _o, _c, _t, _ms):
        if h2:
            diz(f"foreground -> {descreve(h2)} | app visivel={bool(u32.IsWindowVisible(hwnd))}")

    cb = WINEVENTPROC(ao_mudar)
    hook = u32.SetWinEventHook(0x0003, 0x0003, None, cb, 0, 0, 0)  # FOREGROUND
    diz(f"hook={hook} app={hwnd} @({rect.left},{rect.top})")
    try:
        for rodada in range(2):
            k.focar(hwnd)
            diz(f"rodada {rodada}: depois de focar, fg={descreve(u32.GetForegroundWindow())}")
            time.sleep(0.02)
            p.clicar(rect.left + 150, rect.top + 159)
            diz("clique enviado")
            time.sleep(0.5)
            diz(f"fim da rodada {rodada}: visivel={bool(u32.IsWindowVisible(hwnd))}")
            time.sleep(0.3)
    finally:
        u32.UnhookWinEvent(hook)


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:  # noqa: BLE001
        diz(f"ERRO: {type(exc).__name__}: {exc}")
    io.open(SAIDA, "w", encoding="utf-8", newline="\n").write("\n".join(linhas) + "\n")
