import re
path = r"C:\Program Files\WindowsApps\Raycast.Raycast_2.13.1.0_x64__qypenmj9wpt2a\Raycast\Raycast.dll"
data = open(path,'rb').read()
cur = bytearray(); out=[]
for b in data:
    if 32 <= b < 127: cur.append(b)
    else:
        if len(cur)>=6: out.append(cur.decode())
        cur = bytearray()
pat = re.compile(r'^Raycast(\.[A-Za-z0-9_`]+)+$')
ns = sorted({x for x in out if pat.match(x)})
# collapse to namespaces (drop last segment)
import collections
c = collections.Counter()
for n in ns:
    parts = n.split('.')
    # heuristic: namespace = up to segment before first capitalized type that looks like class
    c['.'.join(parts[:4])] += 1
for k,v in sorted(c.items()):
    print(v, k)
print("---- clipboard-related types ----")
for n in ns:
    if 'Clipboard' in n: print(n)
