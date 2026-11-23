Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices;
public class GS {
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string c,string n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  public struct RECT { public int L,T,R,B; }
}
"@
$h=[GS]::FindWindow("Tauri Window","Rayclip")
function Shot($n){
  $b=[System.Windows.Forms.Screen]::PrimaryScreen.Bounds
  $r=New-Object GS+RECT; [GS]::GetWindowRect($h,[ref]$r)|Out-Null
  $bmp=New-Object System.Drawing.Bitmap $b.Width,$b.Height
  $g=[System.Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($b.Location,[System.Drawing.Point]::Empty,$b.Size)
  $c=$bmp.Clone((New-Object System.Drawing.Rectangle (($r.L-24),($r.T-24),($($r.R-$r.L)+48),($($r.B-$r.T)+48))),$bmp.PixelFormat)
  $c.Save("C:\Windows\System32\_rayclip_tmp\$n.png",[System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose();$bmp.Dispose();$c.Dispose()
}
$form=New-Object System.Windows.Forms.Form; $form.Width=200; $form.Height=70; $form.Text="drvG"
$t=New-Object System.Windows.Forms.Timer; $t.Interval=700; $script:s=0
$t.Add_Tick({
  $script:s++
  $k=[System.Windows.Forms.SendKeys]
  switch($script:s){
    1 { $k::SendWait("^+v") }
    2 { if(-not [GS]::IsWindowVisible($h)){ $k::SendWait("^+v") } }
    4 { $k::SendWait("{HOME}") }
    6 { Shot "gif_a" }
    7 { Shot "gif_b" }
    8 { $t.Stop(); $form.Close() }
  }
})
$form.Add_Shown({$t.Start()}); [System.Windows.Forms.Application]::Run($form)
