param([int]$X1, [int]$Y1, [int]$X2, [int]$Y2, [switch]$NoShow)
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class S {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, int dx, int dy, uint d, IntPtr e);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint f, IntPtr e);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string c, string n);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  public static void AltSpace() {
    keybd_event(0x12, 0, 0, IntPtr.Zero); System.Threading.Thread.Sleep(60);
    keybd_event(0x20, 0, 0, IntPtr.Zero); System.Threading.Thread.Sleep(60);
    keybd_event(0x20, 0, 2, IntPtr.Zero); keybd_event(0x12, 0, 2, IntPtr.Zero);
  }
  public static void Drag(int x1, int y1, int x2, int y2) {
    SetCursorPos(x1, y1); System.Threading.Thread.Sleep(150);
    mouse_event(0x0002, 0, 0, 0, IntPtr.Zero); System.Threading.Thread.Sleep(120);
    int steps = 14;
    for (int i = 1; i <= steps; i++) {
      SetCursorPos(x1 + (x2 - x1) * i / steps, y1 + (y2 - y1) * i / steps);
      System.Threading.Thread.Sleep(25);
    }
    System.Threading.Thread.Sleep(150);
    mouse_event(0x0004, 0, 0, 0, IntPtr.Zero);
  }
  public static string Rect(IntPtr h) { RECT r; GetWindowRect(h, out r); return "[" + r.L + "," + r.T + "," + r.R + "," + r.B + "]"; }
}
'@
$app = [S]::FindWindow('Tauri Window','Rayclip')
if (-not $NoShow) { [S]::AltSpace(); Start-Sleep -Milliseconds 1200 }
$before = [S]::Rect($app)
[S]::Drag($X1, $Y1, $X2, $Y2)
Start-Sleep -Milliseconds 400
[S]::SetForegroundWindow($app) | Out-Null
"rect antes=$before depois=" + [S]::Rect($app)
