"""Colar VÁRIOS arquivos de uma vez (v0.1.40).

Três entradas de arquivo copiadas separadamente (cada uma vira um item do
histórico) são marcadas com Ctrl+clique e coladas com Enter numa pasta do
Explorer: o esperado é **uma** `CF_HDROP` com os três caminhos e uma única
colagem — os três arquivos aparecem na pasta, com nome e bytes iguais.

Uso (com o app rodando via `_ref/reiniciar.ps1 -Cdp`):
    cd /c/Users/Lugia/Projects/rayclip && python _ref/teste_colar_arquivos.py
"""

import ctypes
import io
import json
import os
import struct
import sys
import time
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import entrada  # noqa: E402
import harness_compare as h  # noqa: E402
import keysafe  # noqa: E402
import websocket  # noqa: E402

SAIDA = os.path.join(os.environ["TEMP"], "teste_colar_arquivos.txt")
user32 = ctypes.windll.user32
kernel32 = ctypes.windll.kernel32
linhas = []

ORIGEM = os.path.join(os.environ["TEMP"], "alvo_colar_arq")
DESTINO = os.path.join(os.environ["TEMP"], "alvo_colar_destino")
NOMES = ["arq_a.txt", "arq_b.txt", "arq_c.txt"]


def diz(m):
    linhas.append(m)
    print(m, flush=True)


# ------------------------------------------------------------------ clipboard
def file_list(paths):
    """Põe uma lista de arquivos (CF_HDROP) na área de transferência."""
    user32.OpenClipboard.argtypes = [ctypes.c_void_p]
    user32.OpenClipboard.restype = ctypes.c_int
    user32.EmptyClipboard.argtypes = []
    user32.EmptyClipboard.restype = ctypes.c_int
    user32.CloseClipboard.argtypes = []
    user32.CloseClipboard.restype = ctypes.c_int
    user32.SetClipboardData.argtypes = [ctypes.c_uint, ctypes.c_void_p]
    user32.SetClipboardData.restype = ctypes.c_void_p
    kernel32.GlobalAlloc.argtypes = [ctypes.c_uint, ctypes.c_size_t]
    kernel32.GlobalAlloc.restype = ctypes.c_void_p
    kernel32.GlobalLock.argtypes = [ctypes.c_void_p]
    kernel32.GlobalLock.restype = ctypes.c_void_p
    kernel32.GlobalUnlock.argtypes = [ctypes.c_void_p]

    # DROPFILES (20 bytes) + lista de caminhos terminada em dois zeros.
    cab = struct.pack("<iiiii", 20, 0, 0, 0, 1)
    corpo = ("\0".join(paths) + "\0\0").encode("utf-16-le")
    blob = cab + corpo
    GMEM_MOVEABLE = 0x0002
    if not user32.OpenClipboard(None):
        raise RuntimeError("OpenClipboard falhou")
    try:
        user32.EmptyClipboard()
        hmem = kernel32.GlobalAlloc(GMEM_MOVEABLE, len(blob))
        if not hmem:
            raise RuntimeError("GlobalAlloc falhou")
        ptr = kernel32.GlobalLock(hmem)
        ctypes.memmove(ptr, blob, len(blob))
        kernel32.GlobalUnlock(hmem)
        if not user32.SetClipboardData(15, hmem):  # 15 = CF_HDROP
            raise RuntimeError("SetClipboardData falhou")
    finally:
        user32.CloseClipboard()


# ------------------------------------------------------------------------ CDP
class CDP:
    def __init__(self):
        alvo = json.load(urllib.request.urlopen("http://127.0.0.1:9222/json/list", timeout=10))[0]
        self.ws = websocket.create_connection(
            "ws://127.0.0.1:9222/devtools/page/" + alvo["id"], suppress_origin=True, timeout=30
        )
        self.n = 0

    def ev(self, js):
        self.n += 1
        self.ws.send(
            json.dumps(
                {
                    "id": self.n,
                    "method": "Runtime.evaluate",
                    "params": {"expression": js, "awaitPromise": True, "returnByValue": True},
                }
            )
        )
        while True:
            r = json.loads(self.ws.recv())
            if r.get("id") == self.n:
                return r["result"].get("result", {}).get("value")


