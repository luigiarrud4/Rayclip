"""GIF de página do Giphy, com 2,7 MB: prova duas correções de uma vez.

1. **Regra do provedor**: a página do Giphy entrega `og:image` = logo da marca
   para qualquer cliente que não seja navegador; o Post de verdade sai da URL
   direta montada com o id do fim do slug (`media.giphy.com/media/<id>/giphy.gif`).
2. **Sem truncar**: a capa era baixada com teto de 900 KB e gravada pela metade
   (GIF parado/quebrado). Agora o arquivo inteiro é gravado — 2.792.189 B.
"""

import io
import json
import os
import sys
import time
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import entrada  # noqa: E402
import harness_compare as h  # noqa: E402
import keysafe  # noqa: E402
import websocket  # noqa: E402

PAGINA = "https://giphy.com/gifs/prison-l0MYt5jPR6QX5pnqM"
GIF = "https://media.giphy.com/media/l0MYt5jPR6QX5pnqM/giphy.gif"
TAMANHO = 2_792_189
COVERS = os.path.join(
    os.environ["APPDATA"], "com.rayclip.app", "clipboard", "media", "covers"
)


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
        hmem = k.GlobalAlloc(0x0002, len(blob))
        p = k.GlobalLock(hmem)
        ctypes.memmove(p, blob, len(blob))
        k.GlobalUnlock(hmem)
        u.SetClipboardData(13, hmem)
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


def linhas(c, n=6):
    bruto = c.ev(
        "JSON.stringify([...document.querySelectorAll('.row')].slice(0,%d).map(e=>{"
        "const b=e.getBoundingClientRect();"
        "return {id:e.dataset.id, txt:e.innerText.replace(/\\s+/g,' ').slice(0,60),"
        "x:b.x+110, y:b.y+b.height/2};}))" % n
    )
    return json.loads(bruto)


def main():
    texto(PAGINA)
    time.sleep(1.5)

    hwnd = h.find_window("rayclip.exe")
    if not hwnd:
        diz("janela do app nao encontrada")
        return 1
    keysafe.focar(hwnd)
    time.sleep(0.3)
    c = CDP()
    pos = json.loads(c.ev("JSON.stringify({x:window.screenX,y:window.screenY})"))

    filtra(c, "giphy")
    achou = linhas(c, 5)
    if not achou:
        diz("entrada do giphy nao esta na lista")
        return 1
    diz("linha: " + achou[0]["txt"])
    entrada.clicar(int(pos["x"] + achou[0]["x"]), int(pos["y"] + achou[0]["y"]))
    time.sleep(1.0)

    info = None
    for tentativa in range(40):
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

    ok_tela = False
    tam = None
    if isinstance(info, str) and info.startswith("{"):
        d = json.loads(info)
        ok_tela = (d.get("w") or 0) > 0 and (d.get("h") or 0) > 0
        src = d.get("src") or ""
        key = src.rsplit("/", 1)[-1]
        arq = os.path.join(COVERS, key)
        if os.path.isfile(arq):
            tam = os.path.getsize(arq)

    diz(f"bytes_no_disco={tam} (esperado {TAMANHO})")
    ok = ok_tela and tam == TAMANHO
    diz(f"capa_na_tela={ok_tela} tamanho_integro={tam == TAMANHO}")
    diz("RESULTADO: OK" if ok else "RESULTADO: FALHOU")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
