"""GIF de link cuja página é bloqueada (klipy.com): a capa só aparece porque o
app abre uma janela escondida de verdade sob demanda.

Passos: põe `https://klipy.com/gifs/prison-29` na área de transferência, deixa o
app capturar, seleciona a entrada e espera a capa chegar. Verifica:
  - a entrada existe e é do tipo link;
  - o cartão do link ganha `<img>` **ou** `<video>` de capa (nada de retângulo);
  - a capa carrega de verdade (naturalWidth / videoWidth > 0);
  - o log registra o caminho `webmeta:`.
"""

import json
import os
import subprocess
import sys
import time
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import entrada  # noqa: E402
import harness_compare as h  # noqa: E402
import keysafe  # noqa: E402
import websocket  # noqa: E402

URL = "https://klipy.com/gifs/prison-29"
LOG = os.path.join(os.environ["TEMP"], "rayclip.log")


def diz(m):
    print(m, flush=True)


class CDP:
    def __init__(self):
        alvos = json.load(urllib.request.urlopen("http://127.0.0.1:9222/json/list", timeout=8))
        paginas = [t for t in alvos if t.get("type") == "page"]
        do_app = [t for t in paginas if "rayclip.localhost" in (t.get("url") or "")]
        pagina = (do_app or paginas)[0]
        self.ws = websocket.create_connection(
            "ws://127.0.0.1:9222/devtools/page/" + pagina["id"], suppress_origin=True, timeout=30
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
                res = r.get("result", {})
                if "exceptionDetails" in res:
                    return "ERRO_JS: " + json.dumps(res["exceptionDetails"])[:160]
                return res.get("result", {}).get("value")


def texto(t):
    """CF_UNICODETEXT cru — o mesmo caminho que o app escuta."""
    import ctypes
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
        h = k.GlobalAlloc(0x0002, len(blob))
        p = k.GlobalLock(h)
        ctypes.memmove(p, blob, len(blob))
        k.GlobalUnlock(h)
        u.SetClipboardData(13, h)
    finally:
        u.CloseClipboard()


def filtra(c, txt):
    c.ev(
        "(() => { const i=document.querySelector('.search input'); if(!i) return 'sem';"
        " const set=Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value').set;"
        " set.call(i, %s); i.dispatchEvent(new Event('input',{bubbles:true})); return 'ok'; })()"
        % json.dumps(txt)
    )
    time.sleep(1.2)


def linhas(c, n=10):
    bruto = c.ev(
        "JSON.stringify([...document.querySelectorAll('.row')].slice(0,%d).map(e=>{"
        "const b=e.getBoundingClientRect();"
        "return {id:e.dataset.id, txt:e.innerText.replace(/\\s+/g,' ').slice(0,60),"
        "x:b.x+110, y:b.y+b.height/2};}))" % n
    )
    return json.loads(bruto)


def main():
    texto(URL)
    time.sleep(1.5)

    hwnd = h.find_window("rayclip.exe")
    if not hwnd:
        diz("janela do app nao encontrada")
        return 1
    keysafe.focar(hwnd)
    time.sleep(0.3)
    pos = json.loads(
        CDP().ev("JSON.stringify({x:window.screenX,y:window.screenY})")
    )
    c = CDP()

    filtra(c, "klipy")
    achou = linhas(c, 5)
    if not achou:
        diz("entrada do klipy nao esta na lista")
        return 1
    diz("linha: " + achou[0]["txt"])

    entrada.clicar(int(pos["x"] + achou[0]["x"]), int(pos["y"] + achou[0]["y"]))
    time.sleep(1.0)

    # a janela escondida precisa carregar a página, passar no desafio e baixar
    for tentativa in range(24):
        info = c.ev(
            "(() => {"
            " const v=document.querySelector('.link-card video.link-cover');"
            " if (v) return JSON.stringify({tipo:'video', w:v.videoWidth, h:v.videoHeight, src:v.currentSrc});"
            " const i=document.querySelector('.link-card img.link-cover');"
            " if (i) return JSON.stringify({tipo:'imagem', w:i.naturalWidth, h:i.naturalHeight, src:i.currentSrc});"
            " return null; })()"
        )
        if info:
            diz(f"{tentativa * 0.5:.1f}s: capa = {info}")
            break
        time.sleep(0.5)
    else:
        diz("capa nao apareceu em 12 s")
        info = None

    ok = False
    if info:
        d = json.loads(info)
        ok = (d.get("w") or 0) > 0 and (d.get("h") or 0) > 0

    try:
        with open(LOG, "r", encoding="utf-8", errors="replace") as f:
            linhas_log = [l for l in f.read().splitlines() if "webmeta:" in l]
    except OSError:
        linhas_log = []
    diz("log webmeta: " + (" | ".join(linhas_log[-3:]) or "(nada)"))

    diz(f"capa_carregou={ok}")
    if ok:
        diz("RESULTADO: OK")
        return 0
    diz("RESULTADO: FALHOU")
    return 1


if __name__ == "__main__":
    sys.exit(main())