def janela_explorer(titulo):
    """Janela de nível superior cujo título contém `titulo`.

    `explorer.exe <pasta>` entrega o pedido ao processo Explorer que já existe,
    então o pid de quem foi lançado não serve: o filtro é pelo título.
    """
    achado = []

    @ctypes.WINFUNCTYPE(ctypes.c_bool, ctypes.c_void_p, ctypes.c_void_p)
    def cb(hwnd, _l):
        if not user32.IsWindowVisible(hwnd):
            return True
        buf = ctypes.create_unicode_buffer(512)
        user32.GetWindowTextW(hwnd, buf, 512)
        if titulo.lower() in buf.value.lower():
            achado.append(hwnd)
            return False
        return True

    user32.EnumWindows(cb, 0)
    return achado[0] if achado else None


def main():
    for f in (ORIGEM, DESTINO):
        if os.path.isdir(f):
            for n in os.listdir(f):
                try:
                    os.remove(os.path.join(f, n))
                except OSError:
                    pass
        os.makedirs(f, exist_ok=True)

    # 1) três arquivos copiados separadamente = três entradas no histórico
    for i, nome in enumerate(NOMES):
        with io.open(os.path.join(ORIGEM, nome), "w", encoding="utf-8", newline="\n") as fp:
            fp.write(f"conteudo {i} {time.time()}\n")
        file_list([os.path.join(ORIGEM, nome)])
        time.sleep(1.8)
    diz(f"preparados: {', '.join(NOMES)}")

    # 2) app na tela, lista atualizada
    hwnd = None
    for _ in range(300):
        hwnd = h.find_window("rayclip.exe")
        if hwnd:
            break
        time.sleep(0.1)
    if not hwnd:
        diz("app nao esta rodando (janela invisivel)")
        return
    keysafe.focar(hwnd, espera=1.0) or time.sleep(0.5)
    c = CDP()
    for _ in range(40):
        if c.ev("document.querySelectorAll('.row').length"):
            break
        time.sleep(0.25)
    c.ev(
        "(() => { const i=document.querySelector('.search input'); if(!i) return 'sem';"
        " const set=Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value').set;"
        " set.call(i,''); i.dispatchEvent(new Event('input',{bubbles:true})); return 'ok'; })()"
    )
    time.sleep(0.8)

    pos = json.loads(c.ev("JSON.stringify({x:window.screenX, y:window.screenY})"))
    medidas = json.loads(
        c.ev(
            "JSON.stringify([...document.querySelectorAll('.row')].slice(0,3).map(e => {"
            " const b = e.getBoundingClientRect();"
            " return {id: e.dataset.id, txt: (e.innerText||'').replace(/\\s+/g,' ').slice(0,50),"
            " x: b.x + 110, y: b.y + b.height/2}; }))"
        )
    )
    if len(medidas) < 3:
        diz(f"lista curta demais ({len(medidas)} linhas)")
        return
    nomes_lista = [m["txt"] for m in medidas]
    diz(f"top 3 da lista: {nomes_lista}")

    # 3) Explorer na pasta de destino, à direita do app
    import subprocess

    subprocess.Popen(["explorer.exe", DESTINO])
    exp = None
    for _ in range(40):
        time.sleep(0.25)
        exp = janela_explorer(os.path.basename(DESTINO))
    if not exp:
        diz("nao achei a janela do Explorer")
        return
    import ctypes.wintypes as wt

    r = wt.RECT()
    user32.GetWindowRect(hwnd, ctypes.byref(r))
    user32.ShowWindow(exp, 9)  # SW_RESTORE
    user32.SetWindowPos(exp, 0, r.right + 40, 100, 620, 640, 0x0040)
    time.sleep(0.6)
    er = wt.RECT()
    user32.GetWindowRect(exp, ctypes.byref(er))
    # clique no miolo do painel de arquivos: garante foco no Explorer, não na barra
    entrada.clicar((er.left + er.right) // 2, er.top + 300)
    time.sleep(0.8)
    diz(f"explorer em {er.left},{er.top} visivel={bool(user32.IsWindowVisible(hwnd))} (app deve ter escondido)")

    # 4) Alt+Space traz o app de volta com o Explorer como alvo da colagem
    h.send_keys("alt", "space", forcar=True)
    time.sleep(1.0)
    if not user32.IsWindowVisible(hwnd):
        h.send_keys("alt", "space", forcar=True)
        time.sleep(1.0)
    if not keysafe.focar(hwnd, espera=1.0):
        diz("nao consegui focar o app")
        return
    time.sleep(0.4)

    # 5) Ctrl+clique nas três linhas de arquivo.
    #    O Ctrl tem que descer **depois** do primeiro clique (que é o que dá o
    #    foco à janela): tecla apertada enquanto outro app tem o foco vai para
    #    aquele app e o WebView nunca vê o `keydown` — o clique chega com
    #    `ctrlKey=false` e a seleção é substituída em vez de somada.
    medidas = json.loads(
        c.ev(
            "JSON.stringify([...document.querySelectorAll('.row')].slice(0,8)"
            ".filter(e => (e.innerText||'').includes('arq_')).map(e => {"
            " const b = e.getBoundingClientRect();"
            " return {id: e.dataset.id, txt: (e.innerText||'').replace(/\\s+/g,' ').slice(0,40),"
            " x: b.x + 110, y: b.y + b.height/2}; }))"
        )
    )
    if len(medidas) < 3:
        diz(f"so {len(medidas)} linhas de arquivo na lista")
        return
    diz(f"linhas de arquivo: {[m['txt'] for m in medidas]}")
    entrada.clicar(int(pos["x"] + medidas[0]["x"]), int(pos["y"] + medidas[0]["y"]))
    time.sleep(0.5)
    if not keysafe.focar(hwnd, espera=1.0):
        diz("app perdeu o foco antes do Ctrl")
    entrada.tecla(0x11, cima=False)  # Ctrl apertado, agora dentro do app
    time.sleep(0.12)
    # Ctrl+clique nas **três**: a linha primária (sem Ctrl) fica sendo só o
    # `selectedId`; quem a colagem múltipla leva é a `selection`, que é a lista
    # marcada. Marcar a primária também é o que o Raycast faz.
    for m in medidas[0:3]:
        entrada.clicar(int(pos["x"] + m["x"]), int(pos["y"] + m["y"]))
        time.sleep(0.4)
    entrada.tecla(0x11, cima=True)
    time.sleep(0.4)
    # `data-selected` marca só a linha primária; a seleção múltipla aparece como
    # etiqueta de ordem (`data-multi` + `.order-badge`). Contar a primária aqui
    # dava "1" e reprovava uma seleção correta.
    marcadas = int(
        c.ev("document.querySelectorAll('.row[data-multi=\"true\"]').length") or 0
    )
    diz(f"marcadas na lista: {marcadas} (esperado 3)")
    if marcadas != 3:
        diz("RESULTADO: FALHOU (selecao)")
        return

    # 6) Enter cola
    entrada.tecla(0x0D)
    time.sleep(3.0)
    vieram = sorted(os.listdir(DESTINO))
    tamanhos = {}
    for n in vieram:
        p = os.path.join(DESTINO, n)
        tamanhos[n] = os.path.getsize(p) if os.path.isfile(p) else -1
    originais = {
        n: os.path.getsize(os.path.join(ORIGEM, n)) for n in NOMES
    }
    iguais = all(tamanhos.get(n) == originais[n] for n in NOMES)
    diz(f"na pasta: {vieram}")
    diz(f"tamanhos: {tamanhos} (originais {originais}) bytes iguais={iguais}")
    ok = marcadas == 3 and sorted(vieram) == sorted(NOMES) and iguais
    diz("RESULTADO: " + ("OK" if ok else "FALHOU"))


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:  # noqa: BLE001
        linhas.append(f"ERRO: {type(exc).__name__}: {exc}")
    finally:
        io.open(SAIDA, "w", encoding="utf-8", newline="\n").write("\n".join(linhas) + "\n")
