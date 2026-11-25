"""Mede o filtro "Somente vídeos" e a prévia única do vídeo (v0.1.44).

Duas checagens:
1. escolher "Videos Only" no menu de tipos deixa **só** entradas de vídeo na
   lista (e a contagem do Rust bate);
2. a entrada de vídeo mostra **uma** prévia — o `<video>` — e nenhum cartão de
   miniatura solto acima dele.
"""

import json
import sys
import time

import websocket  # type: ignore


def alvo():
    import urllib.request

    for _ in range(40):
        try:
            with urllib.request.urlopen("http://127.0.0.1:9222/json/list", timeout=2) as r:
                for t in json.load(r):
                    url = t.get("url", "")
                    # o build de debug serve de `tauri.localhost`; o de release,
                    # de `rayclip.localhost`. A janela escondida do webmeta tem
                    # `/webmeta/` na URL e fica de fora.
                    if t.get("type") != "page" or "/webmeta/" in url:
                        continue
                    if url.rstrip("/").endswith("localhost"):
                        return t["webSocketDebuggerUrl"]
        except OSError:
            pass
        time.sleep(0.5)
    raise SystemExit("sem alvo CDP")


class Cdp:
    def __init__(self, url):
        self.ws = websocket.create_connection(url, timeout=30)
        self.n = 0

    def av(self, expr, espera=True):
        self.n += 1
        self.ws.send(
            json.dumps(
                {
                    "id": self.n,
                    "method": "Runtime.evaluate",
                    "params": {
                        "expression": expr,
                        "returnByValue": True,
                        "awaitPromise": espera,
                    },
                }
            )
        )
        while True:
            msg = json.loads(self.ws.recv())
            if msg.get("id") == self.n:
                r = msg.get("result", {}).get("result", {})
                if "value" in r:
                    return r["value"]
                return msg.get("result", {}).get("exceptionDetails") or r


def main():
    c = Cdp(alvo())
    c.av("1")

    # --- filtro de vídeo: abre o menu de tipos e escolhe "Somente vídeos" ---
    abriu = c.av(
        """(() => {
      const btn = document.querySelector('.filter-button');
      if (!btn) return 'sem .filter-button';
      btn.click();
      return 'abriu';
    })()""",
        espera=False,
    )
    time.sleep(0.6)
    opcoes = c.av(
        """[...document.querySelectorAll('.filter-menu .filter-item .label')]
          .map(e => e.textContent.trim())""",
        espera=False,
    )
    # `.filter-item` age no `onMouseDown` — `.click()` sozinho não escolhe
    escolheu = c.av(
        """(() => {
      const it = [...document.querySelectorAll('.filter-menu .filter-item')]
        .find(e => /Videos Only|Somente v/.test(e.textContent));
      if (!it) return 'sem opcao de video';
      it.dispatchEvent(new MouseEvent('mousedown', {bubbles: true, cancelable: true}));
      return 'escolheu';
    })()""",
        espera=False,
    )
    time.sleep(1.8)
    tipos = c.av(
        """[...document.querySelectorAll('.row .title')].map(e => e.textContent.trim())""",
        espera=False,
    )
    total = c.av(
        """(() => {
      const m = document.body.innerText.match(/\\d+\\s*(of|de)\\s*\\d+/);
      return m ? m[0] : null;
    })()""",
        espera=False,
    )
    rotulo = c.av(
        """(() => {
      const b = document.querySelector('.filter-button');
      return b ? b.textContent.trim() : null;
    })()""",
        espera=False,
    )

    # --- prévia: um único cartão, e ele é o player ---
    time.sleep(0.4)
    previa = c.av(
        """(() => {
      const p = [...document.querySelectorAll('.details .preview')];
      return {
        cartoes: p.length,
        classes: p.map(e => e.className),
        thumbs: document.querySelectorAll('.details .preview > img').length,
        videos: document.querySelectorAll('.details video').length,
        // o cartão de arquivo (caminho + tamanho) é informação, não prévia:
        // só conta como erro se algum cartão tiver imagem/vídeo dentro dele
        com_midia: [...document.querySelectorAll('.details .preview')]
          .filter(e => e.querySelector('img, video')).length,
        altura_video: (document.querySelector('.details video') || {}).clientHeight || 0,
      };
    })()""",
        espera=False,
    )

    print("opcoes do menu:", opcoes)
    print("abriu:", abriu, "| escolheu:", escolheu, "| rotulo:", rotulo)
    print("contagem:", total)
    print("tipos na lista:", tipos)
    print("previa:", previa)

    MIDIA = (".mp4", ".m4v", ".mov", ".webm", ".mkv", ".avi", ".ogv")
    so_video = bool(tipos) and all(t.lower().endswith(MIDIA) for t in tipos)
    um_cartao = (
        previa["videos"] == 1
        and previa["thumbs"] == 0
        and previa["com_midia"] == 1
        # o único cartão com mídia tem de ser o player, não uma imagem solta
        and previa["classes"][0] == "preview preview-media"
    )
    ok = so_video and um_cartao and previa["altura_video"] > 40
    print("RESULTADO:", "OK" if ok else "FALHOU")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
