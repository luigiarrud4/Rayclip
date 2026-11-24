"""Contraste em cada superfície: lista, painel de ações, filtro e Preferências.

O `audit_temas.py` mede só o que está na tela (a lista). Aqui a gente abre cada
superfície (Ctrl+K, menu de tipos, Preferências) e repete a medição nos 5 temas,
para não sobrar canto escuro no tema claro.
"""

import io
import json
import os
import sys
import time
import urllib.request

import websocket

SAIDA = os.path.join(os.environ["TEMP"], "audit_superficies.txt")
linhas = []
n = 0

MEDIR = r"""
(() => {
  const stop = document.createElement("style");
  stop.textContent = "*,*::before,*::after{transition:none!important;animation:none!important}";
  document.head.appendChild(stop);
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
    document.documentElement.dataset.theme = t;
    const ruim = [];
    const vistos = new Set();
    for (const el of document.querySelectorAll("*")) {
      const txt = [...el.childNodes].filter((x) => x.nodeType === 3)
        .map((x) => x.textContent.trim()).join(" ").trim();
      if (!txt) continue;
      const r = el.getBoundingClientRect();
      if (r.width < 4 || r.height < 4) continue;
      const cs = getComputedStyle(el);
      if (cs.visibility === "hidden" || cs.display === "none" || +cs.opacity < 0.2) continue;
      const cor = opaco(cs.color);
      if (!cor) continue;
      const bg = fundoReal(el);
      const k = contraste(cor, bg);
      const chave = String(el.className) + "|" + txt.slice(0, 20);
      if (vistos.has(chave)) continue;
      vistos.add(chave);
      if (k < 3.0) {
        ruim.push({ cls: String(el.className || el.tagName).slice(0, 34),
                    txt: txt.slice(0, 28), cor: cs.color,
                    bg: "rgb(" + bg.slice(0, 3).join(",") + ")",
                    k: Math.round(k * 100) / 100 });
      }
    }
    res[t] = ruim;
  }
  document.documentElement.dataset.theme = "dark";
  stop.remove();
  return JSON.stringify(res);
})()
"""


class CDP:
    def __init__(self):
        alvo = json.load(urllib.request.urlopen("http://127.0.0.1:9222/json/list", timeout=10))[0]
        self.ws = websocket.create_connection(
            "ws://127.0.0.1:9222/devtools/page/" + alvo["id"], suppress_origin=True, timeout=40
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
                    return "EXC"
                return res["result"].get("value")


def tecla(c, key):
    c.ev("document.dispatchEvent(new KeyboardEvent('keydown',{key:%r,bubbles:true,cancelable:true}))" % key)
    time.sleep(0.3)


def medir(c, onde):
    v = c.ev(MEDIR)
    if not isinstance(v, str) or v.startswith("EXC"):
        linhas.append(f"{onde}: falhou na medição")
        return
    dados = json.loads(v)
    total = sum(len(x) for x in dados.values())
    piores = []
    for tema, itens in dados.items():
        for it in itens:
            piores.append(f"      {tema:9s} {it['k']:>5}  {it['cls']:<34} {it['txt']!r} cor={it['cor']} bg={it['bg']}")
    linhas.append(f"\n## {onde}: {total} texto(s) com contraste < 3.0")
    linhas.extend(piores[:24])


def main():
    time.sleep(2)
    c = CDP()
    medir(c, "lista")
    # painel de ações (Ctrl+K)
    c.ev("""(() => { document.dispatchEvent(new KeyboardEvent('keydown',
        {key:'k', code:'KeyK', ctrlKey:true, bubbles:true, cancelable:true})); })()""")
    time.sleep(0.45)
    medir(c, "painel de ações (Ctrl+K)")
    tecla(c, "Escape")
    # menu de tipos (chip "All types")
    c.ev("""(() => { const b = [...document.querySelectorAll('button')].find((x) => /types|tipos/i.test(x.textContent));
        if (b) b.click(); })()""")
    time.sleep(0.45)
    medir(c, "menu de tipos")
    tecla(c, "Escape")
    # Preferências
    c.ev("""(() => { document.dispatchEvent(new KeyboardEvent('keydown',
        {key:'Comma', code:'Comma', ctrlKey:true, bubbles:true, cancelable:true})); })()""")
    time.sleep(0.8)
    medir(c, "Preferências")
    for secao in range(3):
        c.ev("""(() => { const b = [...document.querySelectorAll('.settings-nav button, .settings-nav a')][%d];
            if (b) b.click(); })()""" % secao)
        time.sleep(0.4)
        medir(c, f"Preferências · aba {secao + 1}")


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:  # noqa: BLE001
        linhas.append(f"ERRO: {type(exc).__name__}: {exc}")
    finally:
        io.open(SAIDA, "w", encoding="utf-8", newline="\n").write("\n".join(linhas) + "\n")
        if sys.stdout:
            print("\n".join(linhas))
