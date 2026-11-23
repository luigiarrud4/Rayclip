"""Wheel/click de verdade no WebView2 (Input domain), para provar que o scroll
na prévia não rola a coluna quando o zoom não está armado.

Uso: python _ref/cdp_wheel.py
"""
import json
import urllib.request

from websockets.sync.client import connect


def page_ws():
    targets = json.load(urllib.request.urlopen("http://127.0.0.1:9222/json"))
    return next(t for t in targets if t["type"] == "page")["webSocketDebuggerUrl"]


class Cdp:
    def __init__(self):
        self.ws = connect(page_ws(), max_size=16 * 1024 * 1024)
        self.n = 0

    def send(self, method, params=None):
        self.n += 1
        mid = self.n
        self.ws.send(json.dumps({"id": mid, "method": method, "params": params or {}}))
        while True:
            msg = json.loads(self.ws.recv())
            if msg.get("id") == mid:
                if "error" in msg:
                    raise RuntimeError(msg["error"])
                return msg.get("result", {})

    def js(self, expr):
        r = self.send(
            "Runtime.evaluate",
            {"expression": expr, "returnByValue": True, "awaitPromise": True},
        )
        if r.get("exceptionDetails"):
            raise RuntimeError(r["exceptionDetails"])
        return r["result"].get("value")

    def wheel(self, x, y, dy):
        self.send(
            "Input.dispatchMouseEvent",
            {"type": "mouseWheel", "x": x, "y": y, "deltaX": 0, "deltaY": dy},
        )

    def click(self, x, y):
        for t in ("mousePressed", "mouseReleased"):
            self.send(
                "Input.dispatchMouseEvent",
                {"type": t, "x": x, "y": y, "button": "left", "clickCount": 1},
            )

    def close(self):
        self.ws.close()


