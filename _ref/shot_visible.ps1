param([string]$name="shot")
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices;
public class SV {
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string c,string n);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  public struct RECT { public int L,T,R,B; }
}
"@
$h=[SV]::FindWindow("Tauri Window","Rayclip")
$r=New-Object SV+RECT; [SV]::GetWindowRect($h,[ref]$r)|Out-Null
$b=[System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$bmp=New-Object System.Drawing.Bitmap (($r.R-$r.L)+40),(($r.B-$r.T)+40)
$g=[System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen(($r.L-20),($r.T-20),0,0,$bmp.Size)
$bmp.Save("C:\Windows\System32\_rayclip_tmp\$name.png",[System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose();$bmp.Dispose()
Write-Output ("visivel: " + [SV]::IsWindowVisible($h) + " rect=" + $r.L + "," + $r.T)
