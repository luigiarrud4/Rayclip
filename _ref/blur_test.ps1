Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices;
public class BT {
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string c,string n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f,uint dx,uint dy,uint d,UIntPtr e);
  [DllImport("user32.dll")] public static extern int GetWindowTextW(IntPtr h, System.Text.StringBuilder s, int n);
  public struct RECT { public int L,T,R,B; }
}
"@
$h=[BT]::FindWindow("Tauri Window","Rayclip")
$r=New-Object BT+RECT; [BT]::GetWindowRect($h,[ref]$r)|Out-Null
Write-Output ("antes: visivel=" + [BT]::IsWindowVisible($h) + " rect=" + $r.L + "," + $r.T + " " + ($r.R-$r.L) + "x" + ($r.B-$r.T))
$b=[System.Windows.Forms.Screen]::PrimaryScreen.Bounds
# clica num ponto do desktop, longe da janela
$x = if ($r.L -gt $b.Width/2) { 60 } else { $b.Width - 60 }
$y = 60
[BT]::SetCursorPos($x,$y); Start-Sleep -Milliseconds 250
[BT]::mouse_event(0x0002,0,0,0,[UIntPtr]::Zero); [BT]::mouse_event(0x0004,0,0,0,[UIntPtr]::Zero)
Start-Sleep -Milliseconds 1200
$fg=[BT]::GetForegroundWindow()
$sb=New-Object System.Text.StringBuilder 256; [BT]::GetWindowTextW($fg,$sb,256)|Out-Null
Write-Output ("cliquei em " + $x + "," + $y + " | fg='" + $sb.ToString() + "' | depois: visivel=" + [BT]::IsWindowVisible($h))
$bmp=New-Object System.Drawing.Bitmap $b.Width,$b.Height
$g=[System.Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($b.Location,[System.Drawing.Point]::Empty,$b.Size)
$bmp.Save("C:\Windows\System32\_rayclip_tmp\blur_after.png",[System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose();$bmp.Dispose()
