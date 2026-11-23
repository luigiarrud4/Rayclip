Add-Type -AssemblyName System.Windows.Forms
Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices;
public class FB3 {
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string c,string n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f,uint dx,uint dy,uint d,UIntPtr e);
  public struct RECT { public int L,T,R,B; }
}
"@
$log = "C:\Windows\System32\_rayclip_tmp\focus_test3.log"
Remove-Item $log -ErrorAction SilentlyContinue
function Say($m) { Add-Content -Path $log -Value $m }
function Click($x, $y) {
  [FB3]::SetCursorPos($x, $y)
  Start-Sleep -Milliseconds 120
  [FB3]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)
  [FB3]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
}

$h = [IntPtr]::Zero
$sw = [Diagnostics.Stopwatch]::StartNew()
while ($sw.ElapsedMilliseconds -lt 9000) {
  $h = [FB3]::FindWindow("Tauri Window","Rayclip")
  if ($h -ne [IntPtr]::Zero -and [FB3]::IsWindowVisible($h)) { break }
  Start-Sleep -Milliseconds 50
}
$r = New-Object FB3+RECT
[FB3]::GetWindowRect($h, [ref]$r) | Out-Null
Say ("visivel=" + [FB3]::IsWindowVisible($h) + " rect=" + $r.L + "," + $r.T + " " + ($r.R-$r.L) + "x" + ($r.B-$r.T))

# 1. clica dentro (painel de detalhes, lado direito) para garantir foco
Click ($r.L + 600) ($r.T + 150)
Start-Sleep -Milliseconds 1500
Say ("depois de clicar dentro: visivel=" + [FB3]::IsWindowVisible($h))

# 2. clica fora
$b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
if ($r.L -gt $b.Width / 2) { $x = 80 } else { $x = $b.Width - 80 }
Click $x 80
Start-Sleep -Milliseconds 700
Say ("+0.7s depois de clicar fora: visivel=" + [FB3]::IsWindowVisible($h))
Start-Sleep -Milliseconds 900
Say ("+1.6s depois de clicar fora: visivel=" + [FB3]::IsWindowVisible($h))
