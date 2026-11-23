param([string]$Action = "info", [int]$X = 0, [int]$Y = 0, [int]$Wait = 1500)
Add-Type @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public class M {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, int dx, int dy, uint d, IntPtr e);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint f, IntPtr e);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string c, string n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  public static void AltSpace() {
    keybd_event(0x12, 0, 0, IntPtr.Zero); System.Threading.Thread.Sleep(60);
    keybd_event(0x20, 0, 0, IntPtr.Zero); System.Threading.Thread.Sleep(60);
    keybd_event(0x20, 0, 2, IntPtr.Zero); keybd_event(0x12, 0, 2, IntPtr.Zero);
  }
  public static void Click(int x, int y, bool right) {
    SetCursorPos(x, y); System.Threading.Thread.Sleep(140);
    uint down = right ? 0x0008u : 0x0002u, up = right ? 0x0010u : 0x0004u;
    mouse_event(down, 0, 0, 0, IntPtr.Zero); System.Threading.Thread.Sleep(80);
    mouse_event(up, 0, 0, 0, IntPtr.Zero);
  }
  public static string Info() {
    IntPtr h = GetForegroundWindow(); uint pid; GetWindowThreadProcessId(h, out pid);
    var c = new StringBuilder(256); GetClassName(h, c, 256);
    var t = new StringBuilder(256); GetWindowText(h, t, 256);
    return "fg=" + pid + "/" + c + "/" + t;
  }
}
'@
$app = [M]::FindWindow('Tauri Window','Rayclip')
$out = @()
switch ($Action) {
  "altspace" { [M]::AltSpace(); Start-Sleep -Milliseconds 1200; $out += "app visivel=" + [M]::IsWindowVisible($app) }
  "click"     { [M]::Click($X, $Y, $false); Start-Sleep -Milliseconds $Wait; $out += "app visivel=" + [M]::IsWindowVisible($app) }
  "rclick"    { [M]::Click($X, $Y, $true);  Start-Sleep -Milliseconds $Wait; $out += "app visivel=" + [M]::IsWindowVisible($app) }
  "info"      { }
  "outside"   { [M]::Click($X, $Y, $false); Start-Sleep -Milliseconds $Wait; $out += "app visivel=" + [M]::IsWindowVisible($app) }
}
$out += [M]::Info()
$out -join " | "
