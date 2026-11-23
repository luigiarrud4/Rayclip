param(
  [int]$X = 530, [int]$Y = 756,
  [int]$WaitAfter = 1600,
  [switch]$NoClick
)
Add-Type @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public class T {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, int dx, int dy, uint d, IntPtr e);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint f, IntPtr e);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string c, string n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  public static void Click(int x, int y) {
    SetCursorPos(x, y); System.Threading.Thread.Sleep(150);
    mouse_event(0x0002, 0, 0, 0, IntPtr.Zero);
    System.Threading.Thread.Sleep(70);
    mouse_event(0x0004, 0, 0, 0, IntPtr.Zero);
  }
  public static void AltSpace() {
    keybd_event(0x12, 0, 0, IntPtr.Zero); System.Threading.Thread.Sleep(60);
    keybd_event(0x20, 0, 0, IntPtr.Zero); System.Threading.Thread.Sleep(60);
    keybd_event(0x20, 0, 2, IntPtr.Zero); keybd_event(0x12, 0, 2, IntPtr.Zero);
  }
  public static string FgInfo() {
    IntPtr h = GetForegroundWindow(); uint pid; GetWindowThreadProcessId(h, out pid);
    var sb = new StringBuilder(256); GetClassName(h, sb, 256);
    return "fg=" + h + " pid=" + pid + " class=" + sb;
  }
}
'@
$app = [T]::FindWindow('Tauri Window','Rayclip')
$log = @()
$log += "antes: visivel=" + [T]::IsWindowVisible($app) + " " + [T]::FgInfo()
if (-not [T]::IsWindowVisible($app)) { [T]::AltSpace(); Start-Sleep -Milliseconds 1300 }
$log += "aberto: visivel=" + [T]::IsWindowVisible($app) + " " + [T]::FgInfo()
if (-not $NoClick) {
  [T]::Click($X, $Y)
  Start-Sleep -Milliseconds $WaitAfter
  $log += "apos click: visivel=" + [T]::IsWindowVisible($app) + " " + [T]::FgInfo()
}
$log -join "`n"
