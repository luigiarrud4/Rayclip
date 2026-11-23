import sys, time
sys.path.insert(0, r'C:\Users\Lugia\Projects\rayclip\_ref')
import harness_compare as H
import ctypes, ctypes.wintypes as w

a = H.Alvo(); time.sleep(0.4)
H.close_app('rayclip', a)
H.send_keys('alt', 'space'); time.sleep(1.0)
H.type_text('hx'); time.sleep(0.6)
h = H.win_of('rayclip')
r = w.RECT(); H.user32.GetWindowRect(h, ctypes.byref(r))
# clica na primeira linha da lista: move o foco para fora do campo de busca
x = r.left + 250
y = r.top + 120
H.user32.SetCursorPos(x, y); time.sleep(0.08)
H.user32.mouse_event(0x0002, 0, 0, 0, 0); time.sleep(0.03); H.user32.mouse_event(0x0004, 0, 0, 0, 0)
time.sleep(0.3)
for i in range(6):
    H.send_keys('delete', gap=0.006); time.sleep(0.12)
H.send_keys('esc'); time.sleep(0.4)
# confere
a.focus(); a.clear(); time.sleep(0.15)
H.type_text('hx'); time.sleep(0.5)
print('OK')
