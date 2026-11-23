"""O mesmo simulador de gerenciador de senha, agora contra o Raycast.

Copia seis conteudos marcados (prefixo comum `rcsenha`), abre o historico de
clipboard do Raycast, busca pelo prefixo e tira o print da janela **no proprio
processo** (PIL.ImageGrab), sem abrir console que roube o foco do Raycast.
"""
import io
import ctypes
import os
import shutil
import subprocess
import sys
import time
import uuid
from ctypes import wintypes as w

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import harness_compare as h  # noqa: E402
from PIL import ImageGrab  # noqa: E402

TEMP = os.environ["TEMP"]
BASE = os.path.join(TEMP, "pmtest", "base.exe")
SAIDA_DIR = r"C:\Windows\System32\_rayclip_tmp"

CONJUNTOS = {
    "1": [
        ("Bitwarden.exe", "text", "A-bitwarden"),
        ("bloco_de_notas.exe", "text", "B-normal"),
        ("bloco_de_notas.exe", "exclude", "C-exclude"),
        ("bloco_de_notas.exe", "caninclude0", "D-caninclude0"),
        ("bloco_de_notas.exe", "caninclude1", "E-caninclude1"),
        ("1Password.exe", "text", "F-1password"),
    ],
    # o caso do mundo real: gerenciador de senha que publica o marcador
    "2": [
        ("Bitwarden.exe", "exclude", "G-bitwarden-exclude"),
        ("Bitwarden.exe", "caninclude0", "H-bitwarden-caninclude0"),
        ("Bitwarden.exe", "nowin_text", "I-bitwarden-sem-janela"),
        ("1Password.exe", "exclude", "J-1password-exclude"),
    ],
}

user32 = ctypes.WinDLL("user32", use_last_error=True)


def janelas(pid):
    """Todas as janelas visiveis do processo, da maior para a menor."""
    user32.EnumWindows.restype = w.BOOL
    achadas = []
    Cb = ctypes.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)

    def cb(hwnd, _l):
        p = w.DWORD()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(p))
        if p.value == pid and user32.IsWindowVisible(hwnd):
            r = w.RECT()
            user32.GetWindowRect(hwnd, ctypes.byref(r))
            area = (r.right - r.left) * (r.bottom - r.top)
            achadas.append((area, hwnd, (r.left, r.top, r.right, r.bottom)))
        return True

    user32.EnumWindows(Cb(cb), 0)
    return sorted(achadas, reverse=True)


def maior_janela(pid):
    user32.EnumWindows.restype = w.BOOL
    melhor = {"area": 0, "hwnd": None}

    Cb = ctypes.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)

    def cb(hwnd, _l):
        p = w.DWORD()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(p))
        if p.value == pid and user32.IsWindowVisible(hwnd):
            r = w.RECT()
            user32.GetWindowRect(hwnd, ctypes.byref(r))
            area = (r.right - r.left) * (r.bottom - r.top)
            if area > melhor["area"]:
                melhor["area"] = area
                melhor["hwnd"] = hwnd
                melhor["rect"] = (r.left, r.top, r.right, r.bottom)
        return True

    user32.EnumWindows(Cb(cb), 0)
    return melhor


def copiar(exe_nome, modo, texto):
    exe = os.path.join(TEMP, "pmtest", exe_nome)
    shutil.copyfile(BASE, exe)
    subprocess.run([exe, modo, texto], capture_output=True, timeout=30)


def abrir_raycast(pid, prefixo, print_saida):
    """Abre, digita e fotografa — sem subprocessos depois do atalho, para nao
    roubar o foco do Raycast (ele esconde ao perder o foco)."""
    h.send_keys("esc")      # fecha, se ja estiver aberto, para a busca vir limpa
    time.sleep(0.6)
    for tentativa in range(3):
        h.send_keys("ctrl", "shift", "v")
        time.sleep(1.8)
        h.type_text(prefixo)
        time.sleep(1.3)
        cands = janelas(pid)
        print("candidatas:", [(a, hx) for a, hx, _ in cands[:4]])
        grande = [c for c in cands if c[0] > 20000]
        if grande:
            area, hx, (l, t, r, b) = grande[0]
            img = ImageGrab.grab(bbox=(l, t, r, b))
            img.save(print_saida)
            print("PRINT:", print_saida, img.size, "area", area)
            return print_saida
        print("tentativa", tentativa + 1, "sem janela grande; reabrindo")
    return None


def main():
    pid = int(subprocess.run(["powershell", "-NoProfile", "-Command",
                              "(Get-Process Raycast).Id"],
                             capture_output=True, text=True).stdout.strip())
    print("pid do raycast:", pid)
    prefixo = "rcsenha" + uuid.uuid4().hex[:4]
    conjunto = sys.argv[1] if len(sys.argv) > 1 else "1"
    for exe, modo, sufixo in CONJUNTOS[conjunto]:
        copiar(exe, modo, f"{prefixo}-{sufixo}")
        print("copiado:", exe, modo, sufixo)
        time.sleep(1.8)
    abrir_raycast(pid, prefixo, os.path.join(SAIDA_DIR, f"raycast_{prefixo}.png"))
    print("PREFIXO:", prefixo)


def _utf8():
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")


if __name__ == "__main__":
    _utf8()
    main()
