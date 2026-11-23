Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices;
public class FB {
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string c,string n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f,uint dx,uint dy,uint d,UIntPtr e);
  public struct RECT { public int L,T,R,B; }
}
"@
$log = "C:\Windows\System32\_rayclip_tmp\focus_test.log"
Remove-Item $log -ErrorAction SilentlyContinue
function Say($m) { Add-Content -Path $log -Value $m }

$h = [FB]::FindWindow("Tauri Window","Rayclip")
$r = New-Object FB+RECT
[FB]::GetWindowRect($h, [ref]$r) | Out-Null
Say ("0s visivel=" + [FB]::IsWindowVisible($h) + " rect=" + $r.L + "," + $r.T + " " + ($r.R-$r.L) + "x" + ($r.B-$r.T))

Start-Sleep -Milliseconds 2000

# clique no desktop, longe da janela
$b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
if ($r.L -gt $b.Width / 2) { $x = 80 } else { $x = $b.Width - 80 }
[FB]::SetCursorPos($x, 80)
Start-Sleep -Milliseconds 200
[FB]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)
[FB]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
Say ("clique fora em " + $x + ",80")

Start-Sleep -Milliseconds 400
Say ("+0.4s visivel=" + [FB]::IsWindowVisible($h))

Start-Sleep -Milliseconds 1200
Say ("+1.6s visivel=" + [FB]::IsWindowVisible($h))

$bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
$bmp.Save("C:\Windows\System32\_rayclip_tmp\focus_after.png", [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()
