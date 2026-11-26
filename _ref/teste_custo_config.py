"""Quanto custa cada `save_settings` — e por que o slider travava.

Cada gravação de preferências passa por `apply_settings`, que faz coisas caras
(re-registrar atalhos globais, `set_startup` com schtasks, `set_replace_win_v`
com registro, backdrop do DWM). Num slider isso acontece a cada passo do mouse.

Este teste mede o tempo de ida e volta de `save_settings` para mudanças
"baratas" (opacidade) e caras (atalhos), em sequência rápida, e reporta o pior
caso. Também mede `get_settings`, que é chamado pela interface a cada
`settings:changed`.
"""

import json
import sys
import time

import websocket


def alvo():
    for _ in range(40):
        try:
            with __import__("urllib.request").request.urlopen(
                "http://127.0.0.1:9222/json/list", timeout=2
            ) as r:
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
        self.ws = websocket.create_connection(alvo(), timeout=60)
        self.n = 0

    def js(self, expr, timeout=60):
        self.n += 1
        self.ws.send(
            json.dumps(
                {
                    "id": self.n,
                    "method": "Runtime.evaluate",
                    "params": {
                        "expression": expr,
                        "awaitPromise": True,
                        "returnByValue": True,
                    },
                }
            )
        )
        self.ws.settimeout(timeout)
        while True:
            r = json.loads(self.ws.recv())
            if r.get("id") == self.n:
                if "exceptionDetails" in r.get("result", {}):
                    raise RuntimeError(r["result"]["exceptionDetails"])
                return r["result"]["result"].get("value")


CICLO = """
(async () => {
  const t = [];
  const cfg0 = await window.__TAURI_INTERNALS__.invoke('get_settings');
  for (const patch of PATCHES) {
    const cfg = { ...cfg0, ...patch };
    const a = performance.now();
    await window.__TAURI_INTERNALS__.invoke('save_settings', { settings: cfg });
    t.push(+(performance.now() - a).toFixed(1));
  }
  const g = performance.now();
  await window.__TAURI_INTERNALS__.invoke('get_settings');
  const leitura = +(performance.now() - g).toFixed(1);
  await window.__TAURI_INTERNALS__.invoke('save_settings', { settings: cfg0 });
  return JSON.stringify({ tempos: t, leitura });
})()
"""


def rodar(s, nome, patches):
    expr = CICLO.replace("PATCHES", json.dumps(patches))
    t0 = time.time()
    saida = json.loads(s.js(expr))
    tempos = saida["tempos"]
    print(
        f"{nome:24s} n={len(tempos):2d} min={min(tempos):7.1f} max={max(tempos):7.1f} "
        f"total={sum(tempos):8.1f} ms | get_settings={saida['leitura']} ms | "
        f"parede={(time.time()-t0)*1000:.0f} ms"
    )
    return tempos


def main():
    s = Sessao()
    rodar(s, "opacidade (20x)", [{"bgOpacity": 0.65 + (i % 5) * 0.01} for i in range(20)])
    rodar(s, "cor de fundo (12x)", [{"bgColor": "#1b1016" if i % 2 else "#0a0a0c"} for i in range(12)])
    rodar(s, "idioma (6x)", [{"language": "pt-BR" if i % 2 else "en"} for i in range(6)])
    rodar(s, "atalho global (4x)", [{"pasteHotkey": f"Ctrl+Shift+F{i}"} for i in range(4)])
    print("(leitura de cada linha: custo da chamada que a interface repete a cada settings:changed)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
