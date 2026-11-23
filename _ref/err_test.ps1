Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices;
public class ER {
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string c,string n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  public struct RECT { public int L,T,R,B; }
}
"@
$h=[ER]::FindWindow("Tauri Window","Rayclip")
$form=New-Object System.Windows.Forms.Form; $form.Width=200; $form.Height=70; $form.Text="drvE"
$t=New-Object System.Windows.Forms.Timer; $t.Interval=650; $script:s=0
$t.Add_Tick({
  $script:s++
  $k=[System.Windows.Forms.SendKeys]
  switch($script:s){
    1 { $k::SendWait("^+v") }
    2 { if(-not [ER]::IsWindowVisible($h)){ $k::SendWait("^+v") } }
    4 { $k::SendWait("{HOME}") }
    6 { $k::SendWait("^c") }
    9 { $b=[System.Windows.Forms.Screen]::PrimaryScreen.Bounds
        $r=New-Object ER+RECT; [ER]::GetWindowRect($h,[ref]$r)|Out-Null
        $bmp=New-Object System.Drawing.Bitmap $b.Width,$b.Height
        $g=[System.Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($b.Location,[System.Drawing.Point]::Empty,$b.Size)
        $c=$bmp.Clone((New-Object System.Drawing.Rectangle (($r.L-24),($r.T-24),($($r.R-$r.L)+48),($($r.B-$r.T)+48))),$bmp.PixelFormat)
        $c.Save("C:\Windows\System32\_rayclip_tmp\s_error.png",[System.Drawing.Imaging.ImageFormat]::Png)
        $g.Dispose();$bmp.Dispose();$c.Dispose()
        $t.Stop(); $form.Close() }
  }
})
$form.Add_Shown({$t.Start()}); [System.Windows.Forms.Application]::Run($form)
