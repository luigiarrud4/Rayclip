param([int]$dx=30,[int]$dy=28)
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices;
public class CT {
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string c,string n);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f,uint dx,uint dy,uint d,UIntPtr e);
  public struct RECT { public int L,T,R,B; }
}
"@
$h=[CT]::FindWindow("Tauri Window","Rayclip")
$r=New-Object CT+RECT; [CT]::GetWindowRect($h,[ref]$r)|Out-Null
function Grab($n){
  $bmp=New-Object System.Drawing.Bitmap (($r.R-$r.L)+40),(($r.B-$r.T)+40)
  $g=[System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen(($r.L-20),($r.T-20),0,0,$bmp.Size)
  $bmp.Save("C:\Windows\System32\_rayclip_tmp\$n.png",[System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose(); $bmp.Dispose()
}
Grab "ct_before"
[CT]::SetCursorPos(($r.L+$dx),($r.B-$dy)); Start-Sleep -Milliseconds 250
[CT]::mouse_event(0x0002,0,0,0,[UIntPtr]::Zero); [CT]::mouse_event(0x0004,0,0,0,[UIntPtr]::Zero)
Start-Sleep -Milliseconds 400
[CT]::mouse_event(0x0002,0,0,0,[UIntPtr]::Zero); [CT]::mouse_event(0x0004,0,0,0,[UIntPtr]::Zero)
Start-Sleep -Milliseconds 900
Grab "ct_after"
Write-Output ("clique em " + ($r.L+$dx) + "," + ($r.B-$dy))
