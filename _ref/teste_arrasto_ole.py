"""Arrasto de arquivo para fora do app pelo **sistema** (OLE / CF_HDROP).

O caminho antigo (Chromium `DownloadURL`) leva os bytes e para de funcionar entre
3 e 25 MB. Agora, quando a entrada tem arquivo, a linha chama o plugin de arrasto
(`CF_HDROP` com o caminho) e nada é copiado.

Uso:
    $env:ALVO_TXT="teste_25mb.zip"; python teste_arrasto_ole.py

Variáveis:
    ALVO_TXT     trecho para filtrar a lista (o mesmo vira título da linha)
    ALVO_NOME    nome esperado do arquivo criado (padrão: ALVO_TXT)
    ALVO_BYTES   tamanho esperado em bytes (opcional)

Por que cada detalhe:
- o app esconde ao perder o foco, e a flag `ARRASTANDO` é o que segura a janela;
  o arrasto precisa começar com a janela visível e em foco;
- o arrasto do sistema começa no **pointermove** (limiar de 4 px), então o
  primeiro movimento tem que ser curto e dentro da linha; ir direto para o
  Explorer solta o mouse fora da linha antes do gesto existir;
- o Explorer não pode ficar debaixo da janela do app (senão a soltura cai sobre o
  próprio Rayclip) nem minimizado em (-32000,-32000).
"""

import ctypes
import hashlib
import io
import json
import os
import subprocess
import cdp_env
import time
import urllib.request
from ctypes import wintypes as w

import entrada
import harness_compare as h
import keysafe
import websocket

ALVO = os.environ.get("ALVO_TXT", "")
NOME = os.environ.get("ALVO_NOME", ALVO)
ESPERADO = int(os.environ.get("ALVO_BYTES", "0") or 0)
EXE = os.path.abspath(
    os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "src-tauri", "target", "debug", "rayclip.exe")
)
PASTA = os.path.join(os.environ["TEMP"], "alvo_arrasto")
SAIDA = os.path.join(os.environ["TEMP"], "teste_arrasto_ole.txt")

user32 = ctypes.windll.user32
linhas = []


def diz(m):
    linhas.append(m)
    import sys

    if sys.stdout is not None:
        print(m)


def janela_explorer(pasta):
    achados = []
    nome_pasta = os.path.basename(pasta)
    prot = ctypes.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)

    def cb(hwnd, _):
        buf = ctypes.create_unicode_buffer(256)
        user32.GetClassNameW(hwnd, buf, 256)
        if buf.value == "CabinetWClass" and user32.IsWindowVisible(hwnd):
            titulo = ctypes.create_unicode_buffer(512)
            user32.GetWindowTextW(hwnd, titulo, 512)
            if nome_pasta.lower() in titulo.value.lower():
                achados.append(hwnd)
        return True

    user32.EnumWindows(prot(cb), 0)
    return achados[0] if achados else None


def rect(hwnd):
    r = w.RECT()
    user32.GetWindowRect(hwnd, ctypes.byref(r))
    return r


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
                res = r.get("result", {})
                if res.get("exceptionDetails"):
                    return "EXCECAO: " + json.dumps(res["exceptionDetails"])[:400]
                return res.get("result", {}).get("value")


def sha256(caminho):
    h_ = hashlib.sha256()
    with open(caminho, "rb") as f:
        for bloco in iter(lambda: f.read(1 << 20), b""):
            h_.update(bloco)
    return h_.hexdigest()


def medir_linha(c, trecho):
    """Retângulo da linha em coordenadas de tela + id."""
    js = (
        "(() => { const busca = " + json.dumps(trecho) + ";"
        " const linhas = [...document.querySelectorAll('.row')];"
        " const e = busca ? linhas.find(x => (x.textContent||'').includes(busca)) : linhas[0];"
        " if (!e) return JSON.stringify({erro: 'sem linha', n: linhas.length});"
        " const b = e.getBoundingClientRect();"
        " return JSON.stringify({x: b.x + 110, y: b.y + b.height / 2, id: e.dataset.id,"
        "  sx: window.screenX, sy: window.screenY}); })()"
    )
    bruto = c.ev(js) or "null"
    return json.loads(bruto)