def main():
    c = Cdp()
    try:
        # 1. seleciona uma entrada de imagem e deixa a coluna rolavel de proposito
        info = c.js(
            """(async()=>{
          const inv=window.__TAURI_INTERNALS__.invoke;
          const l=await inv('list_entries');
          const img=l.find(e=>e.hasImage);
          if(!img) return {erro:'sem imagem'};
          const row=document.querySelector('.row[data-id="'+img.id+'"]');
          if(!row) return {erro:'linha nao visivel'};
          row.dispatchEvent(new MouseEvent('click',{bubbles:true}));
          await new Promise(r=>setTimeout(r,500));
          const d=document.querySelector('.details');
          if(!d.querySelector('#spacer-prova')) {
            const s=document.createElement('div');
            s.id='spacer-prova'; s.style.height='1600px';
            d.appendChild(s);
          }
          d.scrollTop=0;
          const z=document.querySelector('.preview-zoomable .zoomwrap');
          const r=z.getBoundingClientRect();
          return {id:img.id, x:Math.round(r.left+r.width/2), y:Math.round(r.top+r.height/2),
                  scrollTop:d.scrollTop, scrollavel:d.scrollHeight>d.clientHeight};
        })()"""
        )
        print("1) entrada:", info)
        x, y = info["x"], info["y"]

        # 2. NAO armado: wheel de verdade deve rolar a coluna e nao dar zoom
        for _ in range(3):
            c.wheel(x, y, 120)
        c.js("new Promise(r=>setTimeout(r,250))")
        r1 = c.js(
            """(()=>{const d=document.querySelector('.details');
                   const img=document.querySelector('.preview-zoomable .zoomwrap img');
                   return {scrollTop:d.scrollTop, transform:img.style.transform,
                           armed:document.querySelector('.preview-zoomable .zoomwrap').dataset.armed};})()"""
        )
        print("2) sem clicar na imagem:", r1)

        # 3. clica na imagem (arma o zoom) e gira o wheel para BAIXO
        #    volta a coluna ao topo: as coordenadas do primeiro rect valem só lá
        pos = c.js(
            """(()=>{const d=document.querySelector('.details');d.scrollTop=0;
                   const z=document.querySelector('.preview-zoomable .zoomwrap');
                   const r=z.getBoundingClientRect();
                   return {x:Math.round(r.left+r.width/2), y:Math.round(r.top+r.height/2)};})()"""
        )
        c.js("new Promise(r=>setTimeout(r,200))")
        c.click(pos["x"], pos["y"])
        c.js("new Promise(r=>setTimeout(r,250))")
        r2 = c.js(
            """(()=>{const d=document.querySelector('.details');
                   return {scrollTop:d.scrollTop,
                           armed:document.querySelector('.preview-zoomable .zoomwrap').dataset.armed};})()"""
        )
        print("3) depois do clique:", r2)

        for _ in range(3):
            c.wheel(pos["x"], pos["y"], 120)
        c.js("new Promise(r=>setTimeout(r,250))")
        r3 = c.js(
            """(()=>{const d=document.querySelector('.details');
                   const img=document.querySelector('.preview-zoomable .zoomwrap img');
                   const b=document.querySelector('.preview-zoomable .zoom-badge');
                   return {scrollTop:d.scrollTop, transform:img.style.transform,
                           badge:b?b.textContent:null};})()"""
        )
        print("4) wheel para baixo no modo zoom:", r3)

        for _ in range(4):
            c.wheel(pos["x"], pos["y"], -120)
        c.js("new Promise(r=>setTimeout(r,250))")
        r4 = c.js(
            """(()=>{const img=document.querySelector('.preview-zoomable .zoomwrap img');
                   const b=document.querySelector('.preview-zoomable .zoom-badge');
                   return {transform:img.style.transform, badge:b?b.textContent:null};})()"""
        )
        print("5) wheel para cima:", r4)

        # 4. Esc sai do zoom sem esconder a janela
        c.send(
            "Input.dispatchKeyEvent",
            {"type": "keyDown", "key": "Escape", "code": "Escape", "windowsVirtualKeyCode": 27},
        )
        c.send(
            "Input.dispatchKeyEvent",
            {"type": "keyUp", "key": "Escape", "code": "Escape", "windowsVirtualKeyCode": 27},
        )
        c.js("new Promise(r=>setTimeout(r,300))")
        r5 = c.js(
            """(()=>{const z=document.querySelector('.preview-zoomable .zoomwrap');
                   return {armed:z?z.dataset.armed:null,
                           transform:z?z.querySelector('img').style.transform:null,
                           appVivo:!!document.querySelector('.row')};})()"""
        )
        print("6) depois do Esc:", r5)

        # 5. botao ⤢ abre o card grande e o zoom desce abaixo de 100%
        c.js("document.querySelector('.preview-open').click()")
        c.js("new Promise(r=>setTimeout(r,400))")
        r6 = c.js(
            """(()=>{const card=document.querySelector('.viewer-card');
                   if(!card) return {abriu:false};
                   const r=card.getBoundingClientRect();
                   const w=document.querySelector('.viewer-body .zoomwrap');
                   const b=w.getBoundingClientRect();
                   return {abriu:true, card:[Math.round(r.width),Math.round(r.height)],
                           cx:Math.round(b.left+b.width/2), cy:Math.round(b.top+b.height/2)};})()"""
        )
        print("7) card grande:", r6)
        if r6.get("abriu"):
            for _ in range(5):
                c.wheel(r6["cx"], r6["cy"], 120)
            c.js("new Promise(r=>setTimeout(r,250))")
            r7 = c.js(
                """(()=>{const img=document.querySelector('.viewer-body img');
                       const b=document.querySelector('.viewer-body .zoom-badge');
                       return {transform:img.style.transform, badge:b?b.textContent:null,
                               rodape:document.querySelector('.viewer-foot span').textContent};})()"""
            )
            print("8) zoom para fora no card:", r7)
            c.send(
                "Input.dispatchKeyEvent",
                {"type": "keyDown", "key": "Escape", "code": "Escape", "windowsVirtualKeyCode": 27},
            )
            c.send(
                "Input.dispatchKeyEvent",
                {"type": "keyUp", "key": "Escape", "code": "Escape", "windowsVirtualKeyCode": 27},
            )
            c.js("new Promise(r=>setTimeout(r,300))")
            print("9) Esc fechou o card:", c.js("!document.querySelector('.viewer-card')"))
    finally:
        c.js("(()=>{const s=document.getElementById('spacer-prova');if(s)s.remove();const d=document.querySelector('.details');if(d)d.scrollTop=0;return 1})()")
        c.close()


if __name__ == "__main__":
    main()
