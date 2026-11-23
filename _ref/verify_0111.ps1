param([string]$Step = "multi")
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class V {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, int dx, int dy, uint d, IntPtr e);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint f, IntPtr e);
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string c, string n);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  public struct RECT { public int Left, Top, Right, Bottom; }
  public static void Move(int x, int y) { SetCursorPos(x, y); }
  public static void CtrlClick(int x, int y) {
    SetCursorPos(x, y); System.Threading.Thread.Sleep(120);
    keybd_event(0x11, 0, 0, IntPtr.Zero); System.Threading.Thread.Sleep(40);
    mouse_event(0x0002, 0, 0, 0, IntPtr.Zero); System.Threading.Thread.Sleep(60);
    mouse_event(0x0004, 0, 0, 0, IntPtr.Zero); System.Threading.Thread.Sleep(60);
    keybd_event(0x11, 0, 2, IntPtr.Zero);
  }
  public static void Key(byte vk) { keybd_event(vk, 0, 0, IntPtr.Zero); System.Threading.Thread.Sleep(50); keybd_event(vk, 0, 2, IntPtr.Zero); }
  public static void CtrlKey(byte vk) { keybd_event(0x11, 0, 0, IntPtr.Zero); System.Threading.Thread.Sleep(40); Key(vk); keybd_event(0x11, 0, 2, IntPtr.Zero); }
}
'@
$h = [V]::FindWindow('Tauri Window', 'Rayclip')
if ($h -eq [IntPtr]::Zero) { "janela nao encontrada"; exit 1 }
$r = New-Object V+RECT
[void][V]::GetWindowRect($h, [ref]$r)
$w = $r.Right - $r.Left
$out = @("rect=$($r.Left),$($r.Top) ${w}x$($r.Bottom - $r.Top) visivel=$([V]::IsWindowVisible($h))")

switch ($Step) {
  "multi" {
    foreach ($dy in 104, 148, 192) { [V]::CtrlClick($r.Left + 320, $r.Top + $dy); Start-Sleep -Milliseconds 220 }
    [V]::CtrlKey(0x4B)   # Ctrl+K
    Start-Sleep -Milliseconds 700
    [V]::Key(0x28); Start-Sleep -Milliseconds 120
    [V]::Key(0x28); Start-Sleep -Milliseconds 120
    [V]::Key(0x0D)       # Enter
    Start-Sleep -Milliseconds 900
    $out += "passos: 3 ctrl+clique, Ctrl+K, 2xDown, Enter"
  }
  "hover" {
    [V]::Move($r.Left + 300, $r.Top + 210); Start-Sleep -Milliseconds 200
    [V]::Move($r.Left + 306, $r.Top + 214); Start-Sleep -Milliseconds 900
    $out += "mouse sobre 1a linha do dialogo"
  }
  "pin" {
    [V]::Move($r.Left + 90, $r.Top + 214); Start-Sleep -Milliseconds 150
    [V]::Move($r.Left + 92, $r.Top + 216); Start-Sleep -Milliseconds 150
    [V]::CtrlClick($r.Left + 90, $r.Top + 214)
    Start-Sleep -Milliseconds 400
    [V]::Move($r.Left + 700, $r.Top + 400); Start-Sleep -Milliseconds 600
    $out += "icone clicado e mouse levado para longe"
  }
}
$out -join " | "
