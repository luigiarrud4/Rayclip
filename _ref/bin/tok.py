import re
p = r"C:\Program Files\WindowsApps\Raycast.Raycast_2.13.1.0_x64__qypenmj9wpt2a\Raycast\frontend\main-window-CWpbGFYh.css"
d = open(p, encoding='utf-8', errors='replace').read()
for key in ['--space-2:', '--radius-', '--text-', '--color-text-secondary:', '--color-surface-control', '--icon-16:']:
    i = d.find(key)
    print(f"\n### {key} idx={i}")
    if i>0: print(d[max(0,i-300):i+900].replace(';',';\n'))
