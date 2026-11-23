Add-Type -AssemblyName System.Windows.Forms
Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices;
public class FB2 {
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string c,string n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f,uint dx,uint dy,uint d,UIntPtr e);
}
"@
$log = "C:\Windows\System32\_rayclip_tmp\focus_test2.log"
Remove-Item $log -ErrorAction SilentlyContinue
function Say($m) { Add-Content -Path $log -Value $m }

$sw = [Diagnostics.Stopwatch]::StartNew()
$h = [IntPtr]::Zero
while ($sw.ElapsedMilliseconds -lt 8000) {
  $h = [FB2]::FindWindow("Tauri Window","Rayclip")
  if ($h -ne [IntPtr]::Zero -and [FB2]::IsWindowVisible($h)) { break }
  Start-Sleep -Milliseconds 40
}
Say ("janela visivel em " + $sw.ElapsedMilliseconds + "ms")
# clique fora imediatamente (dentro do antigo periodo de carencia de 700ms)
$b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
[FB2]::SetCursorPos(($b.Width - 80), 80)
[FB2]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)
[FB2]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
Say ("clique fora em " + $sw.ElapsedMilliseconds + "ms")
Start-Sleep -Milliseconds 1600
Say ("+1.6s visivel=" + [FB2]::IsWindowVisible($h))
