Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices;
public class FB4 {
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string c,string n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f,uint dx,uint dy,uint d,UIntPtr e);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern int GetWindowTextW(IntPtr h, System.Text.StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern int GetClassNameW(IntPtr h, System.Text.StringBuilder s, int n);
  public struct RECT { public int L,T,R,B; }
}
"@
$log = "C:\Windows\System32\_rayclip_tmp\focus_test4.log"
Remove-Item $log -ErrorAction SilentlyContinue
function Say($m) { Add-Content -Path $log -Value $m }
function Click($x, $y) {
  [FB4]::SetCursorPos($x, $y)
  Start-Sleep -Milliseconds 140
  [FB4]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)
  [FB4]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
}
function FgInfo() {
  $f = [FB4]::GetForegroundWindow()
  $t = New-Object System.Text.StringBuilder 256
  $c = New-Object System.Text.StringBuilder 256
  [FB4]::GetWindowTextW($f, $t, 256) | Out-Null
  [FB4]::GetClassNameW($f, $c, 256) | Out-Null
  return ("fg class='" + $c.ToString() + "' title='" + $t.ToString() + "'")
}
function Shot($n, $r) {
  $bmp = New-Object System.Drawing.Bitmap (($r.R-$r.L)+40),(($r.B-$r.T)+40)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen(($r.L-20), ($r.T-20), 0, 0, $bmp.Size)
  $bmp.Save("C:\Windows\System32\_rayclip_tmp\$n.png", [System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose(); $bmp.Dispose()
}

$h = [IntPtr]::Zero
$sw = [Diagnostics.Stopwatch]::StartNew()
while ($sw.ElapsedMilliseconds -lt 9000) {
  $h = [FB4]::FindWindow("Tauri Window","Rayclip")
  if ($h -ne [IntPtr]::Zero -and [FB4]::IsWindowVisible($h)) { break }
  Start-Sleep -Milliseconds 50
}
$r = New-Object FB4+RECT
[FB4]::GetWindowRect($h, [ref]$r) | Out-Null
Say ("janela " + $r.L + "," + $r.T + " " + ($r.R-$r.L) + "x" + ($r.B-$r.T))

# abre configurações pelo ícone do rodapé
Click ($r.L + 30) ($r.B - 28)
Start-Sleep -Milliseconds 900
Say ("apos icone: visivel=" + [FB4]::IsWindowVisible($h) + " | " + (FgInfo))
Shot "f4_settings" $r

# aba History (3a da sidebar) e o seletor "Maximum entries"
Click ($r.L + 95) ($r.T + 96)
Start-Sleep -Milliseconds 600
Say ("apos aba History: visivel=" + [FB4]::IsWindowVisible($h))
Click ($r.L + 700) ($r.T + 70)
Start-Sleep -Milliseconds 900
Say ("apos abrir select: visivel=" + [FB4]::IsWindowVisible($h) + " | " + (FgInfo))
Shot "f4_select" $r

# escolhe um item e fecha
[FB4]::SetCursorPos(($r.L + 700), ($r.T + 140))
Start-Sleep -Milliseconds 150
[FB4]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)
[FB4]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
Start-Sleep -Milliseconds 900
Say ("apos escolher: visivel=" + [FB4]::IsWindowVisible($h) + " | " + (FgInfo))
Shot "f4_after" $r
