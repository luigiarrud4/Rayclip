import re, sys, collections
p = r"C:\Program Files\WindowsApps\Raycast.Raycast_2.13.1.0_x64__qypenmj9wpt2a\Raycast\frontend\main-window-CWpbGFYh.css"
d = open(p, encoding='utf-8', errors='replace').read()
rules = re.findall(r'([^{}@]+)\{([^{}]*)\}', d)
def find(sub, limit=14):
    print(f"\n#### {sub}")
    n=0
    for sel, body in rules:
        if sub in sel:
            print(sel.strip()[:150], '{', body.strip()[:300], '}')
            n+=1
            if n>=limit: break
for s in ['list-item__root','list-item__title','list-item__content','navigation-bar','action-bar','details-view','search-field','keycap','dropdown','section-header']:
    find(s)
