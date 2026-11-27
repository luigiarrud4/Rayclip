"""Translúcido + navegar links com as setas: quem morre primeiro?

Relato: depois de um tempo com o modo translúcido ligado e passando pelas setas
sobre vários links, o app **perde a transparência**, o **atalho de abrir/fechar
para de funcionar** e os **temas param de responder**.

Este script reproduz o caminho: liga o translúcido, abre pelo atalho e navega
com as setas para baixo sobre as entradas de link, medindo a cada passo:

- quantos `msedgewebview2.exe` filhos do Rayclip existem (a janela oculta do
  `webmeta` é um Chromium inteiro por link visitado);
- uso de memória do processo principal;
- se o backdrop do DWM continua acrílico;
- se o atalho ainda responde (hide/show).

Uso (app aberto com RAYCLIP_CDP=1, modo translúcido ligado):
    python _ref/teste_navegar_links.py 25
"""
import ctypes
import subprocess
import sys
import time
from ctypes import wintypes as w

from PIL import ImageGrab

sys.path.insert(0, "_ref")
import entrada
import teste_fundo
import teste_tray_clique as C

dwmapi = ctypes.windll.dwmapi
u = ctypes.windll.user32
u.GetWindowRect.argtypes = [w.HWND, ctypes.POINTER(w.RECT)]
u.IsWindowVisible.argtypes = [w.HWND]
VK_DOWN, VK_UP = 0x28, 0x26


def filhos_webview2():
    saida = subprocess.run(
        [
            "powershell",
            "-NoProfile",
            "-Command",
            "(Get-CimInstance Win32_Process -Filter \"Name='msedgewebview2.exe'\" | "
            "Where-Object { $_.CommandLine -like '*com.rayclip.app*' }).Count",
        ],
        capture_output=True,
        text=True,
    ).stdout.strip()
    return int(saida or 0)


def mem_mb(nome="rayclip"):
    saida = subprocess.run(
        [
            "powershell",
            "-NoProfile",
            "-Command",
            "Get-Process %s -ErrorAction SilentlyContinue | Measure-Object WorkingSet64 -Sum | "
            "ForEach-Object { [int]($_.Sum/1MB) }" % nome,
        ],
        capture_output=True,
        text=True,
    ).stdout.strip()
    return int(saida or 0)


def backdrop(hw):
    v = ctypes.c_int()
    hr = dwmapi.DwmGetWindowAttribute(hw, 38, ctypes.byref(v), 4)
    return hr, v.value


def estado(hw, rotulo):
    print(
        "%-14s webview2=%2d  rayclip=%4d MB  backdrop=%s  visivel=%s"
        % (
            rotulo,
            filhos_webview2(),
            mem_mb(),
            backdrop(hw)[1],
            bool(u.IsWindowVisible(hw)),
        )
    )


def atalho_mostra(hw):
    entrada.tecla(0x12, cima=False)
    entrada.tecla(0x20, cima=False)
    entrada.tecla(0x20, cima=True)
    entrada.tecla(0x12, cima=True)
    time.sleep(0.7)
    return bool(u.IsWindowVisible(hw))


def main():
    voltas = int(sys.argv[1]) if len(sys.argv) > 1 else 25
    hw = C.achar()
    if not hw:
        raise SystemExit("rayclip nao esta aberto")
    s = teste_fundo.Sessao()
    s.js("(() => window.__TAURI_INTERNALS__.invoke('save_settings', {cfg: null}))()") if False else None

    if not u.IsWindowVisible(hw):
        if not atalho_mostra(hw):
            raise SystemExit("nao abriu pelo atalho")
    u.SetForegroundWindow(hw)
    time.sleep(0.8)

    # quantas entradas de link existem no historico
    n_links = s.js(
        "(() => window.__TAURI_INTERNALS__.invoke('list_entries', {})"
        ".then(r => r.filter(e => e.kind === 'link').length))()"
    )
    print("entradas de link no historico:", n_links)
    entrada.tecla(0x09, cima=False)  # Tab: sai do campo de busca para a lista
    entrada.tecla(0x09, cima=True)
    time.sleep(0.3)

    estado(hw, "inicio")
    for i in range(1, voltas + 1):
        entrada.tecla(VK_DOWN)
        time.sleep(0.55)
        if i % 5 == 0:
            estado(hw, "seta x%d" % i)

    estado(hw, "depois")
    print("atalho ainda fecha?", end=" ")
    antes = bool(u.IsWindowVisible(hw))
    atalho_mostra(hw)
    depois = bool(u.IsWindowVisible(hw))
    print("mudou de estado:", antes != depois, "(antes=%s depois=%s)" % (antes, depois))

    print("backdrop final (3 = acrilico, 1 = nenhum):", backdrop(hw))
    return 0


if __name__ == "__main__":
    sys.exit(main())
