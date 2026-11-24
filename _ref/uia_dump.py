"""Explora a árvore de acessibilidade (UIA) da janela do Rayclip.

Objetivo: achar uma forma de acionar uma linha **sem** depender de foco/mouse —
neste ambiente o console do agente retoma o foco em milissegundos e o app (que se
esconde ao perder o foco) nunca fica na frente tempo suficiente para um clique
sintético. UIA fala direto com o provedor (Chromium), sem foco.

Uso: python _ref/uia_dump.py  -> escreve %TEMP%\\uia_dump.txt
"""

import io
import os

import uiautomation as auto

SAIDA = os.path.join(os.environ["TEMP"], "uia_dump.txt")
linhas = []


def diz(msg):
    linhas.append(msg)


def dump(ctrl, nivel=0, max_nivel=6):
    if nivel > max_nivel:
        return
    try:
        nome = (ctrl.Name or "").replace("\n", " ")[:60]
        tipo = ctrl.ControlTypeName
        padroes = []
        for padrao, rotulo in (
            (auto.PatternId.InvokePattern, "Invoke"),
            (auto.PatternId.SelectionItemPattern, "SelectionItem"),
            (auto.PatternId.LegacyIAccessiblePattern, "Legacy"),
        ):
            try:
                if ctrl.IsPatternAvailable(padrao):
                    padroes.append(rotulo)
            except Exception:
                pass
        diz(f"{'  ' * nivel}{tipo} :: {nome!r} [{'/'.join(padroes)}]")
    except Exception as exc:  # noqa: BLE001
        diz(f"{'  ' * nivel}! {type(exc).__name__}: {exc}")
        return
    try:
        filhos = ctrl.GetChildren()
    except Exception:
        filhos = []
    for f in filhos:
        dump(f, nivel + 1, max_nivel)


def main():
    janela = auto.WindowControl(Name="Rayclip", searchDepth=2)
    if not janela.Exists(1):
        diz("janela 'Rayclip' nao encontrada (ela pode estar escondida na bandeja)")
        return
    diz(f"janela: {janela.ControlTypeName} {janela.Name!r}")
    diz(f"retangulo: {janela.BoundingRectangle}")
    dump(janela)


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:  # noqa: BLE001
        diz(f"ERRO: {type(exc).__name__}: {exc}")
    io.open(SAIDA, "w", encoding="utf-8", newline="\n").write("\n".join(linhas) + "\n")
    print("linhas:", len(linhas))
