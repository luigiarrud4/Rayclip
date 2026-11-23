Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices;
public class SS {
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string c,string n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f,uint dx,uint dy,uint d,UIntPtr e);
  public struct RECT { public int L,T,R,B; }
}
"@
$h=[SS]::FindWindow("Tauri Window","Rayclip")
function Shot($n){
  $b=[System.Windows.Forms.Screen]::PrimaryScreen.Bounds
  $r=New-Object SS+RECT; [SS]::GetWindowRect($h,[ref]$r)|Out-Null
  $bmp=New-Object System.Drawing.Bitmap $b.Width,$b.Height
  $g=[System.Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($b.Location,[System.Drawing.Point]::Empty,$b.Size)
  $c=$bmp.Clone((New-Object System.Drawing.Rectangle (($r.L-20),($r.T-20),($($r.R-$r.L)+40),($($r.B-$r.T)+40))),$bmp.PixelFormat)
  $c.Save("C:\Windows\System32\_rayclip_tmp\$n.png",[System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose();$bmp.Dispose();$c.Dispose()
}
$form=New-Object System.Windows.Forms.Form; $form.Width=200; $form.Height=70; $form.Text="drvS2"
$t=New-Object System.Windows.Forms.Timer; $t.Interval=800; $script:s=0
$t.Add_Tick({
  $script:s++
  $k=[System.Windows.Forms.SendKeys]
  switch($script:s){
    1 { $k::SendWait("^+v") }
    2 { if(-not [SS]::IsWindowVisible($h)){ $k::SendWait("^+v") } }
    4 { $k::SendWait("{HOME}") }
    6 { Shot "img_big" }
    7 { $r=New-Object SS+RECT; [SS]::GetWindowRect($h,[ref]$r)|Out-Null
        [SS]::SetCursorPos(($r.R-140),($r.T+300))
        for($i=0;$i -lt 6;$i++){ [SS]::mouse_event(0x0800,0,0,4294967056,[UIntPtr]::Zero); Start-Sleep -Milliseconds 90 } }
    9 { Shot "img_scrolled"; $t.Stop(); $form.Close() }
  }
})
$form.Add_Shown({$t.Start()}); [System.Windows.Forms.Application]::Run($form)
