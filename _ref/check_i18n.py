import io
import glob
import sys

sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")

BS = chr(92)
Q = chr(34)

def read(p):
    return io.open(p, encoding="utf-8").read()

def scan_string(src, i):
    """src[i] == '"' -> devolve (texto_bruto, indice depois da aspa final)."""
    j = i + 1
    out = []
    while j < len(src):
        ch = src[j]
        if ch == BS:
            out.append(src[j : j + 2])
            j += 2
            continue
        if ch == Q:
            return "".join(out), j + 1
        out.append(ch)
        j += 1
    return None, j

def unescape(s):
    return s.replace(BS + BS, BS).replace(BS + Q, Q)

# ---- chaves do dicionario PT
keys = set()
dict_src = read("src/lib/i18n.ts")
for line in dict_src.split(chr(10)):
    s = line.strip()
    if not s or s[0] not in (Q, "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M",
                             "N", "O", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z", "_"):
        continue
    if s[0] == Q:
        raw, after = scan_string(s, 0)
        if raw is not None and s[after:].lstrip().startswith(":"):
            keys.add(unescape(raw))
    else:
        if ":" not in s:
            continue
        name, rest = s.split(":", 1)
        if rest.lstrip().startswith('"'):
            keys.add(name.strip())
            raw, after = scan_string(rest.lstrip(), 0)
            if raw is not None:
                pass

# ---- usos de t("...") no codigo
files = sorted(glob.glob("src/**/*.tsx", recursive=True) + glob.glob("src/**/*.ts", recursive=True))
missing = []
total = 0
for f in files:
    if f.endswith("i18n.ts"):
        continue
    src = read(f)
    i = 0
    while True:
        i = src.find("t(" + Q, i)
        if i < 0:
            break
        # ignora nomes tipo "set(" / "format("
        prev = src[i - 1] if i else " "
        if prev.isalnum() or prev in "_.":
            i += 1
            continue
        raw, _ = scan_string(src, i + 2)
        i += 2
        if raw is None:
            continue
        lit = unescape(raw)
        total += 1
        if lit not in keys:
            line = src[:i].count(chr(10)) + 1
            missing.append("%s:%d  %s" % (f, line, lit[:90]))

print("chaves PT:", len(keys))
print("usos t():", total)
print("sem traducao:", len(missing))
for x in missing:
    print("  ", x)
