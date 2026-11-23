import re, sys, collections

def utf8_strings(path, minlen=6):
    data = open(path,'rb').read()
    out = []
    cur = bytearray()
    for b in data:
        if 32 <= b < 127:
            cur.append(b)
        else:
            if len(cur) >= minlen:
                out.append(cur.decode('ascii','ignore'))
            cur = bytearray()
    if len(cur) >= minlen: out.append(cur.decode('ascii','ignore'))
    return out

path = r"C:\Program Files\WindowsApps\Raycast.Raycast_2.13.1.0_x64__qypenmj9wpt2a\Raycast\Raycast.dll"
s = utf8_strings(path)
print("total strings:", len(s))
clip = [x for x in s if 'clipboard' in x.lower()]
print("clipboard strings:", len(clip))
for x in sorted(set(clip))[:200]:
    print("  ", x)
