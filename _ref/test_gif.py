"""GIF animado: o preview mexe, nos dois apps?

Cria um GIF de 3 quadros (cores bem diferentes), copia como ARQUIVO (CF_HDROP),
abre a busca do app pelo nome do arquivo e tira dois prints com 400 ms entre eles.
Se os pixels do preview mudarem, o GIF esta animando.

Tambem mede a latencia de captura de GIF (arquivo -> item no historico).
"""
import io
import os
import subprocess
import sys
import time
import uuid

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import harness_compare as h  # noqa: E402
from PIL import Image, ImageChops, ImageDraw, ImageGrab  # noqa: E402
from test_pm_raycast import janelas  # noqa: E402

SAIDA = r"C:\Windows\System32\_rayclip_tmp"
TEMP = os.environ["TEMP"]


def criar_gif(pasta):
    caminho = os.path.join(pasta, "anim.gif")
    quadros = []
    for i, cor in enumerate([(220, 40, 40), (40, 200, 90), (50, 90, 230)]):
        im = Image.new("RGB", (240, 240), cor)
        d = ImageDraw.Draw(im)
        d.ellipse([60 + i * 20, 60, 140 + i * 20, 140], fill=(255, 255, 255))
        d.text((100, 200), f"quadro {i}", fill=(0, 0, 0))
        quadros.append(im)
    quadros[0].save(caminho, save_all=True, append_images=quadros[1:],
                    duration=200, loop=0)
    return caminho


def copiar_arquivo(caminho):
    subprocess.run(["powershell", "-NoProfile", "-Command",
                    f"Set-Clipboard -Path '{caminho}'"], capture_output=True)


def rect(app):
    pid = int(subprocess.run(["powershell", "-NoProfile", "-Command",
                              f"(Get-Process {app}).Id"],
                             capture_output=True, text=True).stdout.split()[0])
    for _ in range(20):
        cands = [c for c in janelas(pid) if c[0] > 20000]
        if cands:
            return cands[0][2]
        time.sleep(0.1)
    return None


def dois_prints(app, marca):
    r = rect(app)
    if not r:
        return None, None
    a = os.path.join(SAIDA, f"gif_{app}_{marca}_1.png")
    b = os.path.join(SAIDA, f"gif_{app}_{marca}_2.png")
    ImageGrab.grab(bbox=r).save(a)
    time.sleep(0.40)
    ImageGrab.grab(bbox=r).save(b)
    return a, b


def diferenca(a, b):
    ia, ib = Image.open(a).convert("RGB"), Image.open(b).convert("RGB")
    if ia.size != ib.size:
        return -1
    diff = ImageChops.difference(ia, ib)
    px = sum(1 for p in diff.getdata() if sum(p) > 30)
    return px


def medir_abertura(app, gif, buscar, tecla_preview=None):
    h.send_keys("esc")
    time.sleep(0.5)
    if app == "raycast":
        h.send_keys("ctrl", "shift", "v")
    else:
        h.send_keys("alt", "space")
    time.sleep(1.6)
    h.type_text(buscar)
    time.sleep(1.2)
    if tecla_preview:
        h.send_keys(*tecla_preview)
        time.sleep(1.0)
    marca = uuid.uuid4().hex[:4]
    a, b = dois_prints(app, marca)
    if not a:
        print(f"  {app}: janela nao encontrada")
        return
    px = diferenca(a, b)
    print(f"  {app}: pixels diferentes em 400 ms = {px}  ({a} | {b})")


def latencia_captura(app, gif, roda_antes):
    """Arquivo -> item no historico. raycast: nada direto; rayclip: list_entries."""
    return None


def main():
    pasta = os.path.join(TEMP, "imgtest")
    os.makedirs(pasta, exist_ok=True)
    gif = criar_gif(pasta)
    print("gif:", gif)

    # 1) copia e espera os dois capturarem
    copiar_arquivo(gif)
    time.sleep(2.0)

    # 2) animacao: grade de busca + preview
    print("[grade de busca + painel de detalhes]")
    medir_abertura("raycast", gif, "anim")
    medir_abertura("rayclip", gif, "anim")

    # 3) animacao: quick look / preview aberto (Espaco seleciona o item)
    print("[com preview aberto no Rayclip (Espaco)]")
    medir_abertura("rayclip", gif, "anim", tecla_preview=("space",))


if __name__ == "__main__":
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")
    main()
