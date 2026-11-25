"""Vídeo no painel, GIF de link direto e a busca travada com a prévia aberta.

1. vídeo: `<video>` no painel de detalhes, mudo por padrão, tocando de verdade
   (`currentTime` avança) — prova que a rota `rayclip://video/<id>` responde
   Range como o Chromium espera;
2. link que **é** um GIF (tipo Discord): capa baixada e servida crua (bytes
   começam com GIF8 e são grandes), então anima;
3. com a prévia (Quick Look) aberta, letras e Espaço não entram na busca.

Uso (app rodando via `_ref/reiniciar.ps1 -Cdp`):
    cd /c/Users/Lugia/Projects/rayclip && python _ref/teste_midia.py
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

SAIDA = os.path.join(os.environ["TEMP"], "teste_midia.txt")
user32 = ctypes.windll.user32
kernel32 = ctypes.windll.kernel32
linhas = []

VIDEO = r"C:\Users\Lugia\Desktop\abccccc\Downloads\vg6E3abgM3zD0yHJ.mp4"
GIF_URL = "https://media.giphy.com/media/3o7abKhOpu0NwenH3O/giphy.gif"
ORIGEM = os.path.join(os.environ["TEMP"], "alvo_midia")


def diz(m):
    linhas.append(str(m))
    print(m, flush=True)


def file_list(paths):
    user32.OpenClipboard.argtypes = [ctypes.c_void_p]
    user32.OpenClipboard.restype = ctypes.c_int
    user32.EmptyClipboard.argtypes = []
    user32.CloseClipboard.argtypes = []
    user32.SetClipboardData.argtypes = [ctypes.c_uint, ctypes.c_void_p]
    user32.SetClipboardData.restype = ctypes.c_void_p
    kernel32.GlobalAlloc.argtypes = [ctypes.c_uint, ctypes.c_size_t]
    kernel32.GlobalAlloc.restype = ctypes.c_void_p
    kernel32.GlobalLock.argtypes = [ctypes.c_void_p]
    kernel32.GlobalLock.restype = ctypes.c_void_p
    kernel32.GlobalUnlock.argtypes = [ctypes.c_void_p]

    cab = struct.pack("<iiiii", 20, 0, 0, 0, 1)
    corpo = ("\0".join(paths) + "\0\0").encode("utf-16-le")
    blob = cab + corpo
    user32.OpenClipboard(None)
    try:
        user32.EmptyClipboard()
        hmem = kernel32.GlobalAlloc(0x0002, len(blob))
        ptr = kernel32.GlobalLock(hmem)
        ctypes.memmove(ptr, blob, len(blob))
        kernel32.GlobalUnlock(hmem)
        user32.SetClipboardData(15, hmem)
    finally:
        user32.CloseClipboard()


def texto(t):
    user32.OpenClipboard.argtypes = [ctypes.c_void_p]
    user32.EmptyClipboard.argtypes = []
    user32.CloseClipboard.argtypes = []
    user32.SetClipboardData.argtypes = [ctypes.c_uint, ctypes.c_void_p]
    kernel32.GlobalAlloc.argtypes = [ctypes.c_uint, ctypes.c_size_t]
    kernel32.GlobalAlloc.restype = ctypes.c_void_p
    kernel32.GlobalLock.argtypes = [ctypes.c_void_p]
    kernel32.GlobalLock.restype = ctypes.c_void_p
    dados = (t + "\0").encode("utf-16-le")
    user32.OpenClipboard(None)
    try:
        user32.EmptyClipboard()
        hmem = kernel32.GlobalAlloc(0x0002, len(dados))
        ptr = kernel32.GlobalLock(hmem)
        ctypes.memmove(ptr, dados, len(dados))
        kernel32.GlobalUnlock(hmem)
        user32.SetClipboardData(13, hmem)  # CF_UNICODETEXT
    finally:
        user32.CloseClipboard()


class CDP:
    def __init__(self):
        alvo = json.load(urllib.request.urlopen("http://127.0.0.1:9222/json/list", timeout=10))[0]
        self.ws = websocket.create_connection(
            "ws://127.0.0.1:9222/devtools/page/" + alvo["id"], suppress_origin=True, timeout=60
        )
        self.n = 0

    def ev(self, js):
        self.n += 1
        self.ws.send(
            json.dumps(
                {
                    "id": self.n,
                    "method": "Runtime.evaluate",
                    "params": {
                        "expression": js,
                        "awaitPromise": True,
                        "returnByValue": True,
                        "userGesture": True,
                    },
                }
            )
        )
        while True:
            r = json.loads(self.ws.recv())
            if r.get("id") == self.n:
                res = r.get("result", {})
                if res.get("exceptionDetails"):
                    return "ERRO_JS: " + json.dumps(res["exceptionDetails"])[:200]
                return res.get("result", {}).get("value")


def prepara():
    os.makedirs(ORIGEM, exist_ok=True)
    png = os.path.join(ORIGEM, "alvo_espaco.png")
    # PNG 64x64 liso, para ter uma entrada de imagem na lista.
    import zlib

    def bloco(tipo, dados):
        return (
            struct.pack(">I", len(dados))
            + tipo
            + dados
            + struct.pack(">I", zlib.crc32(tipo + dados) & 0xFFFFFFFF)
        )

    largura = altura = 64
    linhas_png = b"".join(b"\x00" + bytes([40, 60, 90] * largura) for _ in range(altura))
    png_bytes = (
        b"\x89PNG\r\n\x1a\n"
        + bloco(b"IHDR", struct.pack(">IIBBBBB", largura, altura, 8, 2, 0, 0, 0))
        + bloco(b"IDAT", zlib.compress(linhas_png))
        + bloco(b"IEND", b"")
    )
    io.open(png, "wb").write(png_bytes)

    file_list([png])
    time.sleep(2.0)
    file_list([VIDEO])
    time.sleep(2.0)
    texto(GIF_URL)
    time.sleep(2.0)
    diz("na area de transferencia: imagem, video, link de GIF")


def main():
    prepara()
    hwnd = None
    for _ in range(300):
        hwnd = h.find_window("rayclip.exe")
        if hwnd:
            break
        time.sleep(0.1)
    if not hwnd:
        diz("app nao esta rodando")
        return
    if not keysafe.focar(hwnd, espera=1.0):
        diz("sem foco")
        return
    c = CDP()
    for _ in range(40):
        if c.ev("document.querySelectorAll('.row').length"):
            break
        time.sleep(0.25)
    pos = json.loads(c.ev("JSON.stringify({x:window.screenX, y:window.screenY})"))

    def linhas_da_lista(n=8):
        return json.loads(
            c.ev(
                "JSON.stringify([...document.querySelectorAll('.row')].slice(0,%d).map(e => {"
                " const b=e.getBoundingClientRect();"
                " return {id:e.dataset.id, txt:(e.innerText||'').replace(/\\s+/g,' ').slice(0,60),"
                " x:b.x+110, y:b.y+b.height/2}; }))" % n
            )
        )

    def clica(m):
        entrada.clicar(int(pos["x"] + m["x"]), int(pos["y"] + m["y"]))
        time.sleep(0.6)

    def filtra(texto):
        c.ev(
            "(() => { const i=document.querySelector('.search input'); if(!i) return 'sem';"
            " const set=Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value').set;"
            " set.call(i, %s); i.dispatchEvent(new Event('input',{bubbles:true})); return 'ok'; })()"
            % json.dumps(texto)
        )
        time.sleep(1.0)

    def seleciona(trecho):
        """Remede as linhas e clica na que contém `trecho` (a lista muda de
        posição quando entram capturas novas)."""
        for _ in range(5):
            atual = linhas_da_lista(12)
            alvo = next((l for l in atual if trecho.lower() in l["txt"].lower()), None)
            if not alvo:
                time.sleep(1.0)
                continue
            clica(alvo)
            sel = c.ev(
                "(document.querySelector('.row[data-selected=\"true\"]')||{dataset:{}}).dataset.id || null"
            )
            if str(sel)[:8] == str(alvo["id"])[:8]:
                return alvo
            time.sleep(0.5)
        return None

    # ---------------------------------------------------------------- vídeo
    filtra("")
    linhas_lista = linhas_da_lista(10)
    diz("lista: " + str([l["txt"][:38] for l in linhas_lista]))
    filtra("vg6E3abgM3zD0yHJ")
    if not seleciona("vg6E3abgM3zD0yHJ"):
        diz("nao consegui selecionar o video")
        return
    tem_player = c.ev("!!document.querySelector('.details video.media-player')")
    info = c.ev(
        "(() => { const v=document.querySelector('.details video.media-player');"
        " if(!v) return null;"
        " return JSON.stringify({muted:v.muted, poster:!!v.poster, src:v.currentSrc.slice(0,60),"
        "  dur:v.duration, w:v.videoWidth}); })()"
    )
    diz(f"player presente={tem_player} info={info}")
    tocou = c.ev(
        "(async () => { const v=document.querySelector('.details video.media-player');"
        " if(!v) return 'sem player';"
        " try { await v.play(); } catch (e) { return 'play falhou: ' + e.name; }"
        " await new Promise(r => setTimeout(r, 1200));"
        " const t = v.currentTime; v.pause();"
        " return JSON.stringify({currentTime:t, dur:v.duration, w:v.videoWidth, muted:v.muted}); })()"
    )
    diz(f"tocou: {tocou}")

    # ------------------------------------------------------- GIF de link direto
    capa = None
    if True:
        filtra("giphy")
        if not seleciona("giphy"):
            diz("nao consegui selecionar o link do gif")
            return
        for _ in range(12):
            capa = c.ev(
                "(() => { const i=document.querySelector('.link-card img');"
                " return i ? i.src : null; })()"
            )
            if capa:
                break
            time.sleep(1.0)
        diz(f"capa do link: {capa}")
    prefixo = None
    if capa:
        # O `fetch` do JS não alcança o protocolo próprio (CSP: `connect-src` é só
        # ipc) — o arquivo é lido do disco, que é onde o app gravou os bytes.
        key = capa.rsplit("/", 1)[-1]
        arq = os.path.join(
            os.environ["APPDATA"], "com.rayclip.app", "clipboard", "media", "covers", key
        )
        if os.path.isfile(arq):
            with open(arq, "rb") as f:
                b = f.read()
            prefixo = json.dumps({"bytes": len(b), "magic": b[:4].decode("latin-1")})
            diz(f"capa em disco: {prefixo}")
        else:
            diz(f"capa nao encontrada em disco: {arq}")
        # A imagem carregou na tela?
        carregou = c.ev(
            "(() => { const i=document.querySelector('.link-card img');"
            " return i ? JSON.stringify({w:i.naturalWidth, h:i.naturalHeight}) : 'sem img'; })()"
        )
        diz(f"imagem da capa na tela: {carregou}")

    # ---------------------------------- prévia aberta não deixa digitar na busca
    # Imagem que mora no banco (`hasImage`), não arquivo do disco: o Quick Look
    # abre para imagem capturada, e é esse o caso do usuário.
    filtra("Imagem (")
    if not seleciona("Imagem ("):
        diz("nao consegui selecionar uma imagem do historico")
        return
    # Quick Look exige busca vazia: limpa o filtro sem perder a seleção.
    filtra("")
    entrada.tecla(0x20)  # Espaço abre a prévia
    time.sleep(1.2)
    aberta = c.ev("!!document.querySelector('.viewer-card')")
    diz(f"previa aberta com Espaco: {aberta}")
    for ch in ("a", "b", "c"):
        entrada.tecla(ord(ch.upper()))
        time.sleep(0.12)
    time.sleep(0.4)
    busca = c.ev("(document.querySelector('.search input')||{}).value ?? 'sem input'")
    travado = c.ev("(document.querySelector('.search input')||{}).readOnly ?? null")
    diz(f"busca depois de digitar abc com a previa aberta: {busca!r} (readOnly={travado})")
    entrada.tecla(0x20)  # Espaço fecha a prévia
    time.sleep(1.0)
    fechou = not c.ev("!!document.querySelector('.viewer-card')")
    busca2 = c.ev("(document.querySelector('.search input')||{}).value ?? 'sem input'")
    diz(f"previa fechou com Espaco: {fechou}; busca continua {busca2!r}")

    ok_video = ("currentTime" in str(tocou)) and ('"currentTime":0' not in str(tocou).replace(' ', ""))
    ok_gif = bool(prefixo and "GIF8" in str(prefixo))
    ok_busca = busca == "" and travado is True and fechou and busca2 == ""
    diz(f"video={ok_video} gif_de_link={ok_gif} busca_travada={ok_busca}")
    diz("RESULTADO: " + ("OK" if (ok_video and ok_gif and ok_busca) else "FALHOU"))


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:  # noqa: BLE001
        linhas.append(f"ERRO: {type(exc).__name__}: {exc}")
    finally:
        io.open(SAIDA, "w", encoding="utf-8", newline="\n").write("\n".join(linhas) + "\n")
