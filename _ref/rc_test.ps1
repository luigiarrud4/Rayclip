Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public class M {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, UIntPtr e);
  public const uint RDOWN=0x0008, RUP=0x0010, MOVE=0x0001;
  public static void RClick(int x,int y){ SetCursorPos(x,y); mouse_event(RDOWN,0,0,0,UIntPtr.Zero); System.Threading.Thread.Sleep(60); mouse_event(RUP,0,0,0,UIntPtr.Zero); }
}
"@
function Shot($n) {
  $b=[System.Windows.Forms.Screen]::PrimaryScreen.Bounds
  $x0=[int](($b.Width-750)/2); $y0=[int](($b.Height-475)*0.18)
  $bmp=New-Object System.Drawing.Bitmap $b.Width,$b.Height
  $g=[System.Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($b.Location,[System.Drawing.Point]::Empty,$b.Size)
  $c=$bmp.Clone((New-Object System.Drawing.Rectangle (($x0-24),($y0-24),798,523)),$bmp.PixelFormat)
  $c.Save("C:\Windows\System32\_rayclip_tmp\$n.png",[System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose();$bmp.Dispose();$c.Dispose()
}
$b=[System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$script:x0=[int](($b.Width-750)/2); $script:y0=[int](($b.Height-475)*0.18)
$form=New-Object System.Windows.Forms.Form; $form.Width=200; $form.Height=80; $form.Text="driver"
$t=New-Object System.Windows.Forms.Timer; $t.Interval=520; $script:s=0
$t.Add_Tick({
  $script:s++
  $k=[System.Windows.Forms.SendKeys]
  switch($script:s){
    1  { $k::SendWait("^+v") }
    3  { $k::SendWait("^+v") }
    6  { Shot "rc_before" }
    7  { [M]::RClick(($script:x0+150), ($script:y0+118)) }   # 2nd row
    10 { Shot "rc_after" }
    12 { $t.Stop(); $form.Close() }
  }
})
$form.Add_Shown({$t.Start()}); [System.Windows.Forms.Application]::Run($form); Write-Output ok
