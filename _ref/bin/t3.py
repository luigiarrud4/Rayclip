import re
path = r"C:\Program Files\WindowsApps\Raycast.Raycast_2.13.1.0_x64__qypenmj9wpt2a\Raycast\Raycast.dll"
data = open(path,'rb').read()
cur=bytearray(); out=[]
for b in data:
    if 32 <= b < 127: cur.append(b)
    else:
        if len(cur)>=4: out.append(cur.decode())
        cur=bytearray()
S=set(out)
def show(label, pat, lim=40, minlen=0):
    r = sorted({x for x in S if re.search(pat, x) and len(x)>=minlen})
    print(f"=== {label} ({len(r)}) ===")
    for x in r[:lim]: print("   ", x[:160])
show("content type enums", r'^(Text|Link|Image|Color|File|Files|Pdf|QrCode|RichText|Html)$')
show("clipboard labels ui", r'(?i)^(paste|actions|all types|clipboard history|type to filter|search|metadata|information|name|where|size|source|type|url|title)$')
show("sqlite", r'(?i)(sqlite|CREATE TABLE|INSERT INTO|\.db$)')
show("fonts", r'(?i)^(Inter|SF Pro|Segoe UI|Segoe UI Variable|Roboto|Geist)[A-Za-z ]*$')
show("color tokens", r'(?i)^(raycast-[a-z0-9-]+|[A-Za-z]+Brush|[A-Za-z]+Color)$')
show("shortcut keycaps", r'(?i)^(ctrl|cmd|alt|shift|enter|return|escape|tab|space)$')
