"""Contraste de todo texto visível em cada tema.

O usuário relatou: "quando muda o texto não fica igual". Aqui a gente mede, em
cada um dos 5 temas, a cor do texto contra o fundo realmente pintado atrás dele
(subindo a árvore até achar um fundo opaco) e lista só o que fica ilegível
(contraste < 3,0). Serve de antes/depois: prático, sem achismo de CSS.
"""

import io
import json
import os
import sys
import urllib.request

import websocket

SAIDA = os.path.join(os.environ["TEMP"], "audit_temas.txt")
linhas = []

JS = r"""
(() => {
  // Transições de cor (130 ms) fazem o `getComputedStyle` devolver a cor ANTIGA
  // logo depois da troca de tema — foi isso que gerou 17 "falhas" falsas no
  // tema claro. Para medir o que o usuário vê depois que assenta, desliga tudo.
  const stop = document.createElement("style");
  stop.textContent = "*,*::before,*::after{transition:none!important;animation:none!important}";
  document.head.appendChild(stop);
  const tema = (t) => { document.documentElement.dataset.theme = t; };
  const opaco = (c) => {
    const m = c.match(/rgba?\(([^)]+)\)/);
    if (!m) return null;
    const p = m[1].split(/[ ,/]+/).filter(Boolean).map(Number);
    const a = p.length > 3 ? p[3] : 1;
    return a >= 0.85 ? p : null;
  };
  const fundoReal = (el) => {
    let n = el;
    while (n && n !== document.documentElement) {
      const c = opaco(getComputedStyle(n).backgroundColor);
      if (c) return c;
      n = n.parentElement;
    }
    const b = opaco(getComputedStyle(document.body).backgroundColor);
    return b || [0, 0, 0];
  };
  const lum = (p) => {
    const f = p.slice(0, 3).map((v) => {
      v = v / 255;
      return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4);
    });
    return 0.2126 * f[0] + 0.7152 * f[1] + 0.0722 * f[2];
  };
  const contraste = (a, b) => {
    const l1 = lum(a), l2 = lum(b);
    return (Math.max(l1, l2) + 0.05) / (Math.min(l1, l2) + 0.05);
  };
  const res = {};
  for (const t of ["dark", "light", "midnight", "nord", "rose"]) {
    tema(t);
    const ruim = [];
    const vistos = new Set();
    for (const el of document.querySelectorAll("*")) {
      const txt = [...el.childNodes]
        .filter((n) => n.nodeType === 3)
        .map((n) => n.textContent.trim())
        .join(" ")
        .trim();
      if (!txt) continue;
      const r = el.getBoundingClientRect();
      if (r.width < 4 || r.height < 4) continue;
      const cs = getComputedStyle(el);
      if (cs.visibility === "hidden" || cs.display === "none" || +cs.opacity < 0.2) continue;
      const cor = opaco(cs.color);
      if (!cor) continue;
      const bg = fundoReal(el);
      const k = contraste(cor, bg);
      const chave = el.className + "|" + txt.slice(0, 24);
      if (vistos.has(chave)) continue;
      vistos.add(chave);
      if (k < 3.0) {
        ruim.push({
          cls: String(el.className || el.tagName).slice(0, 40),
          txt: txt.slice(0, 32),
          cor: cs.color,
          bg: "rgb(" + bg.slice(0, 3).join(",") + ")",
          k: Math.round(k * 100) / 100,
        });
      }
    }
    res[t] = ruim.slice(0, 40);
  }
  tema("dark");
  stop.remove();
  return JSON.stringify(res);
})()
"""


class CDP:
    def __init__(self):
        alvo = json.load(urllib.request.urlopen("http://127.0.0.1:9222/json/list", timeout=10))[0]
        self.ws = websocket.create_connection(
            "ws://127.0.0.1:9222/devtools/page/" + alvo["id"], suppress_origin=True, timeout=30
        )
        self.n = 0

    def ev(self, js):
        self.n += 1
        self.ws.send(json.dumps({"id": self.n, "method": "Runtime.evaluate",
                                 "params": {"expression": js, "awaitPromise": True, "returnByValue": True}}))
        while True:
            r = json.loads(self.ws.recv())
            if r.get("id") == self.n:
                res = r["result"]
                if "exceptionDetails" in res:
                    return "EXC: " + json.dumps(res["exceptionDetails"])[:300]
                return res["result"].get("value")


def main():
    c = CDP()
    v = c.ev(JS)
    if isinstance(v, str) and v.startswith("EXC"):
        linhas.append(v)
        return
    dados = json.loads(v)
    for tema, ruins in dados.items():
        linhas.append(f"\n=== {tema}: {len(ruins)} textos com contraste < 3.0 ===")
        for r in ruins:
            linhas.append(f"  {r['k']:>5}  {r['cls']:<40} {r['txt']!r} cor={r['cor']} bg={r['bg']}")


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:  # noqa: BLE001
        linhas.append(f"ERRO: {type(exc).__name__}: {exc}")
    finally:
        io.open(SAIDA, "w", encoding="utf-8", newline="\n").write("\n".join(linhas) + "\n")
        if sys.stdout:
            print("\n".join(linhas))
