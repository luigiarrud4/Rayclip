Add-Type @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public class SD {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, int dx, int dy, uint d, IntPtr e);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint f, IntPtr e);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string c, string n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  public static void AltSpace() {
    keybd_event(0x12, 0, 0, IntPtr.Zero); System.Threading.Thread.Sleep(60);
    keybd_event(0x20, 0, 0, IntPtr.Zero); System.Threading.Thread.Sleep(60);
    keybd_event(0x20, 0, 2, IntPtr.Zero); keybd_event(0x12, 0, 2, IntPtr.Zero);
  }
  public static void Key(byte vk) {
    keybd_event(vk, 0, 0, IntPtr.Zero); System.Threading.Thread.Sleep(60);
    keybd_event(vk, 0, 2, IntPtr.Zero);
  }
  public static void Click(int x, int y, bool right) {
    SetCursorPos(x, y); System.Threading.Thread.Sleep(160);
    uint down = right ? 0x0008u : 0x0002u, up = right ? 0x0010u : 0x0004u;
    mouse_event(down, 0, 0, 0, IntPtr.Zero); System.Threading.Thread.Sleep(90);
    mouse_event(up, 0, 0, 0, IntPtr.Zero);
  }
  public static string Fg() {
    IntPtr h = GetForegroundWindow(); uint pid; GetWindowThreadProcessId(h, out pid);
    var c = new StringBuilder(256); GetClassName(h, c, 256);
    return pid + "/" + c;
  }
}
'@
$app = [SD]::FindWindow('Tauri Window','Rayclip')
$log = @()
[SD]::AltSpace(); Start-Sleep -Milliseconds 1300
$log += "show: visivel=" + [SD]::IsWindowVisible($app) + " fg=" + [SD]::Fg()

# 1. botão direito numa linha
[SD]::Click(657, 400, $true); Start-Sleep -Milliseconds 900
$log += "panel: visivel=" + [SD]::IsWindowVisible($app) + " fg=" + [SD]::Fg()

# 2. navega com o teclado ate "Save Image As..." (indice 5) e abre o submenu
for ($i = 0; $i -lt 5; $i++) { [SD]::Key(0x28); Start-Sleep -Milliseconds 120 }
[SD]::Key(0x0D); Start-Sleep -Milliseconds 900
$log += "fmt-menu: visivel=" + [SD]::IsWindowVisible($app) + " fg=" + [SD]::Fg()

# 3. Enter no primeiro formato (PNG padrao)
[SD]::Key(0x0D); Start-Sleep -Milliseconds 2500
$dlg = [SD]::FindWindow('#32770','Save image as')
$log += "dialogo: hwnd=" + $dlg + " visivel=" + [SD]::IsWindowVisible($dlg)
$log += "com-dialogo: app visivel=" + [SD]::IsWindowVisible($app) + " fg=" + [SD]::Fg()

# 4. cancela o diálogo
if ($dlg -ne [IntPtr]::Zero) { [SD]::PostMessage($dlg, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null }
Start-Sleep -Milliseconds 1500
$log += "apos-cancelar: app visivel=" + [SD]::IsWindowVisible($app) + " fg=" + [SD]::Fg()
$log -join "`n"