def main():
    os.makedirs(PASTA, exist_ok=True)
    for f in os.listdir(PASTA):
        os.remove(os.path.join(PASTA, f))

    # Explorer à direita (o app ocupa ~0..900) e com a pasta vazia aberta.
    subprocess.Popen(["explorer.exe", PASTA])
    exp = None
    for _ in range(40):
        time.sleep(0.5)
        exp = janela_explorer(PASTA)
        if exp:
            break
    if not exp:
        diz("sem janela do Explorer")
        return
    user32.ShowWindow(exp, 9)  # SW_RESTORE
    user32.SetWindowPos(exp, 0, 1290, 100, 620, 640, 0x0040)  # SWP_SHOWWINDOW
    time.sleep(1.2)
    er = rect(exp)
    diz(f"explorer 0x{exp:x} ({er.left},{er.top})-({er.right},{er.bottom})")

    # App reiniciado do zero (WebView2 órfão mata a instância nova).
    subprocess.run(["taskkill", "/IM", "rayclip.exe", "/F"], capture_output=True)
    time.sleep(0.8)
    subprocess.run(
        [
            "powershell",
            "-NoProfile",
            "-Command",
            "Get-CimInstance Win32_Process -Filter \"Name='msedgewebview2.exe'\" |"
            " Where-Object { $_.CommandLine -like '*com.rayclip.app*' -or"
            " $_.CommandLine -like '*webview-exe-name=rayclip.exe*' } |"
            " ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }",
        ],
        capture_output=True,
    )
    time.sleep(0.6)
    subprocess.Popen(
        [EXE, "--open"],
        creationflags=0x08000008,
        env=cdp_env.env_cdp(),
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    hwnd = None
    for _ in range(200):
        if h.find_window("rayclip.exe"):
            hwnd = h.find_window("rayclip.exe")
            break
        time.sleep(0.05)
    if not hwnd:
        diz("janela nunca ficou visivel")
        return
    time.sleep(0.6)
    c = CDP()

    # A lista só existe depois do primeiro `search_entries`; medir antes disso dá
    # "0 linhas" e o teste parece quebrado quando é só pressa.
    n = 0
    for _ in range(40):
        n = c.ev("document.querySelectorAll('.row').length") or 0
        if n:
            break
        time.sleep(0.25)
    diz(f"linhas na tela antes do filtro: {n}")

    # Filtra para a linha ficar parada (a lista recebe entradas o tempo todo).
    if ALVO:
        c.ev(
            "(() => { const i = document.querySelector('.search input'); if (!i) return 'sem campo';"
            " const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set;"
            f" set.call(i, {json.dumps(ALVO)});"
            " i.dispatchEvent(new Event('input', { bubbles: true })); return 'ok'; })()"
        )
        time.sleep(0.9)

    if not keysafe.focar(hwnd, espera=1.0):
        diz("nao consegui focar a janela")
        return

    linha = medir_linha(c, ALVO)
    if "erro" in linha:
        diz(f"{linha['erro']} ({linha.get('n')} linhas) para {ALVO!r}")
        return
    x, y = int(linha["sx"] + linha["x"]), int(linha["sy"] + linha["y"])
    diz(f"linha {linha['id'][:8]} em ({x},{y})")

    # Pairar deixa o caminho pronto antes do gesto (o arrasto do sistema busca o
    # caminho no hover). `SEM_HOVER=1` simula o gesto apressado: pegar e arrastar
    # na hora, sem tempo de prefetch — é o caso que precisa funcionar também.
    sem_hover = os.environ.get("SEM_HOVER") == "1"
    entrada.mover(x, y)
    time.sleep(0.05 if sem_hover else 0.7)
    entrada.baixo()
    time.sleep(0.05)
    # Movimento curto e ainda **dentro** da linha: é isso que dispara o arrasto.
    entrada.mover(x + 14, y)
    time.sleep(0.45)
    diz("arrasto iniciado; indo para o Explorer")

    destino = ((er.left + er.right) // 2, er.top + 300)
    for i in range(1, 21):
        entrada.mover(x + 14 + (destino[0] - x - 14) * i // 20, y + (destino[1] - y) * i // 20)
        time.sleep(0.04)
    time.sleep(0.3)
    visivel = bool(user32.IsWindowVisible(hwnd))
    diz(f"durante: arquivos={os.listdir(PASTA)} janela_visivel={visivel}")
    entrada.cima()

    depois = []
    for _ in range(24):
        time.sleep(0.5)
        depois = os.listdir(PASTA)
        if depois:
            break
    diz(f"depois de soltar: {depois}")
    ok = False
    for nome in depois:
        caminho = os.path.join(PASTA, nome)
        tam = os.path.getsize(caminho)
        diz(f"  {nome} -> {tam} bytes sha256={sha256(caminho)[:16]}")
        nome_ok = (NOME == "") or (nome.lower() == NOME.lower())
        tam_ok = (ESPERADO == 0) or (tam == ESPERADO)
        if nome_ok and tam_ok:
            ok = True
    diz(f"esperado: nome={NOME!r} bytes={ESPERADO}")
    diz("RESULTADO: " + ("OK" if ok else "FALHOU"))


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:  # noqa: BLE001
        linhas.append(f"ERRO: {type(exc).__name__}: {exc}")
    finally:
        io.open(SAIDA, "w", encoding="utf-8", newline="\n").write("\n".join(linhas) + "\n")
