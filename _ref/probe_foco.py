"""Diagnóstico: quem tem o foco depois de cada clique sintético?"""

import ctypes
import time
from ctypes import wintypes as w

import harness_compare as h
import probe_click_release as p

u32 = ctypes.windll.user32
u32.GetWindowThreadProcessId.argtypes = [w.HWND, ctypes.POINTER(w.DWORD)]


def foco_info():
    fg = u32.GetForegroundWindow()
    pid = w.DWORD(0)
    u32.GetWindowThreadProcessId(fg, ctypes.byref(pid))
    buf = ctypes.create_unicode_buffer(120)
    u32.GetWindowTextW(fg, buf, 120)
    try:
        nome = h.proc_name(pid.value)
    except Exception:
        nome = "?"
    return nome, buf.value[:30]


if __name__ == "__main__":
    hwnd = p.mostrar()
    rect = p.retangulo(hwnd)
    u32.SetForegroundWindow(hwnd)
    time.sleep(0.6)
    print("antes   :", foco_info(), "visivel:", bool(u32.IsWindowVisible(hwnd)))
    for off in (159, 199, 239):
        p.clicar(rect.left + 150, rect.top + off)
        time.sleep(0.7)
        print(f"y+{off:3d} :", foco_info(), "visivel:", bool(u32.IsWindowVisible(hwnd)))
