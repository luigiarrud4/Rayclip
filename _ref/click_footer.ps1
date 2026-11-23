Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices;
public class CF {
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string c,string n);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f,uint dx,uint dy,uint d,UIntPtr e);
  public struct RECT { public int L,T,R,B; }
}
"@
$h=[CF]::FindWindow("Tauri Window","Rayclip")
$r=New-Object CF+RECT; [CF]::GetWindowRect($h,[ref]$r)|Out-Null
Write-Output ("janela: " + $r.L + "," + $r.T + " " + ($r.R-$r.L) + "x" + ($r.B-$r.T))
# icone da area esquerda do rodape (~14px da borda, 21px do fundo)
[CF]::SetCursorPos(($r.L+18),($r.B-30))
Start-Sleep -Milliseconds 200
[CF]::mouse_event(0x0002,0,0,0,[UIntPtr]::Zero); [CF]::mouse_event(0x0004,0,0,0,[UIntPtr]::Zero); Start-Sleep -Milliseconds 400; [CF]::mouse_event(0x0002,0,0,0,[UIntPtr]::Zero); [CF]::mouse_event(0x0004,0,0,0,[UIntPtr]::Zero)
Start-Sleep -Milliseconds 800
$b=[System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$bmp=New-Object System.Drawing.Bitmap $b.Width,$b.Height
$g=[System.Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($b.Location,[System.Drawing.Point]::Empty,$b.Size)
$c=$bmp.Clone((New-Object System.Drawing.Rectangle (($r.L-20),($r.T-20),($($r.R-$r.L)+40),($($r.B-$r.T)+40))),$bmp.PixelFormat)
$c.Save("C:\Windows\System32\_rayclip_tmp\click_footer.png",[System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose();$bmp.Dispose();$c.Dispose()
