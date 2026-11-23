"""Roda o alvo_helper.py guardando qualquer excecao num arquivo.

pythonw.exe nao tem stderr: sem isso um erro no helper (que roda em integridade
media) desaparece sem deixar rastro."""
import os
import runpy
import sys
import traceback

REF = os.path.dirname(os.path.abspath(__file__))
ERRO = os.path.join(REF, "alvo_err.txt")

try:
    runpy.run_path(os.path.join(REF, "alvo_helper.py"), run_name="__main__")
except BaseException:
    with open(ERRO, "w", encoding="utf-8") as f:
        f.write(traceback.format_exc())
    sys.exit(1)
