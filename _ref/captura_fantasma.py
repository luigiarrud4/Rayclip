"""Quem pinta a barra clássica nos primeiros quadros?

Abre o Rayclip pelo atalho e, durante ~1,4 s, tira capturas da faixa de topo da
janela e pergunta ao Windows **qual janela está naquele pixel** (`WindowFromPoint`),
com classe, pid e dono. Assim a barra deixa de ser suposição: se o hwnd for o
nosso, é o nosso processo pintando; se for outro, é outra janela por cima.

Uso:
    python _ref/captura_fantasma.py            # 8 aberturas
    python _ref/captura_fantasma.py 20         # 20 aberturas
"""
import ctypes
import sys
import time
from ctypes import wintypes as w

from PIL import ImageGrab

sys.path.insert(0, "_ref")
import entrada
import teste_tray_clique as C

user32 = ctypes.windll.user32
user32.GetWindowRect.argtypes = [w.HWND, ctypes.POINTER(w.RECT)]
user32.WindowFromPoint.argtypes = [w.POINT]
user32.WindowFromPoint.restype = w.HWND
user32.GetClassNameW.argtypes = [w.HWND, w.LPWSTR, ctypes.c_int]
user32.GetWindowTextW.argtypes = [w.HWND, w.LPWSTR, ctypes.c_int]
user32.GetWindowThreadProcessId.argtypes = [w.HWND, ctypes.POINTER(w.DWORD)]
user32.IsWindowVisible.argtypes = [w.HWND]
user32.GetAncestor.argtypes = [w.HWND, ctypes.c_uint]
user32.GetAncestor.restype = w.HWND

VK_MENU, VK_SPACE = 0x12, 0x20
GA_ROOT = 2


def atalho():
    entrada.tecla(VK_MENU, cima=False)
    entrada.tecla(VK_SPACE, cima=False)
    entrada.tecla(VK_SPACE, cima=True)
    entrada.tecla(VK_MENU, cima=True)


def nome(hwnd):
    if not hwnd:
        return "(nenhuma)"
    raiz = user32.GetAncestor(hwnd, GA_ROOT) or hwnd
    buf = ctypes.create_unicode_buffer(256)
    user32.GetClassNameW(hwnd, buf, 256)
    classe = buf.value
    buf2 = ctypes.create_unicode_buffer(256)
    user32.GetWindowTextW(raiz, buf2, 256)
    pid = w.DWORD()
    user32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
    dono = "" if raiz == hwnd else " (filho; raiz=0x%X)" % raiz
    return "%s pid=%d titulo=%r%s" % (classe, pid.value, buf2.value, dono)


def analisa(img):
    """Devolve (tem_faixa, tem_vermelho, luminancia_media).

    "Faixa" = **uma banda clara atravessando a largura toda** e uniforme (desvio
    baixo). Só isso conta: pixel vermelho solto aparece em qualquer conteúdo do
    app e já deu falso positivo.
    """
    img = img.convert("RGB")
    px = list(img.getdata())
    med = sum((r + g + b) / 3 for r, g, b in px) / len(px)
    desvio = (sum(((r + g + b) / 3 - med) ** 2 for r, g, b in px) / len(px)) ** 0.5
    verm = sum(1 for r, g, b in px if r > 110 and r > g + 40 and r > b + 40)
    faixa = med > 70 and desvio < 40
    return faixa, verm > 240, med


def main():
    voltas = int(sys.argv[1]) if len(sys.argv) > 1 else 8
    hw = C.achar()
    if not hw:
        raise SystemExit("rayclip nao esta aberto")
    if user32.IsWindowVisible(hw):
        user32.ShowWindow(hw, 0)
        time.sleep(0.8)
    r = w.RECT()
    user32.GetWindowRect(hw, ctypes.byref(r))
    if r.right - r.left < 200:
        raise SystemExit("janela sem posicao conhecida; abra uma vez pelo atalho")
    topo = (r.left, r.top, r.right, r.top + 34)
    alvo = w.POINT()
    alvo.x = r.right - 60
    alvo.y = r.top + 12

    achados = 0
    for volta in range(1, voltas + 1):
        atalho()
        achou = False
        for k in range(14):
            img = ImageGrab.grab(bbox=topo)
            faixa, verm, med = analisa(img)
            hwnd_ponto = user32.WindowFromPoint(alvo)
            if faixa:
                achou = True
                print(
                    "  volta %d quadro %2d: media=%.1f  janela no ponto: %s"
                    % (volta, k, med, nome(hwnd_ponto))
                )
                img.save(r"C:\Windows\System32\_rayclip_tmpantasma_%d_%d.png" % (volta, k))
                break
            time.sleep(0.03)
        if achou:
            achados += 1
        time.sleep(0.5)
        atalho()  # fecha
        time.sleep(0.9)
    print("aberturas com barra detectada: %d de %d" % (achados, voltas))


if __name__ == "__main__":
    main()
