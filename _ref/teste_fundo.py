"""Cor de fundo e transparência escolhidas pelo usuário.

Mede três coisas, no build de depuração:

1. `save_settings` com `bgColor`/`bgOpacity` muda de fato o `--window-bg`
   computado no `<html>`;
2. cor inválida é recusada pelo Rust (volta para "a do tema");
3. o slider e os chips existem na página de Preferências (a interface).

Roda com o app já aberto e com CDP em `RAYCLIP_CDP=1`.
"""

import json
import sys
import time
import urllib.request

import websocket


def alvo():
    for _ in range(40):
        try:
            with urllib.request.urlopen("http://127.0.0.1:9222/json/list", timeout=2) as r:
                for t in json.load(r):
                    u = t.get("url", "")
                    if t.get("type") == "page" and "/webmeta/" not in u and u.rstrip("/").endswith(
                        "localhost"
                    ):
                        return t["webSocketDebuggerUrl"]
        except OSError:
            pass
        time.sleep(0.4)
    raise SystemExit("sem alvo CDP")


class Sessao:
    def __init__(self):
        self.ws = websocket.create_connection(alvo(), timeout=30)
        self.n = 0

    def js(self, expr, await_promise=True):
        self.n += 1
        self.ws.send(
            json.dumps(
                {
                    "id": self.n,
                    "method": "Runtime.evaluate",
                    "params": {
                        "expression": expr,
                        "awaitPromise": await_promise,
                        "returnByValue": True,
                    },
                }
            )
        )
        while True:
            r = json.loads(self.ws.recv())
            if r.get("id") == self.n:
                if "exceptionDetails" in r.get("result", {}):
                    raise RuntimeError(r["result"]["exceptionDetails"])
                return r["result"]["result"].get("value")


def fundo(s):
    return s.js(
        "getComputedStyle(document.documentElement).getPropertyValue('--window-bg').trim()"
        " + ' | page: ' + getComputedStyle(document.documentElement)"
        ".getPropertyValue('--page-bg').trim()"
    )


def salvar(s, patch):
    return s.js(
        "(async () => {"
        "  const cfg = await window.__TAURI_INTERNALS__.invoke('get_settings');"
        f"  Object.assign(cfg, {json.dumps(patch)});"
        "  const w = await window.__TAURI_INTERNALS__.invoke('save_settings', { settings: cfg });"
        "  return JSON.stringify(w);"
        "})()"
    )


def main():
    s = Sessao()
    print("fundo inicial:", fundo(s))

    salvar(s, {"bgColor": "#1b1016", "bgOpacity": 0.65})
    time.sleep(0.7)
    cor = fundo(s)
    print("depois de cor+transparencia:", cor)
    ok1 = "27 16 22" in cor and "0.65" in cor

    salvar(s, {"bgColor": "red; url(x)", "bgOpacity": 0.1})
    time.sleep(0.7)
    voltou = fundo(s)
    print("com cor invalida + opacidade 0.1:", voltou)
    # a cor volta para "a do tema" (nao pode conter "red") e a opacidade para o minimo
    ok2 = "red" not in voltou and "0.35" in voltou

    salvar(s, {"bgColor": "", "bgOpacity": 1.0})
    time.sleep(0.7)
    limpo = fundo(s)
    print("de volta ao padrao:", limpo)
    ok3 = "1b1016" not in limpo and "27 16 22" not in limpo

    s.js("document.querySelectorAll('.settings')[0] || null")
    tem_ui = s.js(
        "(() => { const b = [...document.querySelectorAll('.accent-chip-theme')];"
        " const r = [...document.querySelectorAll('.slider-row input[type=range]')];"
        " return JSON.stringify({chip: b.length, slider: r.length,"
        " max: r[0] && r[0].max, label: r[0] && r[0].getAttribute('aria-label')}); })()"
    )
    print("controles na pagina:", tem_ui)

    print("RESULTADO:", "OK" if ok1 and ok2 and ok3 else "FALHOU")
    return 0 if ok1 and ok2 and ok3 else 2


if __name__ == "__main__":
    sys.exit(main())
