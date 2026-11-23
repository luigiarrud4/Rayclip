"""Extrai strings (ASCII e UTF-16LE) de um binario -- o "modo strings" sem IDA.

Uso:  python _ref/strings_dump.py <arquivo> [minlen] > saida.txt
"""
import sys
import re

path = sys.argv[1]
minlen = int(sys.argv[2]) if len(sys.argv) > 2 else 4

data = open(path, "rb").read()

# --- ASCII / UTF-8 imprimivel
ascii_re = re.compile(rb"[\x20-\x7e]{%d,}" % minlen)
seen = set()
for m in ascii_re.finditer(data):
    s = m.group().decode("ascii", "replace")
    if s not in seen:
        seen.add(s)
        print(s)

# --- UTF-16LE (WPF guarda muitos rotulos em UTF-16)
utf16_re = re.compile(rb"(?:[\x20-\x7e]\x00){%d,}" % minlen)
for m in utf16_re.finditer(data):
    try:
        s = m.group().decode("utf-16-le")
    except UnicodeDecodeError:
        continue
    if s not in seen:
        seen.add(s)
        print(s)
